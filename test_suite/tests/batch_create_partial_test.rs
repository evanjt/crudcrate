//! Tests for the `create_many` partial-success branches (`POST /batch?partial=true`).
//!
//! These cover the create-side branches of the partial batch handler that were
//! previously left untested (the original create partial test was `#[ignore]`d):
//!
//! - HTTP 207 Multi-Status when some items succeed and some fail
//! - HTTP 400 Bad Request when every item fails
//! - HTTP 201 Created (still `BatchResult` shape) when every item succeeds
//! - Item shape consistency between all-or-nothing and partial-success responses
//!
//! Per-item failure is driven by `crudcrate::validation::Validatable` implemented on
//! the generated Create model. The partial create handler invokes
//! `CRUDResource::create` once per item, so a `create::one::pre` hook that runs the
//! Create model's `validate()` makes individual items fail (rejecting `name == "bad"`
//! or an empty name) while the rest commit.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use crudcrate::validation::{Validatable, ValidationError};
use crudcrate::{ApiError, CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

pub mod bcp_item {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "bcp_items")]
    #[crudcrate(
        generate_router,
        api_struct = "BcpItem",
        name_singular = "bcp_item",
        name_plural = "bcp_items",
        create::one::pre = validate_bcp_item_create,
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

use bcp_item::BcpItemCreate;

/// Reject the literal name `"bad"` and any empty/whitespace-only name.
///
/// Implemented on the generated Create model so that the per-item validation in
/// the partial batch create path has a concrete failure to trip on.
impl Validatable for BcpItemCreate {
    fn validate(&self) -> Result<(), ValidationError> {
        if self.name.trim().is_empty() {
            return Err(ValidationError::new("name", "Name cannot be empty"));
        }
        if self.name == "bad" {
            return Err(ValidationError::new("name", "Name 'bad' is rejected"));
        }
        Ok(())
    }
}

/// `create::one::pre` hook bridging the `Validatable` impl into the create path.
///
/// The partial batch create handler calls `CRUDResource::create` per item, which
/// runs this hook before the insert — so a failure here fails just that one item.
///
/// Must be `async` to match the hook signature the derive macro calls (`.await`).
#[allow(clippy::unused_async)]
async fn validate_bcp_item_create(
    _db: &sea_orm::DatabaseConnection,
    data: &BcpItemCreate,
) -> Result<(), ApiError> {
    data.validate()
        .map_err(|e| ApiError::bad_request(e.to_string()))
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let db = Database::connect(&url).await?;
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    // Persistent backends (Postgres/MySQL) keep tables across tests within a binary;
    // drop first so every test starts from a clean schema and empty data. On
    // sqlite::memory: each connection is a fresh database, so the drop is a no-op.
    db.execute(&Table::drop().table(bcp_item::Entity).if_exists().to_owned())
        .await?;
    db.execute(&schema.create_table_from_entity(bcp_item::Entity))
        .await?;
    Ok(db)
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/items", bcp_item::BcpItem::router(db).into())
}

async fn post_batch(
    db: &DatabaseConnection,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app(db).oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    (status, value)
}

async fn list_names(db: &DatabaseConnection) -> Vec<String> {
    let request = Request::builder()
        .method("GET")
        .uri("/items?range=[0,99]")
        .body(Body::empty())
        .unwrap();
    let response = app(db).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    rows.into_iter()
        .map(|r| r["name"].as_str().unwrap().to_string())
        .collect()
}

/// Mixed batch -> 207 Multi-Status, with only the valid rows committed.
#[tokio::test]
async fn test_create_partial_mixed_returns_207() {
    let db = setup_test_db().await.expect("db setup");

    let (status, result) = post_batch(
        &db,
        "/items/batch?partial=true",
        json!([{"name": "good1"}, {"name": "bad"}, {"name": "good2"}]),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::MULTI_STATUS,
        "mixed partial create should return 207"
    );

    let succeeded = result["succeeded"]
        .as_array()
        .expect("succeeded array present");
    assert_eq!(succeeded.len(), 2, "two items should succeed");

    let failed = result["failed"].as_array().expect("failed array present");
    assert_eq!(failed.len(), 1, "one item should fail");
    assert_eq!(
        failed[0]["index"].as_u64().unwrap(),
        1,
        "the failing item is at index 1"
    );

    // The two good rows must actually be persisted; the bad one must not.
    let mut names = list_names(&db).await;
    names.sort();
    assert_eq!(names, vec!["good1".to_string(), "good2".to_string()]);
}

