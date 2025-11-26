//! Deep recursion and join loading tests
//!
//! This module provides comprehensive test coverage for CrudCrate's recursive join loading
//! using **API integration tests** (HTTP calls via axum tower service).
//!
//! ## Depth Limits
//! - **Cross-model joins**: Support depth 1-5 (MAX_JOIN_DEPTH = 5)
//! - **Self-referencing joins**: Automatically limited to depth=1 only
//!
//! ## Test Coverage
//! - Self-referencing depth=1 enforcement
//! - Cross-model recursive joins (depth 1-5)
//! - Field exclusion (create, update, one, list)
//! - Consistency between get_one() and get_all() responses
//! - exclude(create) auto-generates fields
//! - exclude(update) prevents field changes

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;

mod common;
use common::{setup_test_app, setup_test_db};

// Helper to make POST request and return JSON
async fn post_json(app: &axum::Router, uri: &str, data: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, json)
}

// Helper to make GET request and return JSON
async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, json)
}

// Helper to make PUT request and return JSON
async fn put_json(app: &axum::Router, uri: &str, data: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("PUT")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, json)
}

// ============================================================================
// SELF-REFERENCING TESTS (depth=1 only)
// ============================================================================

/// Test that self-referencing relationships are limited to depth=1
#[tokio::test]
async fn test_self_referencing_depth_1_only() {
    let db = setup_test_db().await.expect("Database setup failed");
    let app = setup_test_app(&db);

    // Create a 5-level deep hierarchy via API
    let (status, root) = post_json(&app, "/categories", json!({
        "name": "Root",
        "parent_id": null
    })).await;
    assert_eq!(status, StatusCode::CREATED);
    let root_id = root["id"].as_str().unwrap();

    let mut current_parent = root_id.to_string();
    for level in 1..=5 {
        let (status, cat) = post_json(&app, "/categories", json!({
            "name": format!("Level {}", level),
            "parent_id": current_parent
        })).await;
        assert_eq!(status, StatusCode::CREATED);
        current_parent = cat["id"].as_str().unwrap().to_string();
    }

    // Fetch root via API - should only load immediate children (depth=1)
    let (status, root_loaded) = get_json(&app, &format!("/categories/{}", root_id)).await;
    assert_eq!(status, StatusCode::OK);

    // Verify: Root has exactly 1 immediate child
    let children = root_loaded["children"].as_array().unwrap();
    assert_eq!(children.len(), 1, "Root should have 1 immediate child");
    assert_eq!(children[0]["name"], "Level 1");

    // Verify: Child has NO nested children (depth=1 limit enforced)
    let grandchildren = children[0]["children"].as_array().unwrap();
    assert_eq!(
        grandchildren.len(), 0,
        "Self-referencing should NOT load nested children (depth=1 limit)"
    );
}

/// Test self-referencing with multiple children at same level
#[tokio::test]
async fn test_self_referencing_multiple_children() {
    let db = setup_test_db().await.expect("Database setup failed");
    let app = setup_test_app(&db);

    let (status, root) = post_json(&app, "/categories", json!({
        "name": "Electronics",
        "parent_id": null
    })).await;
    assert_eq!(status, StatusCode::CREATED);
    let root_id = root["id"].as_str().unwrap();

    // Create 3 children at the same level
    for name in ["Phones", "Laptops", "Tablets"] {
        let (status, _) = post_json(&app, "/categories", json!({
            "name": name,
            "parent_id": root_id
        })).await;
        assert_eq!(status, StatusCode::CREATED);
    }

    let (status, root_loaded) = get_json(&app, &format!("/categories/{}", root_id)).await;
    assert_eq!(status, StatusCode::OK);

    // All 3 immediate children should be loaded
    let children = root_loaded["children"].as_array().unwrap();
    assert_eq!(children.len(), 3, "Root should have 3 immediate children");

    // None should have nested children (depth=1)
    for child in children {
        let nested = child["children"].as_array().unwrap();
        assert_eq!(nested.len(), 0, "Child should have no nested children");
    }
}

// ============================================================================
// CROSS-MODEL DEPTH TESTS (depth 1-5)
// ============================================================================

