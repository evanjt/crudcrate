//! End-to-end HTTP tests for dot-notation joined filters.
//!
//! These tests exercise `GET /customers?filter={"vehicles.make":"BMW"}` and
//! related patterns through the actual Axum router, confirming that:
//! - joined filters on a whitelisted child column filter the parent result set,
//! - operator suffixes (`_gte`, `_lte`, `_neq`) work on joined columns,
//! - main-entity and joined filters combine as an intersection,
//! - unknown or non-whitelisted joined columns are silently dropped,
//! - Content-Range pagination headers reflect the filtered count,
//! - scope middleware still restricts the parent set.
//!
//! The matching parser-level tests live in `join_filter_sort_test.rs`; this
//! file covers the handler wiring that resolves parsed filters into sub-queries.

mod common;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt;

use common::{setup_scoped_app, setup_test_app, setup_test_db};

async fn admin_post(app: &axum::Router, path: &str, payload: Value) -> Value {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "{path} create failed");
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn admin_update(app: &axum::Router, path: &str, payload: Value) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "{path} update failed");
}

async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value, axum::http::HeaderMap) {
    let resp = app
        .clone()
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
    let headers = resp.headers().clone();
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&body).unwrap_or(Value::Null),
        headers,
    )
}

/// Seed three customers, each owning one vehicle with a distinct make/year.
/// Returns `(alice_id, bob_id, carol_id)`.
async fn seed_three_customers(app: &axum::Router) -> (String, String, String) {
    let alice = admin_post(
        app,
        "/customers",
        json!({"name": "Alice", "email": "alice@example.com"}),
    )
    .await;
    let bob = admin_post(
        app,
        "/customers",
        json!({"name": "Bob", "email": "bob@example.com"}),
    )
    .await;
    let carol = admin_post(
        app,
        "/customers",
        json!({"name": "Carol", "email": "carol@example.com"}),
    )
    .await;
    let alice_id = alice["id"].as_str().unwrap().to_string();
    let bob_id = bob["id"].as_str().unwrap().to_string();
    let carol_id = carol["id"].as_str().unwrap().to_string();

    // Alice owns a 2023 BMW
    admin_post(
        app,
        "/vehicles",
        json!({
            "customer_id": alice_id,
            "make": "BMW",
            "model": "3 Series",
            "year": 2023,
            "vin": "ALICE-BMW-001",
        }),
    )
    .await;
    // Bob owns a 2020 Toyota
    admin_post(
        app,
        "/vehicles",
        json!({
            "customer_id": bob_id,
            "make": "Toyota",
            "model": "Camry",
            "year": 2020,
            "vin": "BOB-TOYOTA-001",
        }),
    )
    .await;
    // Carol owns a 2018 Honda
    admin_post(
        app,
        "/vehicles",
        json!({
            "customer_id": carol_id,
            "make": "Honda",
            "model": "Civic",
            "year": 2018,
            "vin": "CAROL-HONDA-001",
        }),
    )
    .await;

    (alice_id, bob_id, carol_id)
}

fn filter_query(filter_json: &str) -> String {
    format!(
        "/customers?filter={}",
        percent_encoding::utf8_percent_encode(filter_json, percent_encoding::NON_ALPHANUMERIC)
    )
}

fn names(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect()
}

// ============================================================================
// 1. Simple joined filter returns only matching parents
// ============================================================================

