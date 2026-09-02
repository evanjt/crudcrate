//! Covers `read::one::body` on a resource served through a `ScopeCondition`.
//!
//! Scenario: an entity uses a custom get-one body (soft-delete style fetch that
//! also marks the result) and is mounted behind scope middleware.
//! Expected behaviour: scope eligibility is enforced in SQL first, so a row the
//! scope excludes is 404, while an eligible row still flows through the custom
//! body. The unscoped mount keeps running the custom body unchanged.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{ApiError, CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{Condition, DatabaseConnection, DbErr};
use tower::ServiceExt;
use uuid::Uuid;

pub mod scb_item {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "scb_items")]
    #[crudcrate(
        generate_router,
        api_struct = "ScbItem",
        read::one::body = fetch_item_marked
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[crudcrate(filterable)]
        pub active: bool,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}

    /// Custom get-one body: fetch by id and stamp the name so responses that ran
    /// through it are distinguishable from the default body.
    async fn fetch_item_marked(db: &DatabaseConnection, id: Uuid) -> Result<ScbItem, ApiError> {
        let model = Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| ApiError::not_found("scb_item", Some(id.to_string())))?;
        let mut item = ScbItem::from(model);
        item.name = format!("{} [custom]", item.name);
        Ok(item)
    }
}

use scb_item::{ScbItem, ScbItemCreate};

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(scb_item::Entity).await
}

fn app_unscoped(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/items", ScbItem::router(db).into())
}

/// Scope that only admits active rows, the soft-delete visibility rule.
fn app_scoped(db: &DatabaseConnection) -> axum::Router {
    app_unscoped(db).layer(axum::Extension(crudcrate::ScopeCondition {
        condition: Condition::all().add(scb_item::Column::Active.eq(true)),
    }))
}

async fn get(app: axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    (status, json)
}

async fn create_item(db: &DatabaseConnection, name: &str, active: bool) -> ScbItem {
    ScbItem::create(
        db,
        ScbItemCreate {
            name: name.to_string(),
            active,
        },
    )
    .await
    .expect("direct trait create should succeed")
}

#[tokio::test]
async fn scoped_get_one_in_scope_runs_custom_body() {
    let db = setup_test_db().await.unwrap();
    let created = create_item(&db, "visible", true).await;

    let (status, json) = get(app_scoped(&db), &format!("/items/{}", created.id)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json["name"], "visible [custom]",
        "an in-scope row must be shaped by the read::one::body hook"
    );
}

#[tokio::test]
async fn scoped_get_one_out_of_scope_returns_404() {
    let db = setup_test_db().await.unwrap();
    let created = create_item(&db, "hidden", false).await;

    let (status, _) = get(app_scoped(&db), &format!("/items/{}", created.id)).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a row the scope excludes must be 404 even though a custom body exists"
    );
}

#[tokio::test]
async fn scoped_get_one_missing_row_returns_404() {
    let db = setup_test_db().await.unwrap();

    let (status, _) = get(app_scoped(&db), &format!("/items/{}", Uuid::new_v4())).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unscoped_get_one_runs_custom_body() {
    let db = setup_test_db().await.unwrap();
    let created = create_item(&db, "plain", false).await;

    let (status, json) = get(app_unscoped(&db), &format!("/items/{}", created.id)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json["name"], "plain [custom]",
        "the unscoped path must keep running the read::one::body hook"
    );
}
