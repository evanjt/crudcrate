//! Batch create over HTTP: response order, optional fields, and rollback on a unique-constraint violation.

// Scenario: the default `create_many` uses a single multi-row INSERT with
// RETURNING on backends that support it (Postgres, SQLite >= 3.35) and a
// per-row insert loop on backends that don't (MySQL).
// Expected behaviour: identical to the previous per-row loop on every backend.
// Response order matches input order, the whole batch rolls back on any row
// failure, a duplicate key inside the batch maps to 409 CONFLICT, and
// server-generated values (`on_create` id, DB round-tripped fields) are
// present on every returned item.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr, PaginatorTrait};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

pub mod bcr_item {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "bcr_items")]
    #[crudcrate(generate_router, api_struct = "BcrItem")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[sea_orm(unique)]
        #[crudcrate(filterable, sortable)]
        pub label: String,

        #[crudcrate(filterable, sortable)]
        pub position: i32,

        pub note: Option<String>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let db = test_suite::reset_db!(bcr_item::Entity).await?;
    db.execute_unprepared("CREATE UNIQUE INDEX bcr_items_label_unique ON bcr_items (label)")
        .await?;
    Ok(db)
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/items", bcr_item::BcrItem::router(db).into())
}

async fn post_batch(db: &DatabaseConnection, body: Value) -> axum::http::Response<Body> {
    app(db)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/items/batch")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn batch_create_preserves_input_order() {
    let db = setup_test_db().await.expect("setup failed");

    let items: Vec<Value> = (0..50)
        .map(|i| json!({ "label": format!("item_{i:03}"), "position": i }))
        .collect();
    let response = post_batch(&db, json!(items)).await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let created: Vec<Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(created.len(), 50);
    for (i, item) in created.iter().enumerate() {
        assert_eq!(
            item["label"],
            format!("item_{i:03}"),
            "response order must match input order at index {i}"
        );
        assert_eq!(item["position"], i64::try_from(i).unwrap());
        assert!(
            Uuid::parse_str(item["id"].as_str().unwrap()).is_ok(),
            "server-generated id must be present"
        );
    }
}

// Scenario: rows in one batch differ in which optional fields carry values.
// Expected behaviour: the multi-row VALUES insert binds NULL for the absent
// ones and every row round-trips its own value.
#[tokio::test]
async fn batch_create_mixed_optional_fields() {
    let db = setup_test_db().await.expect("setup failed");

    let response = post_batch(
        &db,
        json!([
            { "label": "with_note", "position": 0, "note": "hello" },
            { "label": "without_note", "position": 1 },
            { "label": "null_note", "position": 2, "note": null },
        ]),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let created: Vec<Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(created.len(), 3);
    assert_eq!(created[0]["note"], "hello");
    assert_eq!(created[1]["note"], Value::Null);
    assert_eq!(created[2]["note"], Value::Null);
}

#[tokio::test]
async fn batch_create_rolls_back_on_duplicate() {
    let db = setup_test_db().await.expect("setup failed");

    let response = post_batch(
        &db,
        json!([
            { "label": "unique_a", "position": 0 },
            { "label": "dup", "position": 1 },
            { "label": "dup", "position": 2 },
        ]),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::CONFLICT,
        "duplicate key inside a batch must map to 409"
    );

    let count = bcr_item::Entity::find().count(&db).await.unwrap();
    assert_eq!(count, 0, "failed batch must not leave partial rows");
}

#[tokio::test]
async fn batch_create_conflicts_with_existing_row() {
    let db = setup_test_db().await.expect("setup failed");

    let seed = post_batch(&db, json!([{ "label": "taken", "position": 0 }])).await;
    assert_eq!(seed.status(), StatusCode::CREATED);

    let response = post_batch(
        &db,
        json!([
            { "label": "fresh", "position": 1 },
            { "label": "taken", "position": 2 },
        ]),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let count = bcr_item::Entity::find().count(&db).await.unwrap();
    assert_eq!(count, 1, "only the seeded row should remain");
}

#[tokio::test]
async fn batch_create_empty_input_returns_empty() {
    let db = setup_test_db().await.expect("setup failed");

    let response = post_batch(&db, json!([])).await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let created: Vec<Value> = serde_json::from_slice(&body).unwrap();
    assert!(created.is_empty());
}
