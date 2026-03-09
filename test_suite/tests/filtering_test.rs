// String Comparison Operator Tests
// Tests that string filter operators (_neq, _gte, _lt, etc.) are applied correctly.
// These expose BUG 1 (operators ignored for strings) and BUG 2 (operator-suffixed key
// used for like_filterable/is_enum_field checks).

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

/// Test that {"name_neq": "Alice"} returns everyone EXCEPT Alice.
/// BUG 1: process_string_filter ignores the comparison operator and always does equality,
/// so this currently returns only Alice (wrong) instead of everyone except Alice.
#[tokio::test]
async fn test_string_neq_filter() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create test data
    let customers = [
        json!({"name": "Alice", "email": "alice@example.com"}),
        json!({"name": "Bob", "email": "bob@example.com"}),
        json!({"name": "Charlie", "email": "charlie@example.com"}),
    ];

    for customer_data in &customers {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(customer_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Filter for name != Alice
    let filter = json!({"name_neq": "Alice"});
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
    let customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    // Should return Bob and Charlie (everyone except Alice)
    assert_eq!(
        customers.len(),
        2,
        "name_neq should return everyone except Alice, got {} results",
        customers.len()
    );
    assert!(
        customers.iter().all(|c| c.name != "Alice"),
        "No result should be Alice when filtering with _neq"
    );
}

/// Test that {"name_gte": "C"} returns names >= "C" (case-insensitive).
/// BUG 1: process_string_filter always does equality, so this returns nothing (wrong).
#[tokio::test]
async fn test_string_gte_filter() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    let customers = [
        json!({"name": "Alice", "email": "alice@example.com"}),
        json!({"name": "Charlie", "email": "charlie@example.com"}),
        json!({"name": "Zara", "email": "zara@example.com"}),
    ];

    for customer_data in &customers {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(customer_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Filter for name >= "C" (should return Charlie and Zara)
    let filter = json!({"name_gte": "C"});
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
    let customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        customers.len(),
        2,
        "name_gte 'C' should return Charlie and Zara, got {} results",
        customers.len()
    );
}

/// Test that {"name_lt": "C"} returns names < "C" (case-insensitive).
/// BUG 1: process_string_filter always does equality, so this returns nothing (wrong).
#[tokio::test]
async fn test_string_lt_filter() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    let customers = [
        json!({"name": "Alice", "email": "alice@example.com"}),
        json!({"name": "Bob", "email": "bob@example.com"}),
        json!({"name": "Charlie", "email": "charlie@example.com"}),
    ];

    for customer_data in &customers {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(customer_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Filter for name < "C" (should return Alice and Bob)
    let filter = json!({"name_lt": "C"});
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
    let customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        customers.len(),
        2,
        "name_lt 'C' should return Alice and Bob, got {} results",
        customers.len()
    );
}
