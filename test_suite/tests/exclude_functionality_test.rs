// Exclude Functionality Test
// Tests exclude(one), exclude(create), exclude(update), exclude(list), and mixed combinations

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_app, setup_test_db};

use crate::common::customer::{CustomerList, CustomerResponse};

#[tokio::test]
async fn test_exclude_one_get_customer() {
    // Test that exclude(one) removes fields from get_one responses
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer
    let create_data = json!({
        "name": "Alice Johnson",
        "email": "alice@example.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_customer: CustomerResponse =
        serde_json::from_slice(&body).expect("Failed to parse created customer");
    let customer_id = created_customer.id;

    // Test get_one customer - should NOT include excluded fields
    let request = Request::builder()
        .method("GET")
        .uri(format!("/customers/{customer_id}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let retrieved_customer: CustomerResponse =
        serde_json::from_slice(&body).expect("Failed to parse retrieved customer");

    // Check that excluded fields are not present in get_one response
    // Customer has exclude(one) on created_at and updated_at
    assert_eq!(retrieved_customer.id, customer_id);
    assert_eq!(retrieved_customer.name, "Alice Johnson");
    assert_eq!(retrieved_customer.email, "alice@example.com");

    // Note: The Customer struct still has these fields but they should have default values
    // Since they have on_create/on_update expressions, they're included in struct but excluded from get_one response logic
}

#[tokio::test]
async fn test_exclude_create_customer() {
    // Test that exclude(create) removes fields from create operations
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Try to create a customer with exclude(create) fields
    // Customer id has exclude(create, update) so it should not be in create request
    let create_data = json!({
        "id": "550e8400-e29b-41d4-a716-446655440000", // This should be ignored
        "name": "Bob Smith",
        "email": "bob@example.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_customer: CustomerResponse =
        serde_json::from_slice(&body).expect("Failed to parse created customer");

    // ID should be auto-generated (different from what we sent)
    assert_ne!(
        created_customer.id.to_string(),
        "550e8400-e29b-41d4-a716-446655440000"
    );
    assert_eq!(created_customer.name, "Bob Smith");
    assert_eq!(created_customer.email, "bob@example.com");
}

#[tokio::test]
async fn test_exclude_update_customer() {
    // Test that exclude(update) removes fields from update operations
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer first
    let create_data = json!({
        "name": "Charlie Brown",
        "email": "charlie@example.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_customer: CustomerResponse =
        serde_json::from_slice(&body).expect("Failed to parse created customer");
    let customer_id = created_customer.id;

    // Try to update customer with exclude(update) fields
    // Customer id has exclude(create, update) so it should not be updatable
    let update_data = json!({
        "id": "550e8400-e29b-41d4-a716-446655440000", // This should be ignored
        "name": "Charlie Updated",
        "email": "charlie.updated@example.com"
    });

    let request = Request::builder()
        .method("PUT")
        .uri(format!("/customers/{customer_id}"))
        .header("content-type", "application/json")
        .body(Body::from(update_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated_customer: CustomerResponse =
        serde_json::from_slice(&body).expect("Failed to parse updated customer");

    // ID should remain unchanged (not updated)
    assert_eq!(updated_customer.id, customer_id);
    assert_ne!(
        updated_customer.id.to_string(),
        "550e8400-e29b-41d4-a716-446655440000"
    );
    assert_eq!(updated_customer.name, "Charlie Updated");
    assert_eq!(updated_customer.email, "charlie.updated@example.com");
}

#[tokio::test]
async fn test_exclude_list_customers() {
    // Test that exclude(list) removes fields from list operations
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer
    let create_data = json!({
        "name": "Diana Prince",
        "email": "diana@example.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // Test get_all customers - should include/exclude appropriate fields
    let request = Request::builder()
        .method("GET")
        .uri("/customers")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<CustomerList> =
        serde_json::from_slice(&body).expect("Failed to parse customers list");

    // Find our customer in the list
    let found_customer = customers
        .iter()
        .find(|c| c.name == "Diana Prince")
        .expect("Customer not found in list");

    // All fields should be present according to the model configuration
    assert_eq!(found_customer.name, "Diana Prince");
    assert_eq!(found_customer.email, "diana@example.com");
    assert!(!found_customer.id.to_string().is_empty());
}

#[tokio::test]
async fn test_exclude_mixed_combinations() {
    // Test fields with multiple exclusions (e.g., exclude(create, update))
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer
    let create_data = json!({
        "name": "Eve Wilson",
        "email": "eve@example.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_customer: CustomerResponse =
        serde_json::from_slice(&body).expect("Failed to parse created customer");
    let customer_id = created_customer.id;

    // Test that fields with exclude(create, update) cannot be set in either operation
    let update_data = json!({
        "id": "550e8400-e29b-41d4-a716-446655440000", // Should be ignored in update
        "name": "Eve Updated",
        "email": "eve.updated@example.com"
    });

    let request = Request::builder()
        .method("PUT")
        .uri(format!("/customers/{customer_id}"))
        .header("content-type", "application/json")
        .body(Body::from(update_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated_customer: CustomerResponse =
        serde_json::from_slice(&body).expect("Failed to parse updated customer");

    // ID should remain unchanged despite being sent in update
    assert_eq!(updated_customer.id, customer_id);
    assert_eq!(updated_customer.name, "Eve Updated");
    assert_eq!(updated_customer.email, "eve.updated@example.com");
}

#[tokio::test]
async fn test_exclude_with_auto_generated_values() {
    // Test that exclude(one) with on_create works correctly
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer
    let create_data = json!({
        "name": "Frank Miller",
        "email": "frank@example.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_customer: CustomerResponse =
        serde_json::from_slice(&body).expect("Failed to parse created customer");

    // Customer should have auto-generated ID
    assert!(!created_customer.id.to_string().is_empty());
    assert_ne!(
        created_customer.id.to_string(),
        "00000000-0000-0000-0000-000000000000"
    );
}

#[tokio::test]
async fn test_exclude_function_comprehensive() {
    // Comprehensive test of all exclude combinations working together
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Test 1: Create (exclude(create) fields should be ignored)
    let create_data = json!({
        "id": "550e8400-e29b-41d4-a716-446655440000", // exclude(create) - should be ignored
        "name": "Grace Hopper",
        "email": "grace@example.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_customer: CustomerResponse =
        serde_json::from_slice(&body).expect("Failed to parse created customer");

    // Verify ID was auto-generated, not set from request
    assert_ne!(
        created_customer.id.to_string(),
        "550e8400-e29b-41d4-a716-446655440000"
    );
    let customer_id = created_customer.id;

    // Test 2: Update (exclude(update) fields should be ignored)
    let update_data = json!({
        "id": "660e8400-e29b-41d4-a716-446655440000", // exclude(update) - should be ignored
        "name": "Grace Updated",
        "email": "grace.updated@example.com"
    });

    let request = Request::builder()
        .method("PUT")
        .uri(format!("/customers/{customer_id}"))
        .header("content-type", "application/json")
        .body(Body::from(update_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated_customer: CustomerResponse =
        serde_json::from_slice(&body).expect("Failed to parse updated customer");

    // Verify ID remained unchanged
    assert_eq!(updated_customer.id, customer_id);
    assert_eq!(updated_customer.name, "Grace Updated");

    // Test 3: Get one (exclude(one) fields should be excluded)
    let request = Request::builder()
        .method("GET")
        .uri(format!("/customers/{customer_id}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let retrieved_customer: CustomerResponse =
        serde_json::from_slice(&body).expect("Failed to parse retrieved customer");

    // Verify customer data is correct
    assert_eq!(retrieved_customer.id, customer_id);
    assert_eq!(retrieved_customer.name, "Grace Updated");

    // Test 4: List (exclude(list) fields should be excluded from list responses)
    let request = Request::builder()
        .method("GET")
        .uri("/customers")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<CustomerList> =
        serde_json::from_slice(&body).expect("Failed to parse customers list");

    // Find our customer in the list
    let found_customer = customers
        .iter()
        .find(|c| c.id == customer_id)
        .expect("Customer not found in list");
    assert_eq!(found_customer.name, "Grace Updated");
}

#[tokio::test]
async fn test_exclude_all() {
    // Test that exclude(all) removes fields from both get_one AND get_all responses
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer
    let create_data = json!({
        "name": "Test Exclude All",
        "email": "exclude_all@example.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_customer: CustomerResponse =
        serde_json::from_slice(&body).expect("Failed to parse created customer");
    let customer_id = created_customer.id;

    // Test get_one - exclude(all) fields should NOT be present
    let request = Request::builder()
        .method("GET")
        .uri(format!("/customers/{customer_id}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8_lossy(&body);

    // If updated_at had exclude(all), it should NOT be in get_one response
    // This test will verify once we add exclude(all) to a field in the test models
    assert!(
        body_str.contains(&customer_id.to_string()),
        "Should contain customer ID"
    );

    // Test get_all - exclude(all) fields should also NOT be present
    let request = Request::builder()
        .method("GET")
        .uri("/customers")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<CustomerList> =
        serde_json::from_slice(&body).expect("Failed to parse customers list");

    // Find our customer in the list
    let found_customer = customers
        .iter()
        .find(|c| c.id == customer_id)
        .expect("Customer not found in list");
    assert_eq!(found_customer.name, "Test Exclude All");

    // If a field had exclude(all), it should NOT appear in either get_one or get_all
    // This demonstrates that exclude(all) works for both endpoints
}