/// Every item invalid -> 400 Bad Request, empty `succeeded`, all in `failed`.
#[tokio::test]
async fn test_create_partial_all_failed_returns_400() {
    let db = setup_test_db().await.expect("db setup");

    let (status, result) = post_batch(
        &db,
        "/items/batch?partial=true",
        json!([{"name": "bad"}, {"name": "bad"}]),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "all-failed partial create should return 400"
    );

    let succeeded = result["succeeded"]
        .as_array()
        .expect("succeeded array present");
    assert!(succeeded.is_empty(), "no items should succeed");

    let failed = result["failed"].as_array().expect("failed array present");
    assert_eq!(failed.len(), 2, "both items should fail");

    // Nothing should have been persisted.
    assert!(
        list_names(&db).await.is_empty(),
        "no rows should be created"
    );
}

/// Every item valid under partial mode -> 201 Created, still `BatchResult` shape.
#[tokio::test]
async fn test_create_partial_all_succeed_returns_201() {
    let db = setup_test_db().await.expect("db setup");

    let (status, result) = post_batch(
        &db,
        "/items/batch?partial=true",
        json!([{"name": "alpha"}, {"name": "beta"}, {"name": "gamma"}]),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::CREATED,
        "all-valid partial create should return 201"
    );

    let succeeded = result["succeeded"]
        .as_array()
        .expect("succeeded array present");
    assert_eq!(succeeded.len(), 3, "all three items should succeed");

    let failed = result["failed"].as_array().expect("failed array present");
    assert!(failed.is_empty(), "no items should fail");

    let mut names = list_names(&db).await;
    names.sort();
    assert_eq!(
        names,
        vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()]
    );
}

/// The per-item JSON shape is identical between all-or-nothing and partial modes.
#[tokio::test]
async fn test_create_item_shape_consistency() {
    let plain_db = setup_test_db().await.expect("db setup");
    let (plain_status, plain_body) = post_batch(
        &plain_db,
        "/items/batch",
        json!([{"name": "shape1"}, {"name": "shape2"}]),
    )
    .await;
    assert_eq!(
        plain_status,
        StatusCode::CREATED,
        "all-or-nothing create should return 201"
    );

    // All-or-nothing returns a bare array of items.
    let plain_items = plain_body.as_array().expect("plain create returns array");
    assert_eq!(plain_items.len(), 2);

    let partial_db = setup_test_db().await.expect("db setup");
    let (partial_status, partial_body) = post_batch(
        &partial_db,
        "/items/batch?partial=true",
        json!([{"name": "shape1"}, {"name": "shape2"}]),
    )
    .await;
    assert_eq!(
        partial_status,
        StatusCode::CREATED,
        "all-valid partial create should return 201"
    );

    // Partial mode wraps the same items under `succeeded`.
    let partial_items = partial_body["succeeded"]
        .as_array()
        .expect("partial create returns succeeded array");
    assert_eq!(partial_items.len(), 2);

    let keys = |item: &serde_json::Value| -> Vec<String> {
        let mut ks: Vec<String> = item
            .as_object()
            .expect("item is a JSON object")
            .keys()
            .cloned()
            .collect();
        ks.sort();
        ks
    };

    assert_eq!(
        keys(&plain_items[0]),
        keys(&partial_items[0]),
        "item keys must match between all-or-nothing and partial modes"
    );
    // Sanity: the expected fields are actually present.
    assert_eq!(
        keys(&plain_items[0]),
        vec!["id".to_string(), "name".to_string()]
    );
}
