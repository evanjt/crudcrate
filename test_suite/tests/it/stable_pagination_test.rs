//! Pagination is stable across pages: ties break on the primary key, no row repeats or is skipped.

// Stable pagination test
// A sort column with duplicate values must still produce a total order, so paging
// with LIMIT/OFFSET cannot repeat or skip a row between pages.

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::{DatabaseConnection, entity::prelude::*};
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "tickets")]
#[crudcrate(
    api_struct = "Ticket",
    name_singular = "ticket",
    name_plural = "tickets",
    generate_router
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(sortable, filterable)]
    pub queue: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

async fn setup_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(Entity).await
}

async fn get_ids(app: &Router, uri: &str) -> Vec<String> {
    let response = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let items: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    items
        .into_iter()
        .map(|item| item["id"].as_str().unwrap().to_string())
        .collect()
}

async fn seed(app: &Router, count: usize) {
    for _ in 0..count {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/tickets")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "queue": "support" }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 201);
    }
}

#[tokio::test]
async fn test_ties_are_ordered_by_primary_key() {
    let db = setup_db().await.expect("database setup");
    let app = Router::new().nest("/tickets", Ticket::router(&db).into());
    seed(&app, 10).await;

    let ids = get_ids(
        &app,
        "/tickets?sort=%5B%22queue%22%2C%22ASC%22%5D&range=%5B0%2C9%5D",
    )
    .await;

    let mut expected = ids.clone();
    expected.sort();
    assert_eq!(ids, expected, "rows tied on queue must be ordered by id");
}

#[tokio::test]
async fn test_pages_do_not_repeat_or_skip_rows() {
    let db = setup_db().await.expect("database setup");
    let app = Router::new().nest("/tickets", Ticket::router(&db).into());
    seed(&app, 10).await;

    let sort = "sort=%5B%22queue%22%2C%22ASC%22%5D";
    let page_one = get_ids(&app, &format!("/tickets?{sort}&page=1&per_page=5")).await;
    let page_two = get_ids(&app, &format!("/tickets?{sort}&page=2&per_page=5")).await;

    assert_eq!(page_one.len(), 5);
    assert_eq!(page_two.len(), 5);

    let mut combined = [page_one, page_two].concat();
    combined.sort();
    combined.dedup();
    assert_eq!(combined.len(), 10, "every row appears exactly once");
}

#[tokio::test]
async fn test_zero_page_size_returns_a_row() {
    let db = setup_db().await.expect("database setup");
    let app = Router::new().nest("/tickets", Ticket::router(&db).into());
    seed(&app, 3).await;

    let ids = get_ids(&app, "/tickets?page=1&per_page=0").await;
    assert_eq!(ids.len(), 1, "a zero page size clamps to one row, not none");
}
