// Core Filtering Tests - Target the untested filtering logic
// Addresses the major coverage gap in filtering/conditions.rs (8/149 lines covered)

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_db, setup_test_app, Customer};

#[tokio::test]
async fn test_filtering_and_sorting() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create test data
    let customers = [
        json!({"name": "Alice", "email": "alice@example.com"}),
        json!({"name": "Bob", "email": "bob@example.com"}),
        json!({"name": "Charlie", "email": "charlie@example.com"}),
    ];

    for customer_data in &customers {
        let request = Request::builder()
            .method("POST")
            .uri("/customers")
            .header("content-type", "application/json")
            .body(Body::from(customer_data.to_string()))
            .unwrap();

        app.clone().oneshot(request).await.unwrap();
    }

    // Test 1: Basic filtering (exercises apply_filters function)
    let response = app.clone().oneshot(
        Request::builder()
            .method("GET")
            .uri("/customers?filter=%7B%22name%22%3A%22Alice%22%7D")
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let customers: Vec<Customer> = serde_json::from_slice(&body).unwrap();
    assert_eq!(customers.len(), 1);
    assert_eq!(customers[0].name, "Alice");

    // Test 2: Sorting (exercises build_sort_conditions function)  
    let response = app.clone().oneshot(
        Request::builder()
            .method("GET") 
            .uri("/customers?sort=%5B%22name%22%2C%22DESC%22%5D")
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let customers: Vec<Customer> = serde_json::from_slice(&body).unwrap();
    
    // Should be in descending order: Charlie, Bob, Alice
    assert!(customers.len() >= 3);
    assert!(customers[0].name >= customers[1].name);

    // Test 3: Pagination (exercises apply_pagination function)
    let response = app.oneshot(
        Request::builder()
            .method("GET")
            .uri("/customers?page=1&per_page=2")
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let customers: Vec<Customer> = serde_json::from_slice(&body).unwrap();
    assert!(customers.len() <= 2);
}

#[tokio::test]
async fn test_fulltext_search() {
    let db = setup_test_db().await.expect("Failed to setup test database");  
    let app = setup_test_app(&db);

    // Create customer with searchable content
    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(json!({"name": "John Developer", "email": "john@tech.com"}).to_string()))
        .unwrap();
    
    app.clone().oneshot(request).await.unwrap();

    // Test fulltext search with 'q' parameter (exercises handle_fulltext_search function)
    let response = app.oneshot(
        Request::builder()
            .method("GET")
            .uri("/customers?filter=%7B%22q%22%3A%22Developer%22%7D")
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let customers: Vec<Customer> = serde_json::from_slice(&body).unwrap();
    
    // Should find the customer with "Developer" in name
    assert!(!customers.is_empty());
    assert!(customers.iter().any(|c| c.name.contains("Developer")));
}