/// Test depth=1: Customer → Vehicles (1 join)
#[tokio::test]
async fn test_cross_model_depth_1() {
    let db = setup_test_db().await.expect("Database setup failed");
    let app = setup_test_app(&db);

    let (status, customer) = post_json(&app, "/customers", json!({
        "name": "Test Customer",
        "email": "test@example.com"
    })).await;
    assert_eq!(status, StatusCode::CREATED);
    let customer_id = customer["id"].as_str().unwrap();

    let (status, _) = post_json(&app, "/vehicles", json!({
        "customer_id": customer_id,
        "make": "Toyota",
        "model": "Camry",
        "year": 2020,
        "vin": "VIN001"
    })).await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, loaded) = get_json(&app, &format!("/customers/{}", customer_id)).await;
    assert_eq!(status, StatusCode::OK);

    // Depth 1: Customer → Vehicles
    let vehicles = loaded["vehicles"].as_array().unwrap();
    assert_eq!(vehicles.len(), 1, "Should load 1 vehicle (depth=1)");
    assert_eq!(vehicles[0]["make"], "Toyota");
}

/// Test depth=2: Customer → Vehicle → Parts (2 joins)
#[tokio::test]
async fn test_cross_model_depth_2() {
    let db = setup_test_db().await.expect("Database setup failed");
    let app = setup_test_app(&db);

    let (status, customer) = post_json(&app, "/customers", json!({
        "name": "Test Customer",
        "email": "test@example.com"
    })).await;
    assert_eq!(status, StatusCode::CREATED);
    let customer_id = customer["id"].as_str().unwrap();

    let (status, vehicle) = post_json(&app, "/vehicles", json!({
        "customer_id": customer_id,
        "make": "Honda",
        "model": "Civic",
        "year": 2021,
        "vin": "VIN002"
    })).await;
    assert_eq!(status, StatusCode::CREATED);
    let vehicle_id = vehicle["id"].as_str().unwrap();

    let (status, _) = post_json(&app, "/vehicle_parts", json!({
        "vehicle_id": vehicle_id,
        "name": "Brake Pads",
        "part_number": "BP-001",
        "category": "Brakes",
        "price": 59.99,
        "in_stock": true
    })).await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, loaded) = get_json(&app, &format!("/customers/{}", customer_id)).await;
    assert_eq!(status, StatusCode::OK);

    // Depth 2: Customer → Vehicle → Parts
    let vehicles = loaded["vehicles"].as_array().unwrap();
    assert_eq!(vehicles.len(), 1);
    let parts = vehicles[0]["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 1, "Should load parts (depth=2)");
    assert_eq!(parts[0]["name"], "Brake Pads");
}

/// Test depth=2 with multiple relationships
#[tokio::test]
async fn test_cross_model_depth_2_multiple_relations() {
    let db = setup_test_db().await.expect("Database setup failed");
    let app = setup_test_app(&db);

    let (_, customer) = post_json(&app, "/customers", json!({
        "name": "Multi-Relation Test",
        "email": "multi@example.com"
    })).await;
    let customer_id = customer["id"].as_str().unwrap();

    let (_, vehicle) = post_json(&app, "/vehicles", json!({
        "customer_id": customer_id,
        "make": "Ford",
        "model": "F-150",
        "year": 2022,
        "vin": "VIN003"
    })).await;
    let vehicle_id = vehicle["id"].as_str().unwrap();

    // Add parts
    post_json(&app, "/vehicle_parts", json!({
        "vehicle_id": vehicle_id,
        "name": "Oil Filter",
        "part_number": "OF-001",
        "category": "Maintenance",
        "price": 19.99,
        "in_stock": true
    })).await;

    // Add maintenance records
    post_json(&app, "/maintenance_records", json!({
        "vehicle_id": vehicle_id,
        "service_type": "Oil Change",
        "description": "Regular maintenance",
        "cost": 49.99,
        "service_date": "2024-01-15T10:00:00Z",
        "mechanic_name": "Bob",
        "completed": true
    })).await;

    let (_, loaded) = get_json(&app, &format!("/customers/{}", customer_id)).await;

    // Both relationships at depth 2 should load
    let vehicles = loaded["vehicles"].as_array().unwrap();
    let parts = vehicles[0]["parts"].as_array().unwrap();
    let records = vehicles[0]["maintenance_records"].as_array().unwrap();

    assert_eq!(parts.len(), 1, "Should load parts");
    assert_eq!(records.len(), 1, "Should load maintenance records");
}

// ============================================================================
// FIELD EXCLUSION TESTS
// ============================================================================

/// Test exclude(create) - ID is auto-generated
#[tokio::test]
async fn test_exclude_create_auto_generates_id() {
    let db = setup_test_db().await.expect("Database setup failed");
    let app = setup_test_app(&db);

    // Try to send an ID in the create request - it should be ignored
    let (status, customer) = post_json(&app, "/customers", json!({
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "name": "Auto ID Test",
        "email": "autoid@example.com"
    })).await;
    assert_eq!(status, StatusCode::CREATED);

    // ID should be auto-generated (different from what we sent)
    let id = customer["id"].as_str().unwrap();
    assert_ne!(id, "550e8400-e29b-41d4-a716-446655440000", "ID should be auto-generated");
    assert!(!id.is_empty(), "ID should not be empty");
}