#[tokio::test]
async fn joined_filter_by_make_returns_only_matching_customer() {
    let db = setup_test_db().await.unwrap();
    let app = setup_test_app(&db);
    let _ = seed_three_customers(&app).await;

    let (status, body, _) = get_json(&app, &filter_query(r#"{"vehicles.make":"BMW"}"#)).await;
    assert_eq!(status, StatusCode::OK);

    let got = names(&body);
    assert_eq!(got, vec!["Alice".to_string()], "got {got:?}");
}

// ============================================================================
// 2. Operator suffixes on joined columns
// ============================================================================

#[tokio::test]
async fn joined_filter_year_gte_filters_by_range() {
    let db = setup_test_db().await.unwrap();
    let app = setup_test_app(&db);
    let _ = seed_three_customers(&app).await;

    let (status, body, _) = get_json(&app, &filter_query(r#"{"vehicles.year_gte":2020}"#)).await;
    assert_eq!(status, StatusCode::OK);

    let mut got = names(&body);
    got.sort();
    assert_eq!(got, vec!["Alice".to_string(), "Bob".to_string()]);
}

#[tokio::test]
async fn joined_filter_year_lt_excludes_newer_vehicles() {
    let db = setup_test_db().await.unwrap();
    let app = setup_test_app(&db);
    let _ = seed_three_customers(&app).await;

    let (status, body, _) = get_json(&app, &filter_query(r#"{"vehicles.year_lt":2020}"#)).await;
    assert_eq!(status, StatusCode::OK);

    let got = names(&body);
    assert_eq!(got, vec!["Carol".to_string()]);
}

#[tokio::test]
async fn joined_filter_neq_excludes_matching_make() {
    let db = setup_test_db().await.unwrap();
    let app = setup_test_app(&db);
    let _ = seed_three_customers(&app).await;

    let (status, body, _) = get_json(&app, &filter_query(r#"{"vehicles.make_neq":"BMW"}"#)).await;
    assert_eq!(status, StatusCode::OK);

    let mut got = names(&body);
    got.sort();
    assert_eq!(got, vec!["Bob".to_string(), "Carol".to_string()]);
}

// ============================================================================
// 3. Main-entity + joined filters combine as AND
// ============================================================================

#[tokio::test]
async fn main_and_joined_filters_intersect() {
    let db = setup_test_db().await.unwrap();
    let app = setup_test_app(&db);
    let _ = seed_three_customers(&app).await;

    // name=Alice AND vehicles.make=BMW → Alice
    let (status, body, _) = get_json(
        &app,
        &filter_query(r#"{"name":"Alice","vehicles.make":"BMW"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), vec!["Alice".to_string()]);

    // name=Alice AND vehicles.make=Toyota → empty (Alice doesn't own a Toyota)
    let (status, body, _) = get_json(
        &app,
        &filter_query(r#"{"name":"Alice","vehicles.make":"Toyota"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body).len(), 0);
}

// ============================================================================
// 4. Multiple joined filters on the same field narrow further
// ============================================================================

#[tokio::test]
async fn multiple_joined_filters_on_same_field_intersect() {
    let db = setup_test_db().await.unwrap();
    let app = setup_test_app(&db);
    let _ = seed_three_customers(&app).await;

    // Bob owns a 2020 Toyota. Filter year_gte=2019 AND year_lte=2020 → Bob only
    // (Alice has 2023, out of range; Carol has 2018, out of range)
    let (status, body, _) = get_json(
        &app,
        &filter_query(r#"{"vehicles.year_gte":2019,"vehicles.year_lte":2020}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), vec!["Bob".to_string()]);
}

// ============================================================================
// 5. Non-whitelisted joined column is silently dropped
// ============================================================================

#[tokio::test]
async fn non_whitelisted_joined_column_is_ignored() {
    let db = setup_test_db().await.unwrap();
    let app = setup_test_app(&db);
    let _ = seed_three_customers(&app).await;

    // `vehicles.fuel_type` is NOT in Customer.vehicles filterable("make","model","year","vin")
    // The filter should be silently dropped and all 3 customers returned.
    let (status, body, _) =
        get_json(&app, &filter_query(r#"{"vehicles.fuel_type":"Gasoline"}"#)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body).len(), 3);
}

#[tokio::test]
async fn unknown_join_field_is_ignored() {
    let db = setup_test_db().await.unwrap();
    let app = setup_test_app(&db);
    let _ = seed_three_customers(&app).await;

    // `pets` doesn't exist as a join on Customer. Should not 500, should return all.
    let (status, body, _) = get_json(&app, &filter_query(r#"{"pets.name":"Rex"}"#)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body).len(), 3);
}

// ============================================================================
// 6. Content-Range header reflects the joined-filter total
// ============================================================================

#[tokio::test]
async fn content_range_reflects_joined_filter_count() {
    let db = setup_test_db().await.unwrap();
    let app = setup_test_app(&db);
    let _ = seed_three_customers(&app).await;

    let (status, _, headers) = get_json(&app, &filter_query(r#"{"vehicles.make":"BMW"}"#)).await;
    assert_eq!(status, StatusCode::OK);

    let content_range = headers
        .get("Content-Range")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    // Only 1 match (Alice) → total=1
    assert!(
        content_range.ends_with("/1"),
        "expected Content-Range to end with /1, got {content_range}"
    );
}

// ============================================================================
// 7. Joined filter intersects with scope middleware
// ============================================================================

#[tokio::test]
async fn joined_filter_respects_parent_scope_middleware() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    let (alice_id, _, _) = seed_three_customers(&admin).await;

    // Mark Alice (BMW owner) as private via admin. Scoped caller should no
    // longer see her even when filtering by vehicles.make=BMW. The parent
    // scope condition is ANDed with the joined-filter's IN clause.
    admin_update(
        &admin,
        &format!("/customers/{alice_id}"),
        json!({"is_private": true}),
    )
    .await;

    let (status, body, _) = get_json(&scoped, &filter_query(r#"{"vehicles.make":"BMW"}"#)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body).len(), 0, "private Alice should be hidden");

    // Admin still sees her
    let (status, body, _) = get_json(&admin, &filter_query(r#"{"vehicles.make":"BMW"}"#)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), vec!["Alice".to_string()]);
}

// ============================================================================
// 8. No matching children → empty result (not an error)
// ============================================================================

#[tokio::test]
async fn joined_filter_with_no_matches_returns_empty() {
    let db = setup_test_db().await.unwrap();
    let app = setup_test_app(&db);
    let _ = seed_three_customers(&app).await;

    let (status, body, _) =
        get_json(&app, &filter_query(r#"{"vehicles.make":"Lamborghini"}"#)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body).len(), 0);
}

// ============================================================================
// 9. One parent with multiple matching children is not duplicated
// ============================================================================

#[tokio::test]
async fn parent_with_many_matching_children_appears_once() {
    let db = setup_test_db().await.unwrap();
    let app = setup_test_app(&db);
    let (alice_id, _, _) = seed_three_customers(&app).await;

    // Give Alice a second BMW.
    admin_post(
        &app,
        "/vehicles",
        json!({
            "customer_id": alice_id,
            "make": "BMW",
            "model": "X5",
            "year": 2024,
            "vin": "ALICE-BMW-002",
        }),
    )
    .await;

    let (status, body, _) = get_json(&app, &filter_query(r#"{"vehicles.make":"BMW"}"#)).await;
    assert_eq!(status, StatusCode::OK);

    let got = names(&body);
    assert_eq!(got, vec!["Alice".to_string()], "Alice appears exactly once");
}
