//! Coverage for the default method bodies of `crudcrate::CRUDOperations`.
//!
//! The model is wired with `operations = CodOps`, where `CodOps` implements
//! `CRUDOperations` with NO method overrides. Every request therefore exercises
//! the trait's *default* orchestrators (`get_one`, `get_all`, `create`, `update`,
//! `delete`, `delete_many`) and the default core methods they delegate to
//! (`fetch_one`, `fetch_all`, `perform_create`, `perform_update`,
//! `perform_delete`, `perform_delete_many`).
//!
//! ACTUAL-BEHAVIOUR NOTE — `create_many` / `update_many` infinite recursion:
//! When `operations = X` is configured, the derive macro generates
//! `Resource::create_many` / `Resource::update_many` so they delegate to
//! `CRUDOperations::create_many` / `update_many` on the ops struct. But the
//! *default* bodies of those two trait methods delegate straight back to
//! `Self::Resource::create_many` / `update_many` (see crudcrate/src/operations.rs
//! ~line 623 and 644). With no override that is unconditional mutual recursion
//! and any call — POST /batch, PATCH /batch, even an empty batch — aborts the
//! whole process with a stack overflow. (`create`/`update`/`delete`/
//! `get_one`/`get_all`/`delete_many` do not have this problem: their default
//! bodies delegate to `perform_*` / `fetch_*`, which hit the database directly.)
//! These two endpoints are therefore deliberately NOT driven at runtime here;
//! `create_many_update_many_signatures_exist` instead binds their function
//! pointers so the generated code is still type-checked and documents the
//! defect without aborting the test binary.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDOperations, CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

pub mod cod_widget {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "cod_widgets")]
    #[crudcrate(generate_router, api_struct = "CodWidget", operations = CodOps)]
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

    // No method overrides: every CRUDOperations call runs the default trait body.
    pub struct CodOps;

    #[async_trait::async_trait]
    impl CRUDOperations for CodOps {
        type Resource = CodWidget;
    }
}

use cod_widget::CodWidget;

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let db = Database::connect("sqlite::memory:").await?;
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);
    db.execute(backend.build(&schema.create_table_from_entity(cod_widget::Entity)))
        .await?;
    Ok(db)
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/widgets", CodWidget::router(db).into())
}

// Same router under the legacy security profile, where `expose_deleted_ids` is
// true so DELETE /batch returns the array of deleted UUIDs instead of a count.
fn app_legacy(db: &DatabaseConnection) -> axum::Router {
    app(db).layer(axum::Extension(crudcrate::SecurityProfile::legacy()))
}

async fn body_json(resp: axum::response::Response) -> serde_json::Value {
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn create_widget(app: &axum::Router, name: &str) -> serde_json::Value {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/widgets")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "name": name }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    body_json(resp).await
}

// ---------------------------------------------------------------------------
// create + perform_create  (POST /)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_runs_perform_create_and_returns_201() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    let created = create_widget(&app, "Anchor").await;
    assert_eq!(created["name"], "Anchor");
    // Default perform_create assigned the on_create UUID.
    assert!(Uuid::parse_str(created["id"].as_str().unwrap()).is_ok());
}

// ---------------------------------------------------------------------------
// get_one + fetch_one  (GET /{id})
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_one_runs_fetch_one_and_round_trips() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    let created = create_widget(&app, "Beacon").await;
    let id = created["id"].as_str().unwrap();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/widgets/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let fetched = body_json(resp).await;
    assert_eq!(fetched["id"], created["id"]);
    assert_eq!(fetched["name"], "Beacon");
}

#[tokio::test]
async fn get_one_missing_returns_404_from_fetch_one() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    let missing = Uuid::new_v4();
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/widgets/{missing}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// get_all + fetch_all  (GET /)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_all_runs_fetch_all_and_lists_every_row() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    for n in ["Cog", "Dial", "Gear"] {
        create_widget(&app, n).await;
    }

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/widgets")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let list = body_json(resp).await;
    let arr = list.as_array().expect("list response is a JSON array");
    assert_eq!(arr.len(), 3);

    let mut names: Vec<String> = arr
        .iter()
        .map(|v| v["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    assert_eq!(names, vec!["Cog", "Dial", "Gear"]);
}

#[tokio::test]
async fn get_all_honours_sort_through_fetch_all() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    for n in ["Cog", "Dial", "Gear"] {
        create_widget(&app, n).await;
    }

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/widgets?sort=%5B%22name%22%2C%22DESC%22%5D")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let list = body_json(resp).await;
    let names: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    // fetch_all applied the DESC order_column / order_direction.
    assert_eq!(names, vec!["Gear", "Dial", "Cog"]);
}

// ---------------------------------------------------------------------------
// update + perform_update  (PUT /{id})
// ---------------------------------------------------------------------------

