// HTTP-driven coverage for crudcrate/src/filtering/sort.rs:
// parse_sorting / parse_sorting_with_joins / find_column.
//
// Self-contained: a single `shc_widgets` entity with a Uuid PK, a sortable+filterable
// `name` String, and a sortable integer `rank`. Every test drives the generated HTTP
// router so the real sort parsing/column-resolution code runs.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr};
use tower::ServiceExt;
use uuid::Uuid;

pub mod widget {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "shc_widgets")]
    #[crudcrate(generate_router, api_struct = "ShcWidget")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[crudcrate(sortable)]
        pub rank: i32,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

use widget::{ShcWidget, ShcWidgetList};

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(widget::Entity).await
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/widgets", ShcWidget::router(db).into())
}

/// Insert a fixed set of rows with known name/rank ordering.
/// name (alphabetical) and rank (numeric) deliberately disagree so each
/// sort column produces a distinct, checkable ordering.
async fn seed(db: &DatabaseConnection) {
    use sea_orm::{ActiveValue::Set, EntityTrait};

    // (name, rank): name asc -> Apple, Cherry, Mango, Zebra
    //               rank asc -> Mango(1), Zebra(2), Apple(3), Cherry(4)
    let rows = [("Zebra", 2), ("Apple", 3), ("Mango", 1), ("Cherry", 4)];
    for (name, rank) in rows {
        let am = widget::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(name.to_string()),
            rank: Set(rank),
        };
        widget::Entity::insert(am).exec(db).await.unwrap();
    }
}

fn encode(filter: &serde_json::Value) -> String {
    percent_encoding::utf8_percent_encode(&filter.to_string(), percent_encoding::NON_ALPHANUMERIC)
        .to_string()
}

async fn list(app: axum::Router, uri: &str) -> (StatusCode, Vec<ShcWidgetList>) {
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
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        status,
        StatusCode::OK,
        "non-OK status {status}: {}",
        String::from_utf8_lossy(&bytes)
    );
    let parsed: Vec<ShcWidgetList> = serde_json::from_slice(&bytes).unwrap();
    (status, parsed)
}

// =============================================================================
// 1. JSON-array sort on the String column (React Admin format) ASC + DESC.
// =============================================================================

#[tokio::test]
async fn json_array_sort_name_asc_and_desc() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let (status, rows) = list(app(&db), "/widgets?sort=%5B%22name%22%2C%22ASC%22%5D").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rows.len(), 4);
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["Apple", "Cherry", "Mango", "Zebra"]);

    let (status, rows) = list(app(&db), "/widgets?sort=%5B%22name%22%2C%22DESC%22%5D").await;
    assert_eq!(status, StatusCode::OK);
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["Zebra", "Mango", "Cherry", "Apple"]);
}

// JSON array without a direction defaults to ASC (parse_json_sort partial path).
#[tokio::test]
async fn json_array_sort_name_default_direction_is_asc() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let (status, rows) = list(app(&db), "/widgets?sort=%5B%22name%22%5D").await;
    assert_eq!(status, StatusCode::OK);
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["Apple", "Cherry", "Mango", "Zebra"]);
}

// =============================================================================
// 2. JSON-array sort on the integer column orders numerically, not lexically.
// =============================================================================

#[tokio::test]
async fn json_array_sort_rank_asc_orders_by_integer() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let (status, rows) = list(app(&db), "/widgets?sort=%5B%22rank%22%2C%22ASC%22%5D").await;
    assert_eq!(status, StatusCode::OK);
    let ranks: Vec<i32> = rows.iter().map(|r| r.rank).collect();
    assert_eq!(ranks, vec![1, 2, 3, 4]);
    // Cross-check that this ordering differs from name ordering (proves the
    // integer column, not the default/name column, was used).
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["Mango", "Zebra", "Apple", "Cherry"]);

    let (status, rows) = list(app(&db), "/widgets?sort=%5B%22rank%22%2C%22DESC%22%5D").await;
    assert_eq!(status, StatusCode::OK);
    let ranks: Vec<i32> = rows.iter().map(|r| r.rank).collect();
    assert_eq!(ranks, vec![4, 3, 2, 1]);
}

// =============================================================================
// 3. Untrusted column names never reach raw SQL: unknown / injection-shaped
//    columns fall back to the default ordering, still 200, table intact.
// =============================================================================

#[tokio::test]
async fn nonexistent_sort_column_falls_back_without_error() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let (status, rows) = list(
        app(&db),
        "/widgets?sort=%5B%22nonexistent_col%22%2C%22ASC%22%5D",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // find_column returned the default column; all rows still present.
    assert_eq!(rows.len(), 4);
}

