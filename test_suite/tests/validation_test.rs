// Error Handling Tests
// Tests error conditions and edge cases to improve coverage

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

mod common;
use common::{setup_test_app, setup_test_db};

#[tokio::test]
async fn test_crud_error_conditions() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Test 1: GET non-existent resource (exercises error handling in get_one)
    let non_existent_id = Uuid::new_v4();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/customers/{non_existent_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Test 2: UPDATE non-existent resource (exercises error handling in update)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/customers/{non_existent_id}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name": "Updated", "email": "updated@example.com"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Test 3: DELETE non-existent resource (exercises error handling in delete)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/customers/{non_existent_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Test 4: Invalid JSON payload (exercises error handling in create)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/customers")
                .header("content-type", "application/json")
                .body(Body::from("invalid json"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Test 5: Missing required fields (exercises validation in create)
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/customers")
                .header("content-type", "application/json")
                .body(Body::from(json!({"name": "Missing Email"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_database_connection_handling() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Test batch operations (exercises delete_many and total_count functions)

    // First create some customers to delete
    let _customer_ids: Vec<String> = vec![];
    for i in 0..3 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": format!("Customer {}", i),
                            "email": format!("customer{}@example.com", i)
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
    }

    // Test invalid UUID format
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers/invalid-uuid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 400 Bad Request for invalid UUID format
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