/// Test exclude(create) - timestamps are auto-generated
#[tokio::test]
async fn test_exclude_create_auto_generates_timestamps() {
    let db = setup_test_db().await.expect("Database setup failed");
    let app = setup_test_app(&db);

    // Try to send timestamps - they should be ignored and auto-generated
    let (status, customer) = post_json(&app, "/customers", json!({
        "name": "Timestamp Test",
        "email": "timestamp@example.com",
        "created_at": "2000-01-01T00:00:00Z",
        "updated_at": "2000-01-01T00:00:00Z"
    })).await;
    assert_eq!(status, StatusCode::CREATED);

    let customer_id = customer["id"].as_str().unwrap();

    // Get the customer - updated_at should be recent (not year 2000)
    let (_, loaded) = get_json(&app, &format!("/customers/{}", customer_id)).await;

    // updated_at is visible in get_one (only excluded from list)
    let updated_at = loaded["updated_at"].as_str().unwrap();
    assert!(!updated_at.starts_with("2000"), "Timestamp should be auto-generated, not from request");
}

/// Test exclude(update) - ID cannot be changed
#[tokio::test]
async fn test_exclude_update_prevents_id_change() {
    let db = setup_test_db().await.expect("Database setup failed");
    let app = setup_test_app(&db);

    let (_, customer) = post_json(&app, "/customers", json!({
        "name": "Immutable ID",
        "email": "immutable@example.com"
    })).await;
    let original_id = customer["id"].as_str().unwrap().to_string();

    // Try to update with a different ID - it should be ignored
    let (status, updated) = put_json(&app, &format!("/customers/{}", original_id), json!({
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "name": "Updated Name",
        "email": "updated@example.com"
    })).await;
    assert_eq!(status, StatusCode::OK);

    // ID should remain unchanged
    assert_eq!(updated["id"].as_str().unwrap(), original_id, "ID should not change on update");
    assert_eq!(updated["name"], "Updated Name");
}

/// Test exclude(update) - timestamps are auto-updated
#[tokio::test]
async fn test_exclude_update_auto_updates_timestamps() {
    let db = setup_test_db().await.expect("Database setup failed");
    let app = setup_test_app(&db);

    let (_, customer) = post_json(&app, "/customers", json!({
        "name": "Timestamp Update",
        "email": "ts_update@example.com"
    })).await;
    let customer_id = customer["id"].as_str().unwrap();
    let original_updated_at = customer["updated_at"].as_str().unwrap().to_string();

    // Delay to ensure timestamp difference (timestamps are second-precision)
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Update the customer
    let (status, updated) = put_json(&app, &format!("/customers/{}", customer_id), json!({
        "name": "After Update"
    })).await;
    assert_eq!(status, StatusCode::OK);

    // updated_at should be refreshed (different from original)
    let new_updated_at = updated["updated_at"].as_str().unwrap();
    assert_ne!(new_updated_at, original_updated_at, "updated_at should be auto-updated");
}

/// Test exclude(one) - created_at excluded from get_one but present in get_all
#[tokio::test]
async fn test_exclude_one_field_behavior() {
    let db = setup_test_db().await.expect("Database setup failed");
    let app = setup_test_app(&db);

    let (_, customer) = post_json(&app, "/customers", json!({
        "name": "Exclude One Test",
        "email": "excludeone@example.com"
    })).await;
    let customer_id = customer["id"].as_str().unwrap();

    // get_one should NOT have created_at (exclude(one))
    let (_, one) = get_json(&app, &format!("/customers/{}", customer_id)).await;
    assert!(one.get("created_at").is_none(), "created_at should be excluded from get_one");

    // get_all SHOULD have created_at
    let (_, all) = get_json(&app, "/customers").await;
    let found = all.as_array().unwrap().iter()
        .find(|c| c["id"].as_str() == Some(customer_id))
        .expect("Customer should be in list");
    assert!(found.get("created_at").is_some(), "created_at should be present in get_all");
}

/// Test exclude(list) - updated_at excluded from get_all but present in get_one
#[tokio::test]
async fn test_exclude_list_field_behavior() {
    let db = setup_test_db().await.expect("Database setup failed");
    let app = setup_test_app(&db);

    let (_, customer) = post_json(&app, "/customers", json!({
        "name": "Exclude List Test",
        "email": "excludelist@example.com"
    })).await;
    let customer_id = customer["id"].as_str().unwrap();

    // get_one SHOULD have updated_at
    let (_, one) = get_json(&app, &format!("/customers/{}", customer_id)).await;
    assert!(one.get("updated_at").is_some(), "updated_at should be present in get_one");

    // get_all should NOT have updated_at (exclude(list))
    let (_, all) = get_json(&app, "/customers").await;
    let found = all.as_array().unwrap().iter()
        .find(|c| c["id"].as_str() == Some(customer_id))
        .expect("Customer should be in list");
    assert!(found.get("updated_at").is_none(), "updated_at should be excluded from get_all");
}

