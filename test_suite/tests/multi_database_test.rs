// Multi-Database Compatibility Tests
// Tests that filtering and CRUD operations work across SQLite, PostgreSQL, and MySQL

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_db, setup_test_app, Customer};

#[tokio::test]
async fn test_basic_crud_multi_database() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Test basic CRUD operations that should work on all databases
    
    // Create
    let create_response = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/customers")
            .header("content-type", "application/json")
            .body(Body::from(json!({
                "name": "Multi DB Test",
                "email": "multidb@example.com"
            }).to_string()))
            .unwrap()
    ).await.unwrap();
    
    assert_eq!(create_response.status(), StatusCode::CREATED);
    
    // Read (list all)
    let list_response = app.clone().oneshot(
        Request::builder()
            .method("GET")
            .uri("/customers")
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(list_response.into_body(), usize::MAX).await.unwrap();
    let customers: Vec<Customer> = serde_json::from_slice(&body).unwrap();
    assert!(!customers.is_empty());
    
    let created_customer = customers.iter()
        .find(|c| c.email == "multidb@example.com")
        .expect("Created customer should be found");
    
    // Read (get one)
    let get_one_response = app.clone().oneshot(
        Request::builder()
            .method("GET")
            .uri(&format!("/customers/{}", created_customer.id))
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    
    assert_eq!(get_one_response.status(), StatusCode::OK);
    
    // Update
    let update_response = app.clone().oneshot(
        Request::builder()
            .method("PUT")
            .uri(&format!("/customers/{}", created_customer.id))
            .header("content-type", "application/json")
            .body(Body::from(json!({
                "name": "Updated Multi DB Test",
                "email": "updated-multidb@example.com"
            }).to_string()))
            .unwrap()
    ).await.unwrap();
    
    assert!(update_response.status() == StatusCode::OK || update_response.status() == StatusCode::NO_CONTENT);
    
    // Delete
    let delete_response = app.oneshot(
        Request::builder()
            .method("DELETE")
            .uri(&format!("/customers/{}", created_customer.id))
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    
    assert!(delete_response.status() == StatusCode::OK || delete_response.status() == StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_filtering_multi_database() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create test data
    let test_customers = [
        json!({"name": "Alice Database", "email": "alice.db@example.com"}),
        json!({"name": "Bob Database", "email": "bob.db@example.com"}),
    ];

    for customer_data in &test_customers {
        let request = Request::builder()
            .method("POST")
            .uri("/customers")
            .header("content-type", "application/json")
            .body(Body::from(customer_data.to_string()))
            .unwrap();

        app.clone().oneshot(request).await.unwrap();
    }

    // Test filtering (should work on all database backends)
    let filter_response = app.clone().oneshot(
        Request::builder()
            .method("GET")
            .uri("/customers?filter=%7B%22name%22%3A%22Alice%20Database%22%7D")
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    
    assert_eq!(filter_response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(filter_response.into_body(), usize::MAX).await.unwrap();
    let customers: Vec<Customer> = serde_json::from_slice(&body).unwrap();
    assert_eq!(customers.len(), 1);
    assert_eq!(customers[0].name, "Alice Database");

    // Test fulltext search (fallback to LIKE for all databases)
    let search_response = app.oneshot(
        Request::builder()
            .method("GET")
            .uri("/customers?filter=%7B%22q%22%3A%22Database%22%7D")
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    
    assert_eq!(search_response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(search_response.into_body(), usize::MAX).await.unwrap();
    let customers: Vec<Customer> = serde_json::from_slice(&body).unwrap();
    assert!(customers.len() >= 2); // Should find both customers with "Database" in name
}