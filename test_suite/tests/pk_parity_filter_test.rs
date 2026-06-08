//! Filtering parity for a non-UUID (auto-increment `i32`) primary key.
//!
//! Mirrors `filtering_operators_http_coverage_test.rs` (a UUID-keyed model) clause
//! for clause, proving an `i32`-keyed entity produces identical statuses and
//! response shapes through the generated `?filter={...}` HTTP surface:
//!   - string equality on `name`
//!   - integer comparison suffixes `_gte/_lte/_gt/_lt/_neq` on `score`
//!   - array `IN` on `name`
//!   - boolean equality on `active`
//!
//! It then adds the dimension a UUID model cannot express: filtering by the
//! INTEGER primary key itself (`{"id":2}` equality and `{"id":[1,3]}` array IN),
//! confirming an integer-id filter selects exactly the right row(s).

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use tower::ServiceExt;

pub mod ppf_widget {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "ppf_widgets")]
    #[crudcrate(generate_router, api_struct = "PpfWidget", derive_partial_eq)]
    pub struct Model {
        // Auto-increment i32 PK assigned by the DB (no on_create). Marked
        // `filterable` so `?filter={"id":...}` resolves against the PK column,
        // and excluded from create/update like any generated id.
        #[sea_orm(primary_key)]
        #[crudcrate(primary_key, filterable, sortable, exclude(create, update))]
        pub id: i32,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[crudcrate(filterable, sortable)]
        pub score: i32,

        #[crudcrate(filterable)]
        pub active: bool,
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
    // drop first so every test starts from a clean schema. On sqlite::memory: each
    // connection is a fresh database, so the drop is a harmless no-op.
    db.execute(
        backend.build(
            &Table::drop()
                .table(ppf_widget::Entity)
                .if_exists()
                .to_owned(),
        ),
    )
    .await?;
    db.execute(backend.build(&schema.create_table_from_entity(ppf_widget::Entity)))
        .await?;
    Ok(db)
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/widgets", ppf_widget::PpfWidget::router(db).into())
}

/// Seed a fixed set of rows so each filter selects a meaningful subset.
///
/// Inserted in order, so the auto-increment integer PK is deterministic:
///
/// | id | name  | score | active |
/// |----|-------|-------|--------|
/// |  1 | alpha |   10  | true   |
/// |  2 | beta  |   20  | false  |
/// |  3 | gamma |   30  | true   |
/// |  4 | delta |   40  | false  |
async fn seed(db: &DatabaseConnection) {
    let rows = [
        ("alpha", 10, true),
        ("beta", 20, false),
        ("gamma", 30, true),
        ("delta", 40, false),
    ];
    for (name, score, active) in rows {
        let create = ppf_widget::PpfWidgetCreate {
            name: name.to_string(),
            score,
            active,
        };
        ppf_widget::PpfWidget::create(db, create)
            .await
            .expect("seed row");
    }
}

fn filter_uri(filter_json: &str) -> String {
    // per_page=1000 keeps every seeded row on one page so assertions reflect the
    // filter, not pagination.
    format!(
        "/widgets?page=1&per_page=1000&filter={}",
        percent_encoding::utf8_percent_encode(filter_json, percent_encoding::NON_ALPHANUMERIC)
    )
}