#[tokio::test]
async fn update_runs_perform_update_and_changes_value() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    let created = create_widget(&app, "Hinge").await;
    let id = created["id"].as_str().unwrap();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/widgets/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "name": "Latch" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let updated = body_json(resp).await;
    assert_eq!(updated["id"], created["id"]);
    assert_eq!(updated["name"], "Latch");

    // Confirm the change is persisted by reading it back through fetch_one.
    let read = CodWidget::get_one(&db, Uuid::parse_str(id).unwrap())
        .await
        .unwrap();
    assert_eq!(read.name, "Latch");
}

#[tokio::test]
async fn update_missing_returns_404_from_perform_update() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    let missing = Uuid::new_v4();
    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/widgets/{missing}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "name": "Ghost" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// delete + perform_delete  (DELETE /{id})
// ---------------------------------------------------------------------------

#[tokio::test]
async fn delete_runs_perform_delete_and_removes_row() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    let created = create_widget(&app, "Mallet").await;
    let id = created["id"].as_str().unwrap();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/widgets/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // The deleted id is gone (fetch_one now 404s).
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/widgets/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_missing_returns_404_from_perform_delete() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    let missing = Uuid::new_v4();
    let resp = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/widgets/{missing}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// create_many / update_many  (POST /batch, PATCH /batch)
//
// These cannot be driven at runtime: with `operations = CodOps` and no override,
// the default `CRUDOperations::create_many` / `update_many` recurse infinitely
// (see the module doc-comment). Calling either — including an empty batch —
// stack-overflows and aborts the whole test process. We therefore only bind the
// generated function pointers, which keeps the code type-checked and documents
// the defect without aborting the binary.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_many_update_many_signatures_exist() {
    // The generated impls exist and are name-resolvable through `CRUDResource`.
    // Naming the function items (without invoking them) proves the
    // `operations = CodOps` codegen wired both batch methods. Invoking them is
    // intentionally avoided because the default trait bodies recurse without
    // bound and would abort the process.
    fn name_only<T>(_f: T) {}
    name_only(<CodWidget as CRUDResource>::create_many);
    name_only(<CodWidget as CRUDResource>::update_many);

    // Argument types are exercised by constructing the values the generated
    // methods accept, without ever passing them in.
    let create_arg: Vec<cod_widget::CodWidgetCreate> = vec![cod_widget::CodWidgetCreate {
        name: "Untouched".to_string(),
    }];
    let update_arg: Vec<(Uuid, cod_widget::CodWidgetUpdate)> = Vec::new();
    assert_eq!(create_arg.len(), 1);
    assert_eq!(create_arg[0].name, "Untouched");
    assert!(update_arg.is_empty());
}

// ---------------------------------------------------------------------------
// delete_many + perform_delete_many  (DELETE /batch)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn delete_many_runs_perform_delete_many_and_removes_rows() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    let a = create_widget(&app, "Valve").await;
    let b = create_widget(&app, "Washer").await;
    let kept = create_widget(&app, "Yoke").await;
    let a_id = a["id"].as_str().unwrap().to_string();
    let b_id = b["id"].as_str().unwrap().to_string();
    let kept_id = kept["id"].as_str().unwrap();

    let ids = json!([a_id, b_id]);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/widgets/batch")
                .header("content-type", "application/json")
                .body(Body::from(ids.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Default (secure) profile hides the ids and reports only the count.
    let deleted = body_json(resp).await;
    assert_eq!(deleted["deleted"], 2);

    // Deleted ids are gone, the untouched one survives.
    for gone in [&a_id, &b_id] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/widgets/{gone}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/widgets/{kept_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ---------------------------------------------------------------------------
// perform_delete_many de-duplicates input ids and ignores absent ones. The
// de-duplicated id list is only observable when `expose_deleted_ids` is true,
// so this drives the router under the legacy security profile.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn delete_many_reports_only_ids_that_existed() {
    let db = setup_test_db().await.unwrap();
    // Single-row inserts go through the unscoped default router; the legacy
    // router is used only for the batch delete so its response exposes the ids.
    let app = app(&db);
    let legacy = app_legacy(&db);

    let only = create_widget(&app, "Zinc").await;
    let only_id = only["id"].as_str().unwrap().to_string();
    let absent = Uuid::new_v4().to_string();

    // Mix a real id, an absent id, and a duplicate of the real id.
    let ids = json!([only_id, absent, only_id]);

    let resp = legacy
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/widgets/batch")
                .header("content-type", "application/json")
                .body(Body::from(ids.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let deleted = body_json(resp).await;
    let arr = deleted.as_array().unwrap();
    // perform_delete_many returns existing ids de-duplicated: just the one row,
    // with the absent id and the duplicate dropped.
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0].as_str().unwrap(), only_id);
}
