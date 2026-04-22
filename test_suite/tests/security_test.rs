// Security Boundary Tests
// Tests input validation, empty/malicious queries, and boundary conditions.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_app, setup_test_db};

use crate::common::customer::CustomerList;

/// Helper function to URL-encode a filter JSON for use in query strings
fn encode_filter(filter: &serde_json::Value) -> String {
    url_escape::encode_component(&filter.to_string()).to_string()
}

/// Test that {"q": ""} does NOT match all items.
/// BUG 3: Empty fulltext search falls through to LIKE '%%' which matches everything.
/// Correct behavior: empty search query should be a no-op (return all without filter).
#[tokio::test]
async fn test_empty_fulltext_returns_all_without_filter() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create some customers
    for i in 0..3 {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"name": format!("Customer {}", i), "email": format!("c{}@example.com", i)})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Get count without any filter
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let all_customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();
    let total_count = all_customers.len();

    // Search with empty query - should return same results as no filter
    let filter = json!({"q": ""});
    let encoded_filter = encode_filter(&filter);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/customers?filter={}", encoded_filter))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        result.len(),
        total_count,
        "Empty search query should return all results (no filter applied)"
    );
}

/// Test that {"q": "   "} (whitespace-only) does NOT match all items.
/// BUG 3: Whitespace query isn't trimmed in the fulltext search path.
#[tokio::test]
async fn test_whitespace_fulltext_returns_all_without_filter() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    for i in 0..3 {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"name": format!("WS Customer {}", i), "email": format!("ws{}@example.com", i)})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Get count without any filter
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let all_customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();
    let total_count = all_customers.len();

    // Search with whitespace-only query
    let filter = json!({"q": "   "});
    let encoded_filter = encode_filter(&filter);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/customers?filter={}", encoded_filter))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        result.len(),
        total_count,
        "Whitespace-only search query should return all results (no filter applied)"
    );
}

/// Test that filtering by a non-existent field returns all items (no crash)
#[tokio::test]
async fn test_invalid_filter_field_ignored() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/customers")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name": "Test", "email": "test@example.com"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let filter = json!({"nonexistent_field": "x"});
    let encoded_filter = encode_filter(&filter);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/customers?filter={}", encoded_filter))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    assert!(
        !result.is_empty(),
        "Invalid filter field should be ignored, returning all items"
    );
}

/// Test that an invalid sort field doesn't crash
#[tokio::test]
async fn test_invalid_sort_field_uses_default() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/customers")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name": "Test", "email": "test@example.com"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // sort=["bogus","ASC"] should use default sort, not crash
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?sort=%5B%22bogus%22%2C%22ASC%22%5D")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Invalid sort field should fall back to default"
    );
}

/// Defence against DoS via filter-clause flooding.
/// A request whose filter JSON has more than the built-in limit (100 keys)
/// must be rejected with 400 Bad Request — NOT silently dropped into an
/// unfiltered response.
#[tokio::test]
async fn test_filter_with_too_many_clauses_returns_400() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Build a filter object with 101 unique keys — one over the limit.
    let mut obj = serde_json::Map::with_capacity(101);
    for i in 0..=100 {
        obj.insert(format!("key{i}"), json!(i));
    }
    let filter = serde_json::Value::Object(obj);
    let encoded_filter = encode_filter(&filter);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/customers?filter={encoded_filter}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "Filter with 101 clauses must return 400, not silently return unfiltered results"
    );
}

/// A filter at exactly the limit (100 keys) must still succeed.
#[tokio::test]
async fn test_filter_at_clause_limit_succeeds() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Build a filter object with exactly 100 keys. Most will not match any
    // real column, but the framework silently ignores unknown columns — the
    // important check is that the request is accepted (no 400).
    let mut obj = serde_json::Map::with_capacity(100);
    for i in 0..100 {
        obj.insert(format!("nonexistent_field_{i}"), json!(i));
    }
    let filter = serde_json::Value::Object(obj);
    let encoded_filter = encode_filter(&filter);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/customers?filter={encoded_filter}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Filter with exactly 100 clauses must be accepted"
    );
}
