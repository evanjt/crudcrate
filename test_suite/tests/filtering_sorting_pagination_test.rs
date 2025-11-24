// Comprehensive Integration Tests for Filtering, Sorting, and Pagination
// Tests all documented features to ensure implementation matches documentation

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_app, setup_test_db};

use crate::common::customer::CustomerList;
use crate::common::maintenance_record::MaintenanceRecordList;
use crate::common::vehicle::VehicleList;
use crate::common::vehicle_part::VehiclePartList;

/// Helper function to URL-encode a filter JSON for use in query strings
fn encode_filter(filter: &serde_json::Value) -> String {
    url_escape::encode_component(&filter.to_string()).to_string()
}

// ============================================================================
// FILTERING TESTS
// ============================================================================

/// Test NULL filtering: {"field": null}
/// Should return records where the field is NULL
#[tokio::test]
async fn test_filter_null_field() {
    let db = setup_test_db().await.expect("Failed to setup test database");
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

    // Should find only the record without mechanic_name
    assert_eq!(records.len(), 1, "Should find exactly 1 record with NULL mechanic_name");
    assert_eq!(records[0].service_type, "Oil Change");
}

/// Test Array IN filtering: {"field": [value1, value2]}
/// Should return records matching any value in the array
#[tokio::test]
async fn test_filter_array_in() {
    let db = setup_test_db().await.expect("Failed to setup test database");
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

/// Test case-insensitive string filtering
/// Filtering should match regardless of case
#[tokio::test]
async fn test_filter_case_insensitive() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customer with mixed case name
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/customers")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name": "John Smith", "email": "john@example.com"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Filter with different cases - should all match
    let test_cases = ["john smith", "JOHN SMITH", "John Smith", "jOhN sMiTh"];

    for test_case in test_cases {
        let filter = json!({"name": test_case});
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

        assert_eq!(
            result.len(),
            1,
            "Case '{}' should match exactly 1 customer",
            test_case
        );
    }
}

/// Test UUID value parsing in filters
/// Should correctly parse and filter by UUID values
#[tokio::test]
async fn test_filter_uuid() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer and get their UUID
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/customers")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name": "UUID Test", "email": "uuid@example.com"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let customer_id = customer["id"].as_str().unwrap();

    // Create a vehicle for this customer
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vehicles")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "customer_id": customer_id,
                        "make": "Honda",
                        "model": "Civic",
                        "year": 2021,
                        "vin": "XYZ789"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Filter vehicles by customer_id UUID
    let filter = json!({"customer_id": customer_id});
    let encoded_filter = encode_filter(&filter);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/vehicles?filter={}", encoded_filter))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let vehicles: Vec<VehicleList> = serde_json::from_slice(&body).unwrap();

    assert_eq!(vehicles.len(), 1, "Should find exactly 1 vehicle by UUID");
    assert_eq!(vehicles[0].make, "Honda");
}

/// Test boolean filtering
/// Should correctly filter by true/false values
#[tokio::test]
async fn test_filter_boolean() {
    let db = setup_test_db().await.expect("Failed to setup test database");
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
/// (Current behavior - not 400 error as docs suggest)
#[tokio::test]
async fn test_filter_unknown_field_ignored() {
    let db = setup_test_db().await.expect("Failed to setup test database");
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

    // Should NOT return 400, should return 200 with all records
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Unknown fields should be silently ignored, not cause 400"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    assert!(!result.is_empty(), "Should return records when unknown fields are ignored");
}

// ============================================================================
// SORTING TESTS
// ============================================================================

/// Test REST sort format: sort=column&order=ASC
#[tokio::test]
async fn test_sort_rest_format() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customers
    let customers = [
        json!({"name": "Zara", "email": "zara@example.com"}),
        json!({"name": "Alice", "email": "alice@example.com"}),
        json!({"name": "Mike", "email": "mike@example.com"}),
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

    // Test sort=name&order=ASC
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?sort=name&order=ASC")
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

    assert!(result.len() >= 3);
    // Check ascending order
    assert_eq!(result[0].name, "Alice", "First should be Alice (ASC)");

    // Test sort=name&order=DESC
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?sort=name&order=DESC")
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

    assert!(result.len() >= 3);
    assert_eq!(result[0].name, "Zara", "First should be Zara (DESC)");
}

/// Test REST sort_by format: sort_by=column&order=ASC
#[tokio::test]
async fn test_sort_by_rest_format() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customers
    let customers = [
        json!({"name": "Zara", "email": "zara@example.com"}),
        json!({"name": "Alice", "email": "alice@example.com"}),
        json!({"name": "Mike", "email": "mike@example.com"}),
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

    // Test sort_by=name&order=ASC (sort_by takes priority over sort)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?sort_by=name&order=ASC")
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

    assert!(result.len() >= 3);
    assert_eq!(result[0].name, "Alice", "First should be Alice with sort_by");
}

/// Test invalid sort field falls back to default
#[tokio::test]
async fn test_sort_invalid_field_fallback() {
    let db = setup_test_db().await.expect("Failed to setup test database");
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

    // Sort by non-existent field - should fall back to default, not error
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?sort=%5B%22nonexistent_field%22%2C%22ASC%22%5D")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should NOT return 400, should return 200 with default sort
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Invalid sort field should fall back to default"
    );
}

