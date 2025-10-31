// Join Functionality Test
// Tests join(one), join(all), and join(one, all) combinations

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{models::vehicle::Vehicle, setup_test_app, setup_test_db};

use crate::common::customer::{CustomerList, CustomerResponse};

#[tokio::test]
async fn test_join_one_get_customer() {
    // Test that join(one) includes related data in get_one responses
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer first
    let customer_data = json!({
        "name": "Alice Johnson",
        "email": "alice@example.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(customer_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: CustomerResponse =
        serde_json::from_slice(&body).expect("Failed to parse created customer");
    let customer_id = customer.id;

    // Create a vehicle for this customer
    let vehicle_data = json!({
        "customer_id": customer_id,
        "make": "Toyota",
        "model": "Camry",
        "year": 2023,
        "vin": "1HGBH41JXMN109186"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/vehicles")
        .header("content-type", "application/json")
        .body(Body::from(vehicle_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // Test get_one customer - should NOT include vehicles (Customer only has join(all) on vehicles)
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

    // Customer has join(one, all) on vehicles, so vehicles should be populated in get_one
    // Note: This test was updated to reflect the current Customer model configuration
    // which uses join(one, all), meaning vehicles load in both get_one and get_all
    assert!(
        !retrieved_customer.vehicles.is_empty(),
        "Customer get_one should include vehicles with join(one, all)"
    );
}

#[tokio::test]
async fn test_join_all_list_customers() {
    // Test that join(all) includes related data in get_all responses
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer first
    let customer_data = json!({
        "name": "Bob Smith",
        "email": "bob@example.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(customer_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: CustomerResponse =
        serde_json::from_slice(&body).expect("Failed to parse created customer");
    let customer_id = customer.id;

    // Create a vehicle for this customer
    let vehicle_data = json!({
        "customer_id": customer_id,
        "make": "Honda",
        "model": "Civic",
        "year": 2022,
        "vin": "2HGBH41JXMN109187"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/vehicles")
        .header("content-type", "application/json")
        .body(Body::from(vehicle_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // Test get_all customers - should include vehicles (Customer has join(all) on vehicles)
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

    // Customer has join(all) on vehicles, so vehicles should be populated in get_all
    assert!(
        !found_customer.vehicles.is_empty(),
        "Customer get_all should include vehicles with join(all)"
    );
    assert_eq!(found_customer.vehicles[0].make, "Honda");
    assert_eq!(found_customer.vehicles[0].model, "Civic");
}

#[tokio::test]
async fn test_join_one_all_vehicle() {
    // Test vehicle with join(one, all) on both parts and maintenance_records
    // and join(one) on customer
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer first
    let customer_data = json!({
        "name": "Charlie Brown",
        "email": "charlie@example.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(customer_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: CustomerResponse =
        serde_json::from_slice(&body).expect("Failed to parse created customer");
    let customer_id = customer.id;

    // Create a vehicle for this customer
    let vehicle_data = json!({
        "customer_id": customer_id,
        "make": "Ford",
        "model": "F-150",
        "year": 2023,
        "vin": "3HGBH41JXMN109188"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/vehicles")
        .header("content-type", "application/json")
        .body(Body::from(vehicle_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let vehicle: Vehicle = serde_json::from_slice(&body).expect("Failed to parse created vehicle");
    let vehicle_id = vehicle.id;

    // Create a vehicle part
    let part_data = json!({
        "vehicle_id": vehicle_id,
        "name": "Engine Oil Filter",
        "part_number": "OF-12345",
        "category": "Maintenance",
        "in_stock": true
    });

    let request = Request::builder()
        .method("POST")
        .uri("/vehicle_parts")
        .header("content-type", "application/json")
        .body(Body::from(part_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // Create a maintenance record
    let maintenance_data = json!({
        "vehicle_id": vehicle_id,
        "service_type": "Oil Change",
        "description": "Regular oil change service",
        "service_date": "2024-01-15T10:00:00Z",
        "mechanic_name": "Joe Mechanic",
        "completed": true
    });

    let request = Request::builder()
        .method("POST")
        .uri("/maintenance_records")
        .header("content-type", "application/json")
        .body(Body::from(maintenance_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // Test get_one vehicle - should include parts, maintenance_records, and customer
    // (Vehicle has join(one, all) on parts and maintenance_records, and join(one) on customer)
    let request = Request::builder()
        .method("GET")
        .uri(format!("/vehicles/{vehicle_id}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let retrieved_vehicle: Vehicle =
        serde_json::from_slice(&body).expect("Failed to parse retrieved vehicle");

    // Vehicle has join(one, all) on parts and maintenance_records, so they should be populated in get_one
    assert!(
        !retrieved_vehicle.parts.is_empty(),
        "Vehicle get_one should include parts with join(one, all)"
    );
    assert!(
        !retrieved_vehicle.maintenance_records.is_empty(),
        "Vehicle get_one should include maintenance_records with join(one, all)"
    );

    assert_eq!(retrieved_vehicle.parts[0].name, "Engine Oil Filter");
    assert_eq!(
        retrieved_vehicle.maintenance_records[0].service_type,
        "Oil Change"
    );
}

#[tokio::test]
async fn test_join_one_all_list_vehicles() {
    // Test vehicle list with join(one, all) and join(one) combinations
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer first
    let customer_data = json!({
        "name": "Diana Prince",
        "email": "diana@example.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(customer_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: CustomerResponse =
        serde_json::from_slice(&body).expect("Failed to parse created customer");
    let customer_id = customer.id;

    // Create a vehicle for this customer
    let vehicle_data = json!({
        "customer_id": customer_id,
        "make": "Tesla",
        "model": "Model 3",
        "year": 2024,
        "vin": "4HGBH41JXMN109189"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/vehicles")
        .header("content-type", "application/json")
        .body(Body::from(vehicle_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let vehicle: Vehicle = serde_json::from_slice(&body).expect("Failed to parse created vehicle");
    let vehicle_id = vehicle.id;

    // Create a vehicle part
    let part_data = json!({
        "vehicle_id": vehicle_id,
        "name": "Battery Pack",
        "part_number": "BP-67890",
        "category": "Power",
        "in_stock": true
    });

    let request = Request::builder()
        .method("POST")
        .uri("/vehicle_parts")
        .header("content-type", "application/json")
        .body(Body::from(part_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // Test get_all vehicles - should include parts and maintenance_records
    // (Vehicle has join(one, all) on parts and maintenance_records)
    let request = Request::builder()
        .method("GET")
        .uri("/vehicles")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let vehicles: Vec<Vehicle> =
        serde_json::from_slice(&body).expect("Failed to parse vehicles list");

    // Find our vehicle in the list
    let found_vehicle = vehicles
        .iter()
        .find(|v| v.id == vehicle_id)
        .expect("Vehicle not found in list");

    // Vehicle has join(one, all) on parts and maintenance_records, so they should be populated in get_all
    assert!(
        !found_vehicle.parts.is_empty(),
        "Vehicle get_all should include parts with join(one, all)"
    );
    assert_eq!(found_vehicle.parts[0].name, "Battery Pack");
}

#[tokio::test]
async fn test_join_empty_relationships() {
    // Test join behavior when there are no related records
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer without any vehicles
    let customer_data = json!({
        "name": "Eve Wilson",
        "email": "eve@example.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(customer_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: CustomerResponse =
        serde_json::from_slice(&body).expect("Failed to parse created customer");
    let customer_id = customer.id;

    // Test get_all customers - should include empty vehicles array
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

    // Customer has join(all) on vehicles, so vehicles field should be present but empty
    assert_eq!(
        found_customer.vehicles.len(),
        0,
        "Customer get_all should include empty vehicles array with join(all)"
    );
}
