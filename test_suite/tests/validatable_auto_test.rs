//! Regression: `Validatable` is invoked automatically by the generated CRUD
//! handlers (A10).
//!
//! The derive-generated `create`/`create_many`/`update` implementations call
//! `crudcrate::validation::__auto::Probe(&data).crudcrate_auto_validate()` before any
//! database write. When the Create/Update model implements
//! [`crudcrate::validation::Validatable`], the real `validate()` runs and a failure
//! surfaces as HTTP 422. Models that do NOT implement it get the no-op
//! autoref-specialization fallback, so creation succeeds unchanged.
//!
//! This file is fully self-contained: it defines its own entities, its own
//! `setup_test_db`, and its own router wiring. Run with:
//!     DATABASE_URL="sqlite::memory:" cargo test -p test_suite --test validatable_auto_test -- --test-threads=1

use axum::body::Body;
use axum::http::{Request, StatusCode};
use crudcrate::validation::{Validatable, ValidationError};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use tower::ServiceExt;
use uuid::Uuid;

// =============================================================================
// Validated entity: name must be present and at least 3 characters long.
// =============================================================================

pub mod product {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "vld_products")]
    #[crudcrate(generate_router, api_struct = "VldProduct")]
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

fn name_is_valid(name: &str) -> Result<(), ValidationError> {
    if name.is_empty() {
        return Err(ValidationError::new("name", "Name is required"));
    }
    if name.chars().count() < 3 {
        return Err(ValidationError::new(
            "name",
            "Name must be at least 3 characters",
        ));
    }
    Ok(())
}

impl Validatable for product::VldProductCreate {
    fn validate(&self) -> Result<(), ValidationError> {
        name_is_valid(&self.name)
    }
}

impl Validatable for product::VldProductUpdate {
    fn validate(&self) -> Result<(), ValidationError> {
        // The update model wraps every column as `Option<Option<T>>`: outer `None`
        // means "field absent from request", inner `None` means "set to NULL".
        // Validate the supplied string only when it is actually present.
        if let Some(Some(name)) = &self.name {
            name_is_valid(name)?;
        }
        Ok(())
    }
}

// =============================================================================
// Unvalidated entity: identical shape, but no `Validatable` impl. Proves the
// auto-validation is a no-op for models that don't opt in.
// =============================================================================

pub mod widget {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "vld_widgets")]
    #[crudcrate(generate_router, api_struct = "VldWidget")]
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

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let db = Database::connect(&url).await?;
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    // Persistent backends (Postgres/MySQL) keep tables across tests within a binary;
    // drop first so every test starts from a clean schema and empty data. On
    // sqlite::memory: each connection is a fresh database, so the drops are no-ops.
    // create_table_from_entity emits no FK constraints, so drop order is irrelevant.
    for stmt in [
        Table::drop().table(product::Entity).if_exists().to_owned(),
        Table::drop().table(widget::Entity).if_exists().to_owned(),
    ] {
        db.execute(&stmt).await?;
    }

    db.execute(&schema.create_table_from_entity(product::Entity))
        .await?;
    db.execute(&schema.create_table_from_entity(widget::Entity))
        .await?;
    Ok(db)
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new()
        .nest("/products", product::VldProduct::router(db).into())
        .nest("/widgets", widget::VldWidget::router(db).into())
}

// =============================================================================
// Test 1: POST validation runs (empty -> 422, valid -> 201)
// =============================================================================

#[tokio::test]
async fn post_empty_name_is_rejected_with_422() {
    let db = setup_test_db().await.expect("setup db");

    let response = app(&db)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/products")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::json!({"name": ""}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "empty name must fail validation with 422"
    );

    // Nothing was persisted.
    assert_eq!(count_products(&db).await, 0);
}

