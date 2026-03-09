// Integration Tests for Filtering Edge Cases
// Tests unique filter behaviors not covered by comprehensive_* test files

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_app, setup_test_db};

use crate::common::customer::CustomerList;
use crate::common::maintenance_record::MaintenanceRecordList;
use crate::common::vehicle_part::VehiclePartList;

/// Helper function to URL-encode a filter JSON for use in query strings
fn encode_filter(filter: &serde_json::Value) -> String {
    url_escape::encode_component(&filter.to_string()).to_string()
}

/// Test NULL filtering: {"field": null}
/// Should return records where the field is NULL
#[tokio::test]
async fn test_filter_null_field() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // First create a customer and vehicle
    let customer_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/customers")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name": "Test Customer", "email": "test@example.com"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(customer_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let customer_id = customer["id"].as_str().unwrap();

    // Create a vehicle
    let vehicle_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vehicles")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "customer_id": customer_id,
                        "make": "Toyota",
                        "model": "Camry",
                        "year": 2020,
                        "vin": "ABC123"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(vehicle_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let vehicle: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let vehicle_id = vehicle["id"].as_str().unwrap();

    // Create maintenance records - one with mechanic_name NULL, one with value
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/maintenance_records")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "vehicle_id": vehicle_id,
                        "service_type": "Oil Change",
                        "description": "Regular maintenance",
                        "service_date": "2024-01-15T10:00:00Z",
                        "completed": true
                        // mechanic_name is NULL (not provided)
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/maintenance_records")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "vehicle_id": vehicle_id,
                        "service_type": "Tire Rotation",
                        "description": "Rotate all tires",
                        "service_date": "2024-01-20T10:00:00Z",
                        "mechanic_name": "John Smith",
                        "completed": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Filter for records where mechanic_name is NULL
    let filter = json!({"mechanic_name": null});
    let encoded_filter = encode_filter(&filter);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/maintenance_records?filter={}", encoded_filter))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let records: Vec<MaintenanceRecordList> = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        records.len(),
        1,
        "Should find exactly 1 record with NULL mechanic_name"
    );
    assert_eq!(records[0].service_type, "Oil Change");
}

/// Test Array IN filtering: {"field": [value1, value2]}
/// Should return records matching any value in the array
#[tokio::test]
async fn test_filter_array_in() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create multiple customers
    let customers = [
        json!({"name": "Alice", "email": "alice@example.com"}),
        json!({"name": "Bob", "email": "bob@example.com"}),
        json!({"name": "Charlie", "email": "charlie@example.com"}),
        json!({"name": "Diana", "email": "diana@example.com"}),
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

    // Filter for Alice and Charlie using array
    let filter = json!({"name": ["Alice", "Charlie"]});
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

    assert_eq!(result.len(), 2, "Should find exactly 2 customers");
    let names: Vec<&str> = result.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"Alice"), "Should include Alice");
    assert!(names.contains(&"Charlie"), "Should include Charlie");
}

/// Test boolean filtering
/// Should correctly filter by true/false values
#[tokio::test]
async fn test_filter_boolean() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customer and vehicle first
    let customer_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/customers")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name": "Bool Test", "email": "bool@example.com"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(customer_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let customer_id = customer["id"].as_str().unwrap();

    let vehicle_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vehicles")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "customer_id": customer_id,
                        "make": "Ford",
                        "model": "Focus",
                        "year": 2019,
                        "vin": "BOOL123"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(vehicle_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let vehicle: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let vehicle_id = vehicle["id"].as_str().unwrap();

    // Create parts with different in_stock values
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vehicle_parts")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "vehicle_id": vehicle_id,
                        "name": "In Stock Part",
                        "part_number": "IS001",
                        "category": "Engine",
                        "in_stock": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vehicle_parts")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "vehicle_id": vehicle_id,
                        "name": "Out of Stock Part",
                        "part_number": "OS001",
                        "category": "Brakes",
                        "in_stock": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Filter for in_stock = true
    let filter = json!({"in_stock": true});
    let encoded_filter = encode_filter(&filter);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/vehicle_parts?filter={}", encoded_filter))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parts: Vec<VehiclePartList> = serde_json::from_slice(&body).unwrap();

    assert_eq!(parts.len(), 1, "Should find exactly 1 in-stock part");
    assert_eq!(parts[0].name, "In Stock Part");

    // Filter for in_stock = false
    let filter = json!({"in_stock": false});
    let encoded_filter = encode_filter(&filter);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/vehicle_parts?filter={}", encoded_filter))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parts: Vec<VehiclePartList> = serde_json::from_slice(&body).unwrap();

    assert_eq!(parts.len(), 1, "Should find exactly 1 out-of-stock part");
    assert_eq!(parts[0].name, "Out of Stock Part");
}

/// Test field validation - unknown fields should be silently ignored
#[tokio::test]
async fn test_filter_unknown_field_ignored() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer
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

    // Filter by unknown field - should be ignored, return all
    let filter = json!({"unknown_field": "value", "nonexistent": 123});
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

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Unknown fields should be silently ignored, not cause 400"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    assert!(
        !result.is_empty(),
        "Should return records when unknown fields are ignored"
    );
}