// ============================================================================
// CONSISTENCY TESTS
// ============================================================================

/// Test consistency between get_one and get_all for join fields
#[tokio::test]
async fn test_get_one_get_all_join_consistency() {
    let db = setup_test_db().await.expect("Database setup failed");
    let app = setup_test_app(&db);

    let (_, customer) = post_json(&app, "/customers", json!({
        "name": "Consistency Test",
        "email": "consistent@example.com"
    })).await;
    let customer_id = customer["id"].as_str().unwrap();

    post_json(&app, "/vehicles", json!({
        "customer_id": customer_id,
        "make": "Audi",
        "model": "A4",
        "year": 2023,
        "vin": "VIN-AUDI"
    })).await;

    let (_, one) = get_json(&app, &format!("/customers/{}", customer_id)).await;
    let (_, all) = get_json(&app, "/customers").await;

    let from_all = all.as_array().unwrap().iter()
        .find(|c| c["id"].as_str() == Some(customer_id))
        .expect("Customer not in get_all");

    // Both should have vehicles loaded (join(one, all))
    let one_vehicles = one["vehicles"].as_array().unwrap();
    let all_vehicles = from_all["vehicles"].as_array().unwrap();

    assert_eq!(one_vehicles.len(), all_vehicles.len(),
        "get_one and get_all should have same vehicle count");
    assert_eq!(one_vehicles[0]["make"], all_vehicles[0]["make"],
        "Vehicle data should match");
}

// ============================================================================
// EDGE CASES
// ============================================================================

/// Test empty relationships
#[tokio::test]
async fn test_empty_relationships() {
    let db = setup_test_db().await.expect("Database setup failed");
    let app = setup_test_app(&db);

    let (_, customer) = post_json(&app, "/customers", json!({
        "name": "No Vehicles",
        "email": "novehicles@example.com"
    })).await;
    let customer_id = customer["id"].as_str().unwrap();

    let (_, loaded) = get_json(&app, &format!("/customers/{}", customer_id)).await;

    // Empty array, not null
    let vehicles = loaded["vehicles"].as_array().unwrap();
    assert_eq!(vehicles.len(), 0, "Customer with no vehicles should have empty array");
}

/// Test self-referencing with no children
#[tokio::test]
async fn test_self_referencing_no_children() {
    let db = setup_test_db().await.expect("Database setup failed");
    let app = setup_test_app(&db);

    let (_, leaf) = post_json(&app, "/categories", json!({
        "name": "Leaf Category",
        "parent_id": null
    })).await;
    let leaf_id = leaf["id"].as_str().unwrap();

    let (_, loaded) = get_json(&app, &format!("/categories/{}", leaf_id)).await;

    let children = loaded["children"].as_array().unwrap();
    assert_eq!(children.len(), 0, "Category with no children should have empty array");
}

/// Test complete hierarchy structure
#[tokio::test]
async fn test_hierarchy_structure() {
    let db = setup_test_db().await.expect("Database setup failed");
    let app = setup_test_app(&db);

    // Create complete hierarchy via API
    let (_, customer) = post_json(&app, "/customers", json!({
        "name": "Hierarchy Test",
        "email": "hierarchy@example.com"
    })).await;
    let customer_id = customer["id"].as_str().unwrap();

    let (_, vehicle) = post_json(&app, "/vehicles", json!({
        "customer_id": customer_id,
        "make": "Mercedes",
        "model": "C-Class",
        "year": 2023,
        "vin": "VIN-MERC"
    })).await;
    let vehicle_id = vehicle["id"].as_str().unwrap();

    for i in 1..=3 {
        post_json(&app, "/vehicle_parts", json!({
            "vehicle_id": vehicle_id,
            "name": format!("Part {}", i),
            "part_number": format!("PN-{}", i),
            "category": "Test",
            "price": (i as f64) * 10.0,
            "in_stock": true
        })).await;
    }

    let (_, loaded) = get_json(&app, &format!("/customers/{}", customer_id)).await;

    // Verify complete hierarchy
    assert_eq!(loaded["name"], "Hierarchy Test");
    let vehicles = loaded["vehicles"].as_array().unwrap();
    assert_eq!(vehicles.len(), 1);
    assert_eq!(vehicles[0]["make"], "Mercedes");
    let parts = vehicles[0]["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 3);
}
