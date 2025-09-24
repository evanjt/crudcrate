// Basic CRUD Operations Test
// Tests the fundamental Create, Read, Update, Delete operations

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_db, setup_test_app, Customer};

#[tokio::test]
async fn test_customer_crud_operations() {
    // Setup test database and router
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Test 1: Create a customer
    let create_data = json!({
        "name": "John Doe",
        "email": "john@example.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    
    // Debug: Print response status and body if not created
    if status != StatusCode::CREATED {
        eprintln!("Create request failed with status: {}", status);
        eprintln!("Response body: {}", String::from_utf8_lossy(&body));
        panic!("Expected 201 Created, got {}", status);
    }
    
    assert_eq!(status, StatusCode::CREATED);
    let created_customer: Customer = serde_json::from_slice(&body).expect("Failed to parse created customer");
    
    // Verify customer was created with correct data
    assert_eq!(created_customer.name, "John Doe");
    assert_eq!(created_customer.email, "john@example.com");
    
    let customer_id = created_customer.id;

    // Test 2: Get the created customer
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/customers/{}", customer_id))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let retrieved_customer: Customer = serde_json::from_slice(&body).expect("Failed to parse retrieved customer");
    
    assert_eq!(retrieved_customer.id, customer_id);
    assert_eq!(retrieved_customer.name, "John Doe");
    assert_eq!(retrieved_customer.email, "john@example.com");

    // Test 3: Update the customer
    let update_data = json!({
        "name": "John Smith",
        "email": "johnsmith@example.com"
    });

    let request = Request::builder()
        .method("PUT")
        .uri(&format!("/customers/{}", customer_id))
        .header("content-type", "application/json")
        .body(Body::from(update_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let updated_customer: Customer = serde_json::from_slice(&body).expect("Failed to parse updated customer");
    
    assert_eq!(updated_customer.id, customer_id);
    assert_eq!(updated_customer.name, "John Smith");
    assert_eq!(updated_customer.email, "johnsmith@example.com");

    // Test 4: List customers (should include our updated customer)
    let request = Request::builder()
        .method("GET")
        .uri("/customers")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let customers: Vec<Customer> = serde_json::from_slice(&body).expect("Failed to parse customers list");
    
    assert!(!customers.is_empty());
    let found_customer = customers.iter().find(|c| c.id == customer_id).expect("Customer not found in list");
    assert_eq!(found_customer.name, "John Smith");

    // Test 5: Delete the customer
    let request = Request::builder()
        .method("DELETE")
        .uri(&format!("/customers/{}", customer_id))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Test 6: Verify customer is deleted (should return 404)
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/customers/{}", customer_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}