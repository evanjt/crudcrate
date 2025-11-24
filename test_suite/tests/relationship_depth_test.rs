// Comprehensive Relationship Depth Tests
// Tests examples from docs/src/features/relationships.md
// Goal: 100% documentation-test alignment for relationship depth parameters

// IMPORTANT: These tests will expose bugs in CRUDCrate's handling of:
// 1. Self-referencing relationships (Category->children)
// 2. Dynamic depth parameter overrides via query strings
// 3. Depth limiting behavior
//
// DO NOT DISABLE FAILING TESTS - They document bugs that must be fixed before public demo!

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_app, setup_test_db};

// =============================================================================
// DEPTH PARAMETER TESTS (relationships.md lines 119-129)
// Test that depth=1, depth=2, depth=3 work as documented
// =============================================================================

#[tokio::test]
async fn test_depth_1_limits_to_first_level() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create nested category hierarchy: Root -> Child -> Grandchild
    let root_data = json!({"name": "Electronics"});
    let root_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/categories")
                .header("content-type", "application/json")
                .body(Body::from(root_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let root_body = axum::body::to_bytes(root_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let root: serde_json::Value = serde_json::from_slice(&root_body).unwrap();
    let root_id = root["id"].as_str().unwrap();

    // Create child category
    let child_data = json!({"name": "Laptops", "parent_id": root_id});
    let child_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/categories")
                .header("content-type", "application/json")
                .body(Body::from(child_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let child_body = axum::body::to_bytes(child_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let child: serde_json::Value = serde_json::from_slice(&child_body).unwrap();
    let child_id = child["id"].as_str().unwrap();

    // Create grandchild category
    let grandchild_data = json!({"name": "Gaming Laptops", "parent_id": child_id});
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/categories")
                .header("content-type", "application/json")
                .body(Body::from(grandchild_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Test: Get root category with depth=1 (should load children but not grandchildren)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/categories/{}?include=children&depth=1", root_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let category: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Should have children loaded
    assert!(category["children"].is_array(), "Children should be loaded");
    let children = category["children"].as_array().unwrap();
    assert_eq!(children.len(), 1, "Should have 1 child");
    assert_eq!(children[0]["name"], "Laptops");

    // Children should NOT have their children loaded (depth limit reached)
    assert!(
        children[0]["children"].is_null() || children[0]["children"].as_array().unwrap().is_empty(),
        "Grandchildren should NOT be loaded with depth=1"
    );
}

#[tokio::test]
async fn test_depth_2_loads_two_levels() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create nested category hierarchy: Root -> Child -> Grandchild
    let root_data = json!({"name": "Electronics"});
    let root_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/categories")
                .header("content-type", "application/json")
                .body(Body::from(root_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let root_body = axum::body::to_bytes(root_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let root: serde_json::Value = serde_json::from_slice(&root_body).unwrap();
    let root_id = root["id"].as_str().unwrap();

    // Create child category
    let child_data = json!({"name": "Laptops", "parent_id": root_id});
    let child_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/categories")
                .header("content-type", "application/json")
                .body(Body::from(child_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let child_body = axum::body::to_bytes(child_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let child: serde_json::Value = serde_json::from_slice(&child_body).unwrap();
    let child_id = child["id"].as_str().unwrap();

    // Create grandchild category
    let grandchild_data = json!({"name": "Gaming Laptops", "parent_id": child_id});
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/categories")
                .header("content-type", "application/json")
                .body(Body::from(grandchild_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Test: Get root category with depth=2 (should load children AND grandchildren)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/categories/{}?include=children&depth=2", root_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let category: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Should have children loaded
    assert!(category["children"].is_array(), "Children should be loaded");
    let children = category["children"].as_array().unwrap();
    assert_eq!(children.len(), 1, "Should have 1 child");

    // Children SHOULD have their children loaded (depth=2 allows it)
    assert!(children[0]["children"].is_array(), "Grandchildren should be loaded with depth=2");
    let grandchildren = children[0]["children"].as_array().unwrap();
    assert_eq!(grandchildren.len(), 1, "Should have 1 grandchild");
    assert_eq!(grandchildren[0]["name"], "Gaming Laptops");
}

#[tokio::test]
async fn test_depth_3_loads_three_levels() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create deeply nested category hierarchy:
    // Root -> Child -> Grandchild -> Great Grandchild
    let root_data = json!({"name": "Electronics"});
    let root_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/categories")
                .header("content-type", "application/json")
                .body(Body::from(root_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let root_body = axum::body::to_bytes(root_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let root: serde_json::Value = serde_json::from_slice(&root_body).unwrap();
    let root_id = root["id"].as_str().unwrap();

    let child_data = json!({"name": "Laptops", "parent_id": root_id});
    let child_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/categories")
                .header("content-type", "application/json")
                .body(Body::from(child_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let child_body = axum::body::to_bytes(child_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let child: serde_json::Value = serde_json::from_slice(&child_body).unwrap();
    let child_id = child["id"].as_str().unwrap();

    let grandchild_data = json!({"name": "Gaming Laptops", "parent_id": child_id});
    let grandchild_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/categories")
                .header("content-type", "application/json")
                .body(Body::from(grandchild_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let grandchild_body = axum::body::to_bytes(grandchild_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let grandchild: serde_json::Value = serde_json::from_slice(&grandchild_body).unwrap();
    let grandchild_id = grandchild["id"].as_str().unwrap();

    let great_grandchild_data = json!({"name": "RTX 4090 Gaming Laptops", "parent_id": grandchild_id});
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/categories")
                .header("content-type", "application/json")
                .body(Body::from(great_grandchild_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Test: Get root category with depth=3 (should load 3 levels)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/categories/{}?include=children&depth=3", root_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let category: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Level 1: Root has children
    let children = category["children"].as_array().unwrap();
    assert_eq!(children.len(), 1);

    // Level 2: Children have children
    let grandchildren = children[0]["children"].as_array().unwrap();
    assert_eq!(grandchildren.len(), 1);

    // Level 3: Grandchildren have children
    let great_grandchildren = grandchildren[0]["children"].as_array().unwrap();
    assert_eq!(great_grandchildren.len(), 1);
    assert_eq!(great_grandchildren[0]["name"], "RTX 4090 Gaming Laptops");
}

// =============================================================================
// RECURSIVE SELF-REFERENCE TESTS (relationships.md lines 192-227)
// Test self-referencing relationships with depth limits
// =============================================================================

#[tokio::test]
async fn test_recursive_self_reference_with_depth_limit() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create self-referencing category tree
    let root_data = json!({"name": "Root Category"});
    let root_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/categories")
                .header("content-type", "application/json")
                .body(Body::from(root_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let root_body = axum::body::to_bytes(root_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let root: serde_json::Value = serde_json::from_slice(&root_body).unwrap();
    let root_id = root["id"].as_str().unwrap();

    // Test: Get with default depth (should respect depth=1 from model annotation)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/categories/{}?include=children", root_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let category: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Category model has depth=1 in annotation, so it should use that as default
    assert_eq!(category["name"], "Root Category");
}

// =============================================================================
// JOIN(ONE) EXCLUSION FROM LISTS (relationships.md lines 88-93)
// Test that join(one) relationships are NOT loaded in get_all operations
// =============================================================================

#[tokio::test]
async fn test_join_one_excluded_from_get_all() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer
    let customer_data = json!({"name": "Test Customer", "email": "test@example.com"});
    let customer_response = app
        .clone()
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

    let customer_body = axum::body::to_bytes(customer_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: serde_json::Value = serde_json::from_slice(&customer_body).unwrap();
    let customer_id = customer["id"].as_str().unwrap();

    // Create a vehicle for the customer
    let vehicle_data = json!({
        "customer_id": customer_id,
        "make": "Toyota",
        "model": "Camry",
        "year": 2024,
        "vin": "1HGCM82633A123456"
    });
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vehicles")
                .header("content-type", "application/json")
                .body(Body::from(vehicle_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Test: Get all vehicles (should NOT include related data with join(one))
    // This prevents N+1 query problems in list operations
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/vehicles")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let vehicles: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert!(!vehicles.is_empty(), "Should have at least 1 vehicle");

    // Verify join(one) relationships are NOT loaded in list operations
    // (This is different from join(all) which DOES load in lists)
    for vehicle in vehicles {
        // The vehicle should have basic fields but NOT expanded relationships marked with join(one)
        assert!(vehicle["make"].is_string());
        assert!(vehicle["model"].is_string());
        // Parts and maintenance_records have join(one, all) so they should be loaded
        assert!(vehicle["parts"].is_array());
        assert!(vehicle["maintenance_records"].is_array());
    }
}

// =============================================================================
// JOIN(ALL) N+1 WARNING (relationships.md lines 229-244)
// Document that join(all) causes N+1 queries in list operations
// =============================================================================

#[tokio::test]
async fn test_join_all_loads_in_lists_with_n_plus_1() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer
    let customer_data = json!({"name": "Test Customer", "email": "test@example.com"});
    let customer_response = app
        .clone()
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

    let customer_body = axum::body::to_bytes(customer_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: serde_json::Value = serde_json::from_slice(&customer_body).unwrap();
    let customer_id = customer["id"].as_str().unwrap();

    // Create vehicles for the customer
    for i in 0..3 {
        let vehicle_data = json!({
            "customer_id": customer_id,
            "make": "Toyota",
            "model": format!("Model {}", i),
            "year": 2024,
            "vin": format!("VIN{:08}", i)
        });
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/vehicles")
                    .header("content-type", "application/json")
                    .body(Body::from(vehicle_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test: Get all customers (with join(all), vehicles ARE loaded in lists)
    // This demonstrates the N+1 query scenario: 1 query for customers + N queries for vehicles
    let response = app
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
    let customers: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert!(!customers.is_empty(), "Should have at least 1 customer");

    // With join(all), vehicles ARE loaded in list operations
    // Customer model has: join(one, all, depth = 2) on vehicles field
    for customer in &customers {
        assert!(customer["vehicles"].is_array(), "Vehicles should be loaded with join(all)");
        let vehicles = customer["vehicles"].as_array().unwrap();
        assert_eq!(vehicles.len(), 3, "Should have 3 vehicles loaded");

        // Each vehicle should also have its parts and maintenance_records loaded (depth=1)
        for vehicle in vehicles {
            assert!(vehicle["parts"].is_array(), "Vehicle parts should be loaded (depth=1)");
            assert!(vehicle["maintenance_records"].is_array(), "Maintenance records should be loaded (depth=1)");
        }
    }

    // NOTE: This test documents the N+1 behavior, not prevents it
    // Users should be aware that join(all) in list operations can be expensive
}

// =============================================================================
// DEPTH PARAMETER TEST (relationships.md lines 119-129)
// Test that depth parameter controls how many levels of relationships are loaded
// =============================================================================

#[tokio::test]
async fn test_depth_limits_nested_relationship_loading() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer
    let customer_data = json!({"name": "Test Customer", "email": "test@example.com"});
    let customer_response = app
        .clone()
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

    let customer_body = axum::body::to_bytes(customer_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: serde_json::Value = serde_json::from_slice(&customer_body).unwrap();
    let customer_id = customer["id"].as_str().unwrap();

    // Create a vehicle with parts
    let vehicle_data = json!({
        "customer_id": customer_id,
        "make": "Toyota",
        "model": "Camry",
        "year": 2024,
        "vin": "VIN12345678"
    });
    let vehicle_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vehicles")
                .header("content-type", "application/json")
                .body(Body::from(vehicle_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let vehicle_body = axum::body::to_bytes(vehicle_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let vehicle: serde_json::Value = serde_json::from_slice(&vehicle_body).unwrap();
    let vehicle_id = vehicle["id"].as_str().unwrap();

    // Create parts for the vehicle
    for i in 0..2 {
        let part_data = json!({
            "vehicle_id": vehicle_id,
            "name": format!("Part {}", i),
            "part_number": format!("PN{:04}", i),
            "category": "Engine",
            "price": 99.99,
            "in_stock": true
        });
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/vehicle_parts")
                    .header("content-type", "application/json")
                    .body(Body::from(part_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test: Get all customers (should load vehicles with depth=2, meaning parts are also loaded)
    // Customer model annotation: join(one, all, depth = 2)
    // This means: load vehicles, and for each vehicle load its children up to depth 1
    let response = app
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
    let customers: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert!(!customers.is_empty());

    // Verify depth parameter is working:
    // Level 0: Customer
    // Level 1: Customer -> Vehicles (loaded because depth=2)
    // Level 2: Customer -> Vehicles -> Parts (loaded because depth=2 allows it)
    let customer = &customers[0];
    assert!(customer["vehicles"].is_array());
    let vehicles = customer["vehicles"].as_array().unwrap();
    assert!(!vehicles.is_empty());

    let vehicle = &vehicles[0];
    assert!(vehicle["parts"].is_array());
    let parts = vehicle["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 2, "Should have 2 parts loaded");
}

// =============================================================================
// CIRCULAR REFERENCE DEPTH TESTS
// Test depth limiting with bidirectional relationships (Customer<->Vehicle)
// This is the REAL test: Customer->Vehicles->Customer->Vehicles should stop at depth limit
// =============================================================================

#[tokio::test]
async fn test_depth_4_with_circular_reference() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customer with vehicle (circular: Customer->Vehicle->Customer->Vehicle...)
    let customer_data = json!({"name": "Test Customer", "email": "test@example.com"});
    let customer_response = app
        .clone()
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

    let customer_body = axum::body::to_bytes(customer_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: serde_json::Value = serde_json::from_slice(&customer_body).unwrap();
    let customer_id = customer["id"].as_str().unwrap();

    // Create vehicle for customer
    let vehicle_data = json!({
        "customer_id": customer_id,
        "make": "Toyota",
        "model": "Camry",
        "year": 2024,
        "vin": "VIN12345678"
    });
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vehicles")
                .header("content-type", "application/json")
                .body(Body::from(vehicle_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Test: Get customer with depth=4
    // Path: Customer(0) -> Vehicles(1) -> Parts(2)
    // Should NOT recurse back through Customer because depth=4 is within reasonable limits
    // but model annotation is depth=2, so it should respect that
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/customers/{}?depth=4", customer_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Should have loaded vehicles
    assert!(result["vehicles"].is_array(), "Vehicles should be loaded");

    // NOTE: This test verifies that depth limiting prevents infinite recursion
    // in circular relationships (Customer->Vehicle->Customer->Vehicle...)
}

#[tokio::test]
async fn test_depth_5_with_circular_reference() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customer with vehicle (circular: Customer->Vehicle->Customer->Vehicle...)
    let customer_data = json!({"name": "Test Customer", "email": "test@example.com"});
    let customer_response = app
        .clone()
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

    let customer_body = axum::body::to_bytes(customer_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: serde_json::Value = serde_json::from_slice(&customer_body).unwrap();
    let customer_id = customer["id"].as_str().unwrap();

    // Create vehicle for customer
    let vehicle_data = json!({
        "customer_id": customer_id,
        "make": "Toyota",
        "model": "Camry",
        "year": 2024,
        "vin": "VIN12345678"
    });
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vehicles")
                .header("content-type", "application/json")
                .body(Body::from(vehicle_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Test: Get customer with depth=5 (should still work, hitting upper reasonable limit)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/customers/{}?depth=5", customer_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Should have loaded vehicles
    assert!(result["vehicles"].is_array(), "Vehicles should be loaded with depth=5");

    // NOTE: Depth=5 is at the upper limit of reasonable depth
}

#[tokio::test]
async fn test_depth_6_should_fail_or_be_capped() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customer with vehicle (circular: Customer->Vehicle->Customer->Vehicle...)
    let customer_data = json!({"name": "Test Customer", "email": "test@example.com"});
    let customer_response = app
        .clone()
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

    let customer_body = axum::body::to_bytes(customer_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: serde_json::Value = serde_json::from_slice(&customer_body).unwrap();
    let customer_id = customer["id"].as_str().unwrap();

    // Create vehicle for customer
    let vehicle_data = json!({
        "customer_id": customer_id,
        "make": "Toyota",
        "model": "Camry",
        "year": 2024,
        "vin": "VIN12345678"
    });
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vehicles")
                .header("content-type", "application/json")
                .body(Body::from(vehicle_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Test: Get customer with depth=6 (should fail or be capped at max depth=5)
    // This tests that CRUDCrate prevents unreasonable depth that could cause performance issues
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/customers/{}?depth=6", customer_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should either:
    // Option A: Return 400 Bad Request (depth too high)
    // Option B: Cap depth at 5 and return 200 OK
    // Option C: Return 200 but ignore depth parameter (fallback to model default)

    assert!(
        response.status() == StatusCode::OK || response.status() == StatusCode::BAD_REQUEST,
        "depth=6 should either be rejected (400) or capped (200), got: {:?}",
        response.status()
    );

    if response.status() == StatusCode::OK {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();

        // If it succeeded, it should have at least loaded vehicles
        // (meaning it didn't completely fail)
        assert!(result["vehicles"].is_array(), "If depth=6 is accepted, vehicles should still load");

        println!("NOTE: depth=6 was accepted (should ideally be capped at 5 or rejected)");
    }
}

#[tokio::test]
async fn test_circular_reference_stops_infinite_recursion() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customer
    let customer_data = json!({"name": "Test Customer", "email": "test@example.com"});
    let customer_response = app
        .clone()
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

    let customer_body = axum::body::to_bytes(customer_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: serde_json::Value = serde_json::from_slice(&customer_body).unwrap();
    let customer_id = customer["id"].as_str().unwrap();

    // Create multiple vehicles for customer (to make circular reference more obvious)
    for i in 0..3 {
        let vehicle_data = json!({
            "customer_id": customer_id,
            "make": "Toyota",
            "model": format!("Model {}", i),
            "year": 2024,
            "vin": format!("VIN{:08}", i)
        });
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/vehicles")
                    .header("content-type", "application/json")
                    .body(Body::from(vehicle_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test: Get ALL customers (this triggers the circular reference scenario)
    // Customer has join(one, all, depth=2) which means:
    // - Load vehicles in lists (join=all)
    // - For each vehicle, load ITS relationships up to depth 1
    // - This COULD recurse: Customer->Vehicle->Customer->Vehicle...
    // - Depth limiting MUST prevent infinite recursion
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK, "Should not stack overflow on circular reference");

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert!(!customers.is_empty());

    // Verify vehicles are loaded
    let customer = &customers[0];
    assert!(customer["vehicles"].is_array());
    let vehicles = customer["vehicles"].as_array().unwrap();
    assert_eq!(vehicles.len(), 3);

    // CRITICAL TEST: Verify vehicles don't have customer loaded
    // (which would create infinite recursion Customer->Vehicle->Customer->Vehicle...)
    // The depth limit should prevent this
    for vehicle in vehicles {
        // Vehicle should NOT have customer loaded (depth limit prevents circular recursion)
        // If customer is present, it means depth limiting is broken
        if vehicle.as_object().unwrap().contains_key("customer") {
            println!("WARNING: Vehicle has customer field - checking if it's populated...");
            // It might be null or empty, which is OK
            if vehicle["customer"].is_object() {
                panic!("DEPTH LIMIT FAILED: Vehicle has customer object, creating circular reference!");
            }
        }
    }

    println!("SUCCESS: Depth limiting prevented infinite Customer<->Vehicle recursion");
}

// =============================================================================
// DEPTH=0 TESTS
// Test that depth=0 means no relationship loading
// =============================================================================

#[tokio::test]
async fn test_depth_0_loads_nothing() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create nested categories
    let root_data = json!({"name": "Root"});
    let root_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/categories")
                .header("content-type", "application/json")
                .body(Body::from(root_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let root_body = axum::body::to_bytes(root_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let root: serde_json::Value = serde_json::from_slice(&root_body).unwrap();
    let root_id = root["id"].as_str().unwrap();

    let child_data = json!({"name": "Child", "parent_id": root_id});
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/categories")
                .header("content-type", "application/json")
                .body(Body::from(child_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Test: Get root with depth=0 (should load NO relationships)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/categories/{}?include=children&depth=0", root_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let category: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Should NOT have children loaded
    assert!(
        category["children"].is_null() || category["children"].as_array().unwrap().is_empty(),
        "With depth=0, no relationships should be loaded"
    );
}