#[tokio::test]
async fn post_valid_name_is_created() {
    let db = setup_test_db().await.expect("setup db");

    let response = app(&db)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/products")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::json!({"name": "okay"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "a >=3-char name must pass validation and be created"
    );

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["name"], "okay");

    assert_eq!(count_products(&db).await, 1);
}

#[tokio::test]
async fn post_too_short_name_is_rejected_with_422() {
    let db = setup_test_db().await.expect("setup db");

    let response = app(&db)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/products")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::json!({"name": "ab"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "a 2-char name is shorter than the 3-char minimum"
    );
    assert_eq!(count_products(&db).await, 0);
}

// =============================================================================
// Test 2: PUT validation runs (empty -> 422, valid -> 200)
// =============================================================================

#[tokio::test]
async fn put_empty_name_is_rejected_with_422() {
    let db = setup_test_db().await.expect("setup db");
    let id = create_valid_product(&db).await;

    let response = app(&db)
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/products/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::json!({"name": ""}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "updating to an empty name must fail validation with 422"
    );

    // The original name is unchanged.
    assert_eq!(get_product_name(&db, id).await, "valid name");
}

#[tokio::test]
async fn put_valid_name_succeeds() {
    let db = setup_test_db().await.expect("setup db");
    let id = create_valid_product(&db).await;

    let response = app(&db)
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/products/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"name": "renamed"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "a valid update must succeed with 200"
    );

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["name"], "renamed");
    assert_eq!(get_product_name(&db, id).await, "renamed");
}

// =============================================================================
// Test 3: Batch create is all-or-nothing; one invalid item rejects the batch.
// =============================================================================

#[tokio::test]
async fn batch_create_with_one_invalid_item_persists_nothing() {
    let db = setup_test_db().await.expect("setup db");

    let payload = serde_json::json!([
        {"name": "first ok"},
        {"name": ""},
        {"name": "third ok"}
    ]);

    let response = app(&db)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/products/batch")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // All-or-nothing (no ?partial): the validation failure aborts the whole batch.
    // It must not be a 201, and validation maps to 422.
    assert_ne!(
        response.status(),
        StatusCode::CREATED,
        "a batch containing an invalid item must not report success"
    );
    assert!(
        response.status() == StatusCode::UNPROCESSABLE_ENTITY
            || response.status() == StatusCode::BAD_REQUEST,
        "validation failure should be 422 (or 400); got {}",
        response.status()
    );

    // No rows persisted: the transaction never committed.
    assert_eq!(
        count_products(&db).await,
        0,
        "no products should be persisted when the batch is rejected"
    );
}

#[tokio::test]
async fn batch_create_all_valid_succeeds() {
    let db = setup_test_db().await.expect("setup db");

    let payload = serde_json::json!([
        {"name": "alpha"},
        {"name": "beta"}
    ]);

    let response = app(&db)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/products/batch")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "an all-valid batch must succeed"
    );
    assert_eq!(count_products(&db).await, 2);
}

// =============================================================================
// Test 4: A model without `Validatable` accepts an empty name (fallback no-op).
// =============================================================================

#[tokio::test]
async fn unvalidated_model_accepts_empty_name() {
    let db = setup_test_db().await.expect("setup db");

    let response = app(&db)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/widgets")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::json!({"name": ""}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "a model that does not implement Validatable must skip validation (no-op fallback)"
    );

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["name"], "");
}

// =============================================================================
// Helpers
// =============================================================================

async fn count_products(db: &DatabaseConnection) -> u64 {
    product::Entity::find().count(db).await.expect("count")
}

async fn create_valid_product(db: &DatabaseConnection) -> Uuid {
    let created = product::VldProduct::create(
        db,
        product::VldProductCreate {
            name: "valid name".to_string(),
        },
    )
    .await
    .expect("create valid product");
    created.id
}

async fn get_product_name(db: &DatabaseConnection, id: Uuid) -> String {
    product::Entity::find_by_id(id)
        .one(db)
        .await
        .expect("query")
        .expect("product exists")
        .name
}
