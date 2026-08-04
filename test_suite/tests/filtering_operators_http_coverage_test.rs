//! End-to-end coverage of `crudcrate/src/filtering/conditions.rs` driven entirely
//! through the generated HTTP router with `?filter={...}` query parameters.
//!
//! Each test exercises a distinct branch of `apply_filters` / `parse_filter_json`:
//! - string equality (`process_string_filter`)
//! - integer comparison suffixes `_gte/_lte/_gt/_lt/_neq` (`parse_comparison_operator` +
//!   `process_number_filter` + `apply_numeric_comparison`)
//! - array `IN` (`process_array_filter`)
//! - boolean equality
//! - the `MAX_FILTER_CLAUSES` guard (101 keys -> 400, 100 keys -> 200)
//! - the joined-filter whitelist guard: a dot-notation key on a model with no
//!   joined filterable columns is silently skipped (200, results unaffected).

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use tower::ServiceExt;
use uuid::Uuid;

pub mod foh_record {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "foh_records")]
    #[crudcrate(generate_router, api_struct = "FohRecord")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

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
    // drop first so every test starts from a clean schema and empty data. On
    // sqlite::memory: each connection is a fresh database, so the drops are no-ops.
    db.execute(&Table::drop()
                .table(foh_record::Entity)
                .if_exists()
                .to_owned())
    .await?;
    db.execute(&schema.create_table_from_entity(foh_record::Entity))
        .await?;
    Ok(db)
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/records", foh_record::FohRecord::router(db).into())
}

/// Seed a fixed set of rows so each filter selects a meaningful subset.
///
/// | name  | score | active |
/// |-------|-------|--------|
/// | alpha |   10  | true   |
/// | beta  |   20  | false  |
/// | gamma |   30  | true   |
/// | delta |   40  | false  |
async fn seed(db: &DatabaseConnection) {
    let rows = [
        ("alpha", 10, true),
        ("beta", 20, false),
        ("gamma", 30, true),
        ("delta", 40, false),
    ];
    for (name, score, active) in rows {
        let create = foh_record::FohRecordCreate {
            name: name.to_string(),
            score,
            active,
        };
        foh_record::FohRecord::create(db, create)
            .await
            .expect("seed row");
    }
}

fn filter_uri(filter_json: &str) -> String {
    // per_page=1000 keeps every seeded row in a single page so the assertions
    // reflect the filter, not pagination.
    format!(
        "/records?page=1&per_page=1000&filter={}",
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
    // A 400 returns a non-array error body; only parse JSON, callers branch on status.
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

// ============================================================================
// 1. String equality
// ============================================================================

#[tokio::test]
async fn equality_filter_selects_single_row() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let (status, body) = get_filtered(&db, r#"{"name":"alpha"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), vec!["alpha".to_string()]);
}

// ============================================================================
// 2. Integer comparison suffixes
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

    // Two clauses on the same base field: 20 <= score <= 30 -> beta, gamma.
    let (status, body) = get_filtered(&db, r#"{"score_gte":20,"score_lte":30}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), vec!["beta".to_string(), "gamma".to_string()]);
}

// ============================================================================
// 3. Array IN
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
// 4. Boolean equality
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
// 5. MAX_FILTER_CLAUSES guard
// ============================================================================

#[tokio::test]
async fn filter_with_101_keys_is_rejected_400() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // 101 distinct keys (none of which need to be real columns) exceeds
    // MAX_FILTER_CLAUSES (100) and must be rejected before any column lookup.
    let entries: Vec<String> = (0..101).map(|i| format!("\"k{i}\":{i}")).collect();
    let filter_json = format!("{{{}}}", entries.join(","));

    let (status, _body) = get_filtered(&db, &filter_json).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "101-key filter must return 400, not silently drop filters"
    );
}

#[tokio::test]
async fn filter_with_100_keys_is_accepted_200() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // Exactly 100 keys sits at the limit and is accepted. The keys reference
    // non-existent columns, so every clause is skipped at column lookup and the
    // full result set comes back unfiltered.
    let entries: Vec<String> = (0..100).map(|i| format!("\"k{i}\":{i}")).collect();
    let filter_json = format!("{{{}}}", entries.join(","));

    let (status, body) = get_filtered(&db, &filter_json).await;
    assert_eq!(status, StatusCode::OK, "100-key filter sits at the limit");
    assert_eq!(
        names(&body),
        vec![
            "alpha".to_string(),
            "beta".to_string(),
            "delta".to_string(),
            "gamma".to_string()
        ],
        "unknown-column clauses are skipped, so all rows remain"
    );
}

// ============================================================================
// 6. Joined-filter whitelist guard (model has no joined filterable columns)
// ============================================================================

#[tokio::test]
async fn dot_notation_filter_is_silently_skipped() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // This model declares no join(..., filterable(...)) columns, so a
    // dot-notation key fails the whitelist check and is skipped silently:
    // no error, and the result set is unaffected.
    let (status, body) = get_filtered(&db, r#"{"foo.bar":"x"}"#).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "unknown joined filter must not error"
    );
    assert_eq!(
        names(&body),
        vec![
            "alpha".to_string(),
            "beta".to_string(),
            "delta".to_string(),
            "gamma".to_string()
        ],
        "skipped joined filter must not change results"
    );
}

#[tokio::test]
async fn dot_notation_filter_does_not_leak_via_combination() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // A valid main-entity clause combined with an unknown dot-notation clause:
    // the valid clause still applies, the dot-notation clause is dropped.
    let (status, body) = get_filtered(&db, r#"{"name":"alpha","foo.bar":"x"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), vec!["alpha".to_string()]);
}
