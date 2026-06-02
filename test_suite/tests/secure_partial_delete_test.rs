//! Regression coverage for partial batch-delete (`DELETE /batch?partial=true`) under the
//! default secure `SecurityProfile`.
//!
//! Scenario: a partial batch-delete mixes existing IDs with a non-existent one. Under the
//! secure profile (`expose_deleted_ids = false`, the default) the `succeeded` ID array is
//! redacted to a `succeeded_count` scalar so the response no longer enumerates which submitted
//! IDs existed. We contrast that against the legacy profile, which returns the full
//! `BatchResult` with a populated `succeeded`/`failed` array.
//!
//! ACTUAL-vs-SPEC NOTE (A6, current branch): the redaction is partial. The secure response
//! shape is `{ "succeeded_count": N, "failed": [{ index, error }] }`. Two points diverge from
//! a "no existence leak" expectation:
//!   1. There is no `failed_count` scalar — failures are returned as the full `failed` array.
//!   2. Each `failed[].error` is the per-item not-found message, which embeds the submitted
//!      non-existent UUID (e.g. `"spd_items with ID '<uuid>' not found"`). So the secure
//!      profile still leaks row existence for the FAILED ids via the error strings, even
//!      though the SUCCEEDED ids are now redacted to a count.
//!
//! The secure test below asserts this actual behaviour rather than the (not-yet-true) spec.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels, SecurityProfile};
use sea_orm::entity::prelude::*;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use tower::ServiceExt;
use uuid::Uuid;

pub mod spd_item {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "spd_items")]
    #[crudcrate(generate_router, api_struct = "SpdItem")]
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
    let db = Database::connect("sqlite::memory:").await?;
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);
    db.execute(backend.build(&schema.create_table_from_entity(spd_item::Entity)))
        .await?;
    Ok(db)
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/items", spd_item::SpdItem::router(db).into())
}

async fn create_item(db: &DatabaseConnection, name: &str) -> String {
    let response = app(db)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/items")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::json!({ "name": name }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    json["id"].as_str().unwrap().to_string()
}

/// Under the default secure profile a partial batch-delete with a non-existent ID returns
/// 207 Multi-Status. The `succeeded` ID array is redacted to a `succeeded_count` scalar.
///
/// Documents ACTUAL behaviour: the redaction does not extend to the `failed` array, whose
/// per-item error messages still embed the submitted non-existent UUID. See the module-level
/// note (A6).
#[tokio::test]
async fn secure_partial_delete_redacts_succeeded_ids() {
    let db = setup_test_db().await.expect("setup db");

    let real_id_1 = create_item(&db, "first").await;
    let real_id_2 = create_item(&db, "second").await;
    let ghost_id = Uuid::new_v4().to_string();

    // secure() is the default; layered explicitly here for clarity.
    let response = app(&db)
        .layer(axum::Extension(SecurityProfile::secure()))
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/items/batch?partial=true")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!([real_id_1, ghost_id, real_id_2]).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::MULTI_STATUS,
        "mixed existing/non-existing partial delete should be 207"
    );

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let raw = String::from_utf8(bytes.to_vec()).unwrap();
    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();

    // The succeeded IDs are redacted to a count, and no raw `succeeded` ID array is present.
    assert!(
        json.get("succeeded_count").is_some(),
        "secure response should expose succeeded_count, got {raw}"
    );
    assert!(
        json.get("succeeded").is_none(),
        "secure response must not expose the succeeded ID array, got {raw}"
    );
    assert_eq!(
        json["succeeded_count"].as_u64().unwrap(),
        2,
        "two real IDs should be deleted"
    );

    // A6 fix: under the secure profile the partial-delete response collapses failures to a
    // `failed_count` scalar — it must NOT expose a `failed` array, and must NOT leak the
    // submitted non-existent UUID (which the per-item not-found message would otherwise echo,
    // re-creating the existence-enumeration oracle that expose_deleted_ids=false closes).
    assert!(
        json.get("failed").is_none(),
        "secure response must not expose the failed array, got {raw}"
    );
    assert_eq!(
        json["failed_count"].as_u64().unwrap(),
        1,
        "the single non-existent ID is the only failure"
    );
    assert!(
        !raw.contains(&ghost_id),
        "secure profile must NOT leak the non-existent UUID anywhere in the response, got {raw}"
    );
}

/// Contrast: under the legacy profile the same request returns the full `BatchResult`, whose
/// `failed` array exposes per-item indices and error messages.
#[tokio::test]
async fn legacy_partial_delete_exposes_failed_entries() {
    let db = setup_test_db().await.expect("setup db");

    let real_id_1 = create_item(&db, "first").await;
    let real_id_2 = create_item(&db, "second").await;
    let ghost_id = Uuid::new_v4().to_string();

    let response = app(&db)
        .layer(axum::Extension(SecurityProfile::legacy()))
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/items/batch?partial=true")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!([real_id_1, ghost_id, real_id_2]).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::MULTI_STATUS,
        "mixed existing/non-existing partial delete should be 207"
    );

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    let succeeded = json["succeeded"]
        .as_array()
        .expect("legacy response should expose a succeeded array");
    assert_eq!(succeeded.len(), 2, "two real IDs should be deleted");

    let failed = json["failed"]
        .as_array()
        .expect("legacy response should expose a failed array");
    assert_eq!(failed.len(), 1, "one failed entry expected");
    assert_eq!(
        failed[0]["index"].as_u64().unwrap(),
        1,
        "the non-existent ID was at index 1 of the request"
    );
    assert!(
        failed[0]["error"].is_string(),
        "legacy failed entry should carry an error message"
    );
}
