// Relationship Loading Tests
// Tests the join functionality that loads related entities

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_app, setup_test_db};

use crate::common::customer::{CustomerList, CustomerResponse};

#[tokio::test]
async fn test_relationship_loading() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer
    let customer_request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"name": "John Doe", "email": "john@example.com"}).to_string(),
        ))
        .unwrap();

    let customer_response = app.clone().oneshot(customer_request).await.unwrap();
    assert_eq!(customer_response.status(), StatusCode::CREATED);

    let customer_body = axum::body::to_bytes(customer_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_customer: CustomerResponse = serde_json::from_slice(&customer_body).unwrap();

    // Create a vehicle for this customer
    let vehicle_request = Request::builder()
        .method("POST")
        .uri("/vehicles")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "customer_id": created_customer.id,
                "make": "Toyota",
                "model": "Camry",
                "year": 2020,
                "vin": "12345678901234567"
            })
            .to_string(),
        ))
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

    let get_customer_body = axum::body::to_bytes(get_customer_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer_with_vehicles: CustomerResponse =
        serde_json::from_slice(&get_customer_body).unwrap();

    // Verify the relationship WAS loaded (Customer has join(one, all))
    assert_eq!(customer_with_vehicles.id, created_customer.id);
    assert_eq!(
        customer_with_vehicles.vehicles.len(),
        1,
        "Customer get_one should include vehicles with join(one, all)"
    );
}

#[tokio::test]
async fn test_relationship_loading_in_get_all() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer
    let customer_request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"name": "Jane Smith", "email": "jane@example.com"}).to_string(),
        ))
        .unwrap();

    let customer_response = app.clone().oneshot(customer_request).await.unwrap();
    assert_eq!(customer_response.status(), StatusCode::CREATED);

    let customer_body = axum::body::to_bytes(customer_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_customer: CustomerResponse = serde_json::from_slice(&customer_body).unwrap();

    // Create a vehicle for this customer
    let vehicle_request = Request::builder()
        .method("POST")
        .uri("/vehicles")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "customer_id": created_customer.id,
                "make": "Honda",
                "model": "Civic",
                "year": 2021,
                "vin": "98765432109876543"
            })
            .to_string(),
        ))
        .unwrap();

    let vehicle_response = app.clone().oneshot(vehicle_request).await.unwrap();
    assert_eq!(vehicle_response.status(), StatusCode::CREATED);

    // Test relationship loading: Get all customers and verify vehicles are loaded
    let get_all_customers_request = Request::builder()
        .method("GET")
        .uri("/customers")
        .body(Body::empty())
        .unwrap();

    let get_all_customers_response = app.oneshot(get_all_customers_request).await.unwrap();
    assert_eq!(get_all_customers_response.status(), StatusCode::OK);

    let get_all_customers_body =
        axum::body::to_bytes(get_all_customers_response.into_body(), usize::MAX)
            .await
            .unwrap();
    let customers: Vec<CustomerList> = serde_json::from_slice(&get_all_customers_body).unwrap();

    // Find our customer in the list
    let found_customer = customers
        .iter()
        .find(|c| c.id == created_customer.id)
        .expect("Customer not found in list");

    // Verify the relationship WAS loaded in get_all() (Customer has join(all))
    assert_eq!(found_customer.id, created_customer.id);
    assert_eq!(
        found_customer.vehicles.len(),
        1,
        "Customer get_all should include vehicles with join(all)"
    );
    assert_eq!(found_customer.vehicles[0].make, "Honda");
    assert_eq!(found_customer.vehicles[0].model, "Civic");
}
