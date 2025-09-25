// Relationship Loading Tests
// Tests the join functionality that loads related entities

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_db, setup_test_app, Customer};

#[tokio::test]
async fn test_relationship_loading() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer
    let customer_request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(json!({"name": "John Doe", "email": "john@example.com"}).to_string()))
        .unwrap();

    let customer_response = app.clone().oneshot(customer_request).await.unwrap();
    assert_eq!(customer_response.status(), StatusCode::CREATED);

    let customer_body = axum::body::to_bytes(customer_response.into_body(), usize::MAX).await.unwrap();
    let created_customer: Customer = serde_json::from_slice(&customer_body).unwrap();

    // Create a vehicle for this customer
    let vehicle_request = Request::builder()
        .method("POST")
        .uri("/vehicles")
        .header("content-type", "application/json")
        .body(Body::from(json!({
            "customer_id": created_customer.id,
            "make": "Toyota",
            "model": "Camry", 
            "year": 2020,
            "vin": "12345678901234567"
        }).to_string()))
        .unwrap();

    let vehicle_response = app.clone().oneshot(vehicle_request).await.unwrap();
    assert_eq!(vehicle_response.status(), StatusCode::CREATED);

    // Test relationship loading: Get customer with vehicles loaded
    let get_customer_request = Request::builder()
        .method("GET")
        .uri(format!("/customers/{}", created_customer.id))
        .body(Body::empty())
        .unwrap();

    let get_customer_response = app.oneshot(get_customer_request).await.unwrap();
    assert_eq!(get_customer_response.status(), StatusCode::OK);

    let get_customer_body = axum::body::to_bytes(get_customer_response.into_body(), usize::MAX).await.unwrap();
    let customer_with_vehicles: Customer = serde_json::from_slice(&get_customer_body).unwrap();

    // Verify the relationship was loaded
    assert_eq!(customer_with_vehicles.id, created_customer.id);
    assert_eq!(customer_with_vehicles.vehicles.len(), 1);
    assert_eq!(customer_with_vehicles.vehicles[0].make, "Toyota");
    assert_eq!(customer_with_vehicles.vehicles[0].model, "Camry");
}