#[tokio::test]
async fn sql_injection_sort_column_is_inert_and_table_survives() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // A column name carrying a DROP TABLE payload must be treated as an
    // unknown column name (string compared in find_column), never interpolated
    // into SQL. Drives the request, then re-lists to prove the table exists.
    let malicious = serde_json::json!(["name; DROP TABLE shc_widgets", "ASC"]);
    let uri = format!("/widgets?sort={}", encode(&malicious));

    let (status, rows) = list(app(&db), &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rows.len(), 4, "fallback ordering still returns every row");

    // The table must still exist and still hold all seeded rows.
    let (status, rows) = list(app(&db), "/widgets").await;
    assert_eq!(status, StatusCode::OK, "table must survive the request");
    assert_eq!(rows.len(), 4, "no rows were dropped");
}

// A bare, non-JSON, non-bracketed sort value that is also not a real column
// must fall back (parse_sorting REST branch + find_column default).
#[tokio::test]
async fn plain_unknown_sort_value_falls_back() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let (status, rows) = list(app(&db), "/widgets?sort=not-an-array").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rows.len(), 4);
}

// =============================================================================
// 4. REST-style sort params (sort_by / order, and sort=<col>&order=<dir>).
// =============================================================================

#[tokio::test]
async fn rest_sort_by_and_order_desc() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let (status, rows) = list(app(&db), "/widgets?sort_by=name&order=DESC").await;
    assert_eq!(status, StatusCode::OK);
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["Zebra", "Mango", "Cherry", "Apple"]);
}

#[tokio::test]
async fn rest_sort_by_default_order_is_asc() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // sort_by present, order omitted -> DEFAULT_SORT_ORDER (ASC).
    let (status, rows) = list(app(&db), "/widgets?sort_by=rank").await;
    assert_eq!(status, StatusCode::OK);
    let ranks: Vec<i32> = rows.iter().map(|r| r.rank).collect();
    assert_eq!(ranks, vec![1, 2, 3, 4]);
}

#[tokio::test]
async fn rest_plain_sort_with_order_and_case_insensitive_direction() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // Plain (non-bracketed) `sort` + `order`: REST branch of parse_sorting.
    // Lowercase "asc" exercises parse_order case-insensitivity.
    let (status, rows) = list(app(&db), "/widgets?sort=rank&order=asc").await;
    assert_eq!(status, StatusCode::OK);
    let ranks: Vec<i32> = rows.iter().map(|r| r.rank).collect();
    assert_eq!(ranks, vec![1, 2, 3, 4]);

    let (status, rows) = list(app(&db), "/widgets?sort=rank&order=desc").await;
    assert_eq!(status, StatusCode::OK);
    let ranks: Vec<i32> = rows.iter().map(|r| r.rank).collect();
    assert_eq!(ranks, vec![4, 3, 2, 1]);
}

// sort_by takes priority over sort when both are present (first branch of
// parse_sorting). Here sort_by=name should win over sort=["rank","ASC"].
#[tokio::test]
async fn rest_sort_by_takes_priority_over_sort() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let sort_json = encode(&serde_json::json!(["rank", "ASC"]));
    let uri = format!("/widgets?sort_by=name&order=ASC&sort={sort_json}");
    let (status, rows) = list(app(&db), &uri).await;
    assert_eq!(status, StatusCode::OK);
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["Apple", "Cherry", "Mango", "Zebra"],
        "sort_by=name must win over sort=[rank,...]"
    );
}

// =============================================================================
// 5. No sort param at all -> default ordering, still 200 with all rows.
// =============================================================================

#[tokio::test]
async fn no_sort_param_uses_default_ordering() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let (status, rows) = list(app(&db), "/widgets").await;
    assert_eq!(status, StatusCode::OK, "default sort must not error");
    assert_eq!(rows.len(), 4);
}

// An invalid (non-uppercase, non-ASC) direction defaults to DESC per parse_order.
#[tokio::test]
async fn invalid_direction_defaults_to_desc() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let sort_json = encode(&serde_json::json!(["rank", "sideways"]));
    let uri = format!("/widgets?sort={sort_json}");
    let (status, rows) = list(app(&db), &uri).await;
    assert_eq!(status, StatusCode::OK);
    let ranks: Vec<i32> = rows.iter().map(|r| r.rank).collect();
    assert_eq!(
        ranks,
        vec![4, 3, 2, 1],
        "any non-ASC direction is treated as DESC"
    );
}