async fn get_filtered(
    db: &DatabaseConnection,
    filter_json: &str,
) -> (StatusCode, serde_json::Value) {
    let resp = app(db)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(filter_uri(filter_json))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

fn names(list: &serde_json::Value) -> Vec<String> {
    let mut out: Vec<String> = list
        .as_array()
        .expect("list endpoint returns a JSON array")
        .iter()
        .map(|r| r["name"].as_str().expect("name field").to_string())
        .collect();
    out.sort();
    out
}

fn ids(list: &serde_json::Value) -> Vec<i64> {
    let mut out: Vec<i64> = list
        .as_array()
        .expect("list endpoint returns a JSON array")
        .iter()
        .map(|r| {
            assert!(
                r["id"].is_i64() || r["id"].is_u64(),
                "id must serialise as an integer, got {:?}",
                r["id"]
            );
            r["id"].as_i64().expect("integer id")
        })
        .collect();
    out.sort_unstable();
    out
}

// ============================================================================
// 1. String equality  (mirrors `equality_filter_selects_single_row`)
// ============================================================================

#[tokio::test]
async fn equality_filter_selects_single_row() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let (status, body) = get_filtered(&db, r#"{"name":"alpha"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), vec!["alpha".to_string()]);
    assert_eq!(ids(&body), vec![1]);
}

// ============================================================================
// 2. Integer comparison suffixes on `score`
//    (mirrors score_gte/lte/gt/lt/neq + combined range)
// ============================================================================

#[tokio::test]
async fn score_gte_includes_boundary() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // score >= 30 -> gamma(30), delta(40)
    let (status, body) = get_filtered(&db, r#"{"score_gte":30}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), vec!["delta".to_string(), "gamma".to_string()]);
}

#[tokio::test]
async fn score_lte_includes_boundary() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // score <= 20 -> alpha(10), beta(20)
    let (status, body) = get_filtered(&db, r#"{"score_lte":20}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), vec!["alpha".to_string(), "beta".to_string()]);
}

#[tokio::test]
async fn score_gt_excludes_boundary() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // score > 30 -> delta(40) only; gamma(30) excluded
    let (status, body) = get_filtered(&db, r#"{"score_gt":30}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), vec!["delta".to_string()]);
}

#[tokio::test]
async fn score_lt_excludes_boundary() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // score < 20 -> alpha(10) only; beta(20) excluded
    let (status, body) = get_filtered(&db, r#"{"score_lt":20}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), vec!["alpha".to_string()]);
}

#[tokio::test]
async fn score_neq_excludes_matching_value() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // score != 10 -> everything except alpha
    let (status, body) = get_filtered(&db, r#"{"score_neq":10}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        names(&body),
        vec!["beta".to_string(), "delta".to_string(), "gamma".to_string()]
    );
}

#[tokio::test]
async fn combined_range_brackets_a_window() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // 20 <= score <= 30 -> beta, gamma
    let (status, body) = get_filtered(&db, r#"{"score_gte":20,"score_lte":30}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), vec!["beta".to_string(), "gamma".to_string()]);
}

// ============================================================================
// 3. Array IN on `name`  (mirrors array_filter_matches_set_membership)
// ============================================================================

#[tokio::test]
async fn array_filter_matches_set_membership() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let (status, body) = get_filtered(&db, r#"{"name":["alpha","beta"]}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), vec!["alpha".to_string(), "beta".to_string()]);
}

#[tokio::test]
async fn array_filter_with_no_matches_returns_empty() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let (status, body) = get_filtered(&db, r#"{"name":["zzz","qqq"]}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert!(names(&body).is_empty());
}

// ============================================================================
// 4. Boolean equality  (mirrors bool_filter_true/false)
// ============================================================================

#[tokio::test]
async fn bool_filter_true_selects_active_rows() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // active = true -> alpha, gamma
    let (status, body) = get_filtered(&db, r#"{"active":true}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), vec!["alpha".to_string(), "gamma".to_string()]);
}

#[tokio::test]
async fn bool_filter_false_selects_inactive_rows() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // active = false -> beta, delta
    let (status, body) = get_filtered(&db, r#"{"active":false}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), vec!["beta".to_string(), "delta".to_string()]);
}

// ============================================================================
// 5. Filtering by the INTEGER primary key itself.
//    No UUID counterpart exists: this is the parity dimension unique to an
//    i32 PK. An integer-valued `{"id":...}` filter must resolve against the PK
//    column and select exactly the matching row(s).
// ============================================================================

#[tokio::test]
async fn id_equality_filter_selects_the_one_row() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // filter={"id":2} -> the single row whose integer PK is 2 (beta).
    let (status, body) = get_filtered(&db, r#"{"id":2}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids(&body), vec![2]);
    assert_eq!(names(&body), vec!["beta".to_string()]);
}

#[tokio::test]
async fn id_array_filter_selects_exact_subset() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // filter={"id":[1,3]} -> rows with PK 1 (alpha) and 3 (gamma); id 2/4 excluded.
    let (status, body) = get_filtered(&db, r#"{"id":[1,3]}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids(&body), vec![1, 3]);
    assert_eq!(names(&body), vec!["alpha".to_string(), "gamma".to_string()]);
}

#[tokio::test]
async fn id_equality_filter_for_absent_pk_returns_empty() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // An integer PK with no row matches selects nothing (200, empty), the same
    // empty-result shape a non-matching UUID id filter would produce.
    let (status, body) = get_filtered(&db, r#"{"id":999}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert!(ids(&body).is_empty());
}
