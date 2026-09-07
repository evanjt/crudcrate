//! The batch routes run the per-row lifecycle: `before_create`/`after_create`,
//! `before_update`/`after_update` and `before_delete`/`after_delete` fire once per row, alongside
//! the batch-level `before_delete_many`/`after_delete_many`.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{ApiError, CRUDOperations, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use tower::ServiceExt;
use uuid::Uuid;

static BEFORE_CREATE: AtomicUsize = AtomicUsize::new(0);
static AFTER_CREATE: AtomicUsize = AtomicUsize::new(0);
static BEFORE_UPDATE: AtomicUsize = AtomicUsize::new(0);
static AFTER_UPDATE: AtomicUsize = AtomicUsize::new(0);
static BEFORE_DELETE: AtomicUsize = AtomicUsize::new(0);
static AFTER_DELETE: AtomicUsize = AtomicUsize::new(0);
static BEFORE_DELETE_MANY: AtomicUsize = AtomicUsize::new(0);
static AFTER_DELETE_MANY: AtomicUsize = AtomicUsize::new(0);

fn reset_counters() {
    for counter in [
        &BEFORE_CREATE,
        &AFTER_CREATE,
        &BEFORE_UPDATE,
        &AFTER_UPDATE,
        &BEFORE_DELETE,
        &AFTER_DELETE,
        &BEFORE_DELETE_MANY,
        &AFTER_DELETE_MANY,
    ] {
        counter.store(0, Ordering::SeqCst);
    }
}

pub mod counted_widget {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "counted_widgets")]
    #[crudcrate(generate_router, api_struct = "CountedWidget", operations = CountedOps)]
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

    pub struct CountedOps;

    #[async_trait::async_trait]
    impl CRUDOperations for CountedOps {
        type Resource = CountedWidget;

        async fn before_create(
            &self,
            _db: &DatabaseConnection,
            _data: &CountedWidgetCreate,
        ) -> Result<(), ApiError> {
            BEFORE_CREATE.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn after_create(
            &self,
            _db: &DatabaseConnection,
            _entity: &mut CountedWidget,
        ) -> Result<(), ApiError> {
            AFTER_CREATE.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn before_update(
            &self,
            _db: &DatabaseConnection,
            _id: Uuid,
            _data: &CountedWidgetUpdate,
        ) -> Result<(), ApiError> {
            BEFORE_UPDATE.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn after_update(
            &self,
            _db: &DatabaseConnection,
            _entity: &mut CountedWidget,
        ) -> Result<(), ApiError> {
            AFTER_UPDATE.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn before_delete(
            &self,
            _db: &DatabaseConnection,
            _id: Uuid,
        ) -> Result<(), ApiError> {
            BEFORE_DELETE.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn after_delete(&self, _db: &DatabaseConnection, _id: Uuid) -> Result<(), ApiError> {
            AFTER_DELETE.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn before_delete_many(
            &self,
            _db: &DatabaseConnection,
            _ids: &[Uuid],
        ) -> Result<(), ApiError> {
            BEFORE_DELETE_MANY.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn after_delete_many(
            &self,
            _db: &DatabaseConnection,
            _ids: &[Uuid],
        ) -> Result<(), ApiError> {
            AFTER_DELETE_MANY.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
}

use counted_widget::CountedWidget;

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(counted_widget::Entity).await
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/widgets", CountedWidget::router(db).into())
}

async fn body_json(resp: axum::response::Response) -> serde_json::Value {
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn every_batch_route_runs_its_row_hooks() {
    reset_counters();
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/widgets/batch")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!([{ "name": "Anvil" }, { "name": "Bellows" }]).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let created = body_json(resp).await;
    let rows = created.as_array().unwrap().clone();
    assert_eq!(rows.len(), 2);
    assert_eq!(BEFORE_CREATE.load(Ordering::SeqCst), 2);
    assert_eq!(AFTER_CREATE.load(Ordering::SeqCst), 2);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/widgets/batch")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!([
                        { "id": rows[0]["id"], "name": "Anvil II" },
                        { "id": rows[1]["id"], "name": "Bellows II" },
                    ])
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(BEFORE_UPDATE.load(Ordering::SeqCst), 2);
    assert_eq!(AFTER_UPDATE.load(Ordering::SeqCst), 2);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/widgets/batch")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!([rows[0]["id"], rows[1]["id"]]).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(BEFORE_DELETE.load(Ordering::SeqCst), 2);
    assert_eq!(AFTER_DELETE.load(Ordering::SeqCst), 2);
    // The batch-level pair still fires once for the request.
    assert_eq!(BEFORE_DELETE_MANY.load(Ordering::SeqCst), 1);
    assert_eq!(AFTER_DELETE_MANY.load(Ordering::SeqCst), 1);
}