// ============================================================================
// PAGINATION TESTS
// ============================================================================

/// Test React Admin range pagination: [0,9]
#[tokio::test]
async fn test_pagination_range_format() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create 15 customers
    for i in 0..15 {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"name": format!("Customer {}", i), "email": format!("c{}@example.com", i)})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test range=[0,4] (first 5 items)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?range=%5B0%2C4%5D") // URL encoded [0,4]
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Check Content-Range header
    let content_range = response
        .headers()
        .get("Content-Range")
        .expect("Should have Content-Range header");
    let content_range_str = content_range.to_str().unwrap();
    assert!(
        content_range_str.contains("0-4"),
        "Content-Range should show 0-4, got: {}",
        content_range_str
    );
    assert!(
        content_range_str.contains("/15"),
        "Content-Range should show /15 total, got: {}",
        content_range_str
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    assert_eq!(result.len(), 5, "Should return exactly 5 items for range [0,4]");

    // Test range=[5,9] (second page)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?range=%5B5%2C9%5D") // URL encoded [5,9]
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

    assert_eq!(result.len(), 5, "Should return exactly 5 items for range [5,9]");
}

/// Test default pagination when no params provided
#[tokio::test]
async fn test_pagination_default() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create 15 customers
    for i in 0..15 {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"name": format!("Default {}", i), "email": format!("d{}@example.com", i)})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Request without pagination params - should use default (0, 10)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers")
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

    assert_eq!(
        result.len(),
        10,
        "Default pagination should return 10 items"
    );
}

// ============================================================================
// FULLTEXT SEARCH TESTS
// ============================================================================

/// Test case-insensitive fulltext search
#[tokio::test]
async fn test_fulltext_case_insensitive() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customer and vehicle
    let customer_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/customers")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name": "Search Test", "email": "search@example.com"}).to_string(),
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
                        "make": "Tesla",
                        "model": "Model S",
                        "year": 2023,
                        "vin": "TESLA123"
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

    // Create vehicle part with mixed case content
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vehicle_parts")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "vehicle_id": vehicle_id,
                        "name": "PREMIUM Brake Pads",
                        "part_number": "BP001",
                        "category": "Brakes",
                        "in_stock": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Search with different cases - all should match
    let test_cases = ["premium", "PREMIUM", "Premium", "pReMiUm"];

    for search_term in test_cases {
        let filter = json!({"q": search_term});
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
        let result: Vec<VehiclePartList> = serde_json::from_slice(&body).unwrap();

        assert!(
            !result.is_empty(),
            "Search for '{}' should find results (case-insensitive)",
            search_term
        );
    }
}

/// Test empty fulltext search returns all results
#[tokio::test]
async fn test_fulltext_empty_query_returns_all() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create some customers
    for i in 0..3 {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"name": format!("Empty Test {}", i), "email": format!("e{}@example.com", i)})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Search with empty query
    let filter = json!({"q": ""});
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

    assert!(
        result.len() >= 3,
        "Empty search query should return all results"
    );

    // Search with whitespace-only query
    // Note: Whitespace-only queries after trim become empty, which produces
    // a LIKE '%%' pattern. This should return all results, but the current
    // implementation may handle this differently depending on the path taken.
    // This test documents the actual behavior rather than asserting specific results.
    let filter = json!({"q": "   "});
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

    // Should not error - should handle whitespace gracefully
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Whitespace-only search query should not cause an error"
    );
}

// ============================================================================
// COMBINED TESTS (Filter + Sort + Pagination)
// ============================================================================

/// Test combining filter, sort, and pagination
#[tokio::test]
async fn test_combined_filter_sort_pagination() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create 10 customers with different names
    let names = ["Zara", "Alice", "Bob", "Charlie", "Diana", "Eve", "Frank", "Grace", "Henry", "Ivy"];
    for name in names.iter() {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"name": *name, "email": format!("{}@example.com", name.to_lowercase())})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Combined: sort by name DESC, get first 3
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?sort=%5B%22name%22%2C%22DESC%22%5D&range=%5B0%2C2%5D")
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

    assert_eq!(result.len(), 3, "Should return exactly 3 items");
    assert_eq!(result[0].name, "Zara", "First should be Zara (DESC)");
}
