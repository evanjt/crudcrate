// HTTP coverage for pagination over the generated list endpoint.
//
// Exercises crudcrate::filtering::pagination::calculate_content_range and
// crudcrate::filtering::conditions::parse_pagination end-to-end by driving the
// generated GET list handler and inspecting both the body length and the
// Content-Range response header.
//
// Self-contained: the entity, schema, seeding, and router live in this file.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use tower::ServiceExt;
use uuid::Uuid;

pub mod widget {
    use super::*;

    // max_page_size is set below the seeded row count so the "huge range" case
    // can demonstrate the limit cap rather than just being bounded by the data.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "pcr_widgets")]
    #[crudcrate(
        generate_router,
        api_struct = "PcrWidget",
        name_plural = "pcr_widgets",
        max_page_size = 20
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[crudcrate(filterable, sortable)]
        pub position: i32,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

const SEED_COUNT: i32 = 25;

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let db = Database::connect(&url).await?;
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    // Persistent backends (Postgres/MySQL) keep tables across tests within a binary;
    // drop first so every test starts from a clean schema and empty data. On
    // sqlite::memory: each connection is a fresh database, so the drops are no-ops.
    db.execute(&Table::drop().table(widget::Entity).if_exists().to_owned())
        .await?;
    db.execute(&schema.create_table_from_entity(widget::Entity))
        .await?;
    Ok(db)
}

/// Seed `SEED_COUNT` rows with deterministic `position` `0..SEED_COUNT` so that
/// sorting by position yields a predictable ordering across pages.
async fn seed(db: &DatabaseConnection) {
    let items: Vec<widget::PcrWidgetCreate> = (0..SEED_COUNT)
        .map(|i| widget::PcrWidgetCreate {
            name: format!("widget-{i:02}"),
            position: i,
        })
        .collect();
    widget::PcrWidget::create_many(db, items)
        .await
        .expect("seed create_many should succeed");
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/widgets", widget::PcrWidget::router(db).into())
}

/// Issue a GET against the list endpoint with the given query string and return
/// the status, the Content-Range header value, and the parsed JSON body.
async fn list(
    db: &DatabaseConnection,
    query: &str,
) -> (StatusCode, Option<String>, serde_json::Value) {
    let uri = format!("/widgets?{query}");
    let resp = app(db)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let content_range = resp
        .headers()
        .get("content-range")
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    (status, content_range, json)
}

fn body_len(json: &serde_json::Value) -> usize {
    json.as_array()
        .expect("list body should be a JSON array")
        .len()
}

#[tokio::test]
async fn first_range_returns_first_ten_with_header() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // range=[0,9] with an explicit ascending sort on position so the slice is
    // the first ten rows.
    let (status, content_range, json) = list(
        &db,
        "range=%5B0%2C9%5D&sort=%5B%22position%22%2C%22ASC%22%5D",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body_len(&json), 10, "range=[0,9] should yield 10 rows");
    assert_eq!(
        content_range.as_deref(),
        Some("pcr_widgets 0-9/25"),
        "Content-Range must report offset 0-9 of total 25"
    );

    let positions: Vec<i64> = json
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["position"].as_i64().unwrap())
        .collect();
    assert_eq!(positions, (0..10).collect::<Vec<_>>());
}

#[tokio::test]
async fn second_range_offsets_header_and_slice() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // range=[10,19] is the next ten rows; the header offset must advance to 10.
    let (status, content_range, json) = list(
        &db,
        "range=%5B10%2C19%5D&sort=%5B%22position%22%2C%22ASC%22%5D",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body_len(&json), 10, "range=[10,19] should yield 10 rows");
    assert_eq!(content_range.as_deref(), Some("pcr_widgets 10-19/25"));

    let positions: Vec<i64> = json
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["position"].as_i64().unwrap())
        .collect();
    assert_eq!(positions, (10..20).collect::<Vec<_>>());
}

#[tokio::test]
async fn page_and_per_page_paginate() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // page=2&per_page=5 => offset 5, limit 5 (rows at positions 5..10).
    let (status, content_range, json) = list(
        &db,
        "page=2&per_page=5&sort=%5B%22position%22%2C%22ASC%22%5D",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body_len(&json), 5, "page 2 of size 5 should yield 5 rows");
    assert_eq!(content_range.as_deref(), Some("pcr_widgets 5-9/25"));

    let positions: Vec<i64> = json
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["position"].as_i64().unwrap())
        .collect();
    assert_eq!(positions, (5..10).collect::<Vec<_>>());
}

#[tokio::test]
async fn huge_range_end_does_not_panic_and_caps_to_max_page_size() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // range=[0, u64::MAX]: parse_pagination uses saturating_add for the `end + 1`
    // limit computation, so this must produce a valid bounded limit rather than
    // overflowing. The limit is then capped at the resource max_page_size (20),
    // not the 25 seeded rows.
    let (status, content_range, json) = list(
        &db,
        "range=%5B0%2C18446744073709551615%5D&sort=%5B%22position%22%2C%22ASC%22%5D",
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "huge range end must return 200, not 500/panic"
    );
    assert_eq!(
        body_len(&json),
        20,
        "limit must be capped at max_page_size (20), even though 25 rows exist"
    );
    assert_eq!(
        content_range.as_deref(),
        Some("pcr_widgets 0-19/25"),
        "Content-Range end reflects the capped page, total still 25"
    );
}

#[tokio::test]
async fn out_of_range_offset_returns_empty_with_sane_header() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // range=[100,109]: offset 100 is past the 25 rows. The handler must return
    // 200 with an empty array, and calculate_content_range collapses the range
    // to start==end (100-100) rather than producing start>end.
    let (status, content_range, json) = list(
        &db,
        "range=%5B100%2C109%5D&sort=%5B%22position%22%2C%22ASC%22%5D",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body_len(&json), 0, "offset beyond total yields no rows");

    let header = content_range.expect("Content-Range header must be present");
    assert_eq!(
        header, "pcr_widgets 100-100/25",
        "out-of-range offset collapses to start==end with total 25"
    );

    // Defensive parse: start must never exceed end.
    let range_part = header.split(' ').nth(1).unwrap();
    let nums: Vec<u64> = range_part
        .split('/')
        .next()
        .unwrap()
        .split('-')
        .map(|s| s.parse().unwrap())
        .collect();
    assert!(
        nums[0] <= nums[1],
        "range start must not exceed end: {header}"
    );
}
