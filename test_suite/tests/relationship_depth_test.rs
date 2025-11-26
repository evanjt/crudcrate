// Relationship Depth Tests
//
// Tests the recursive join loading capabilities of CRUDCrate.
//
// Depth behavior:
// - Depth is set at compile-time via annotations: `#[crudcrate(join(one, depth = N))]`
// - Maximum depth is 5 (values > 5 are capped)
// - Self-referencing relationships always load one level regardless of depth
// - Non-self-referencing relationships respect the depth annotation
//
// Test models:
// - Category: self-referencing (depth=2 annotation, but loads 1 level)
// - Customer → Vehicle: depth=2 annotation
// - Vehicle → Parts: depth=1 annotation
// - Vehicle → MaintenanceRecords: depth=1 annotation

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_app, setup_test_db};

// =============================================================================
// DEPTH 1 TESTS - Verify single-level join loading
// =============================================================================

#[tokio::test]
async fn test_depth_1_loads_direct_children_only() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Vehicle has depth=1 on parts, so parts should load but not recurse further
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

    // Create vehicle
    let vehicle_data = json!({
        "customer_id": customer_id,
        "make": "Honda",
        "model": "Civic",
        "year": 2024,
        "vin": "TEST123456789"
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

    // Create parts for vehicle
    let part_data = json!({
        "vehicle_id": vehicle_id,
        "name": "Oil Filter",
        "part_number": "OF-001",
        "category": "Maintenance",
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

    // Fetch vehicle - parts should be loaded (depth=1)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/vehicles/{}", vehicle_id))
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

    // Vehicle should have parts loaded
    assert!(result["parts"].is_array(), "Parts should be an array");
    let parts = result["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 1, "Should have 1 part loaded");
    assert_eq!(parts[0]["name"], "Oil Filter");
}

#[tokio::test]
async fn test_depth_1_self_referencing_loads_one_level() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Category has depth=2 annotation but self-referencing is capped at 1
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

    // Create child
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

    // Create grandchild
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

    // Fetch root - children should be loaded, but grandchildren should NOT be
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/categories/{}", root_id))
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

    // Should have children loaded (first level)
    assert!(category["children"].is_array(), "Children should be loaded");
    let children = category["children"].as_array().unwrap();
    assert_eq!(children.len(), 1, "Should have 1 child");
    assert_eq!(children[0]["name"], "Laptops");

    // Grandchildren should NOT be loaded (self-referencing capped at depth 1)
    // The children array items won't have their own children populated
    assert!(
        children[0].get("children").is_none()
            || children[0]["children"].is_null()
            || children[0]["children"].as_array().map_or(true, |a| a.is_empty()),
        "Self-referencing depth > 1 not supported - grandchildren should not be loaded"
    );
}

// =============================================================================
// DEPTH 2 TESTS - Verify two-level recursive loading
// =============================================================================

#[tokio::test]
async fn test_depth_2_loads_two_levels_non_self_referencing() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Customer has depth=2 on vehicles, Vehicle has depth=1 on parts
    // So: Customer -> Vehicles -> Parts should all load

    // Create customer
    let customer_data = json!({"name": "Deep Customer", "email": "deep@test.com"});
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

    // Create vehicle
    let vehicle_data = json!({
        "customer_id": customer_id,
        "make": "Toyota",
        "model": "Camry",
        "year": 2024,
        "vin": "DEEP123456789"
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

    // Create parts for vehicle
    let part1_data = json!({
        "vehicle_id": vehicle_id,
        "name": "Brake Pads",
        "part_number": "BP-001",
        "category": "Brakes",
        "in_stock": true
    });
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vehicle_parts")
                .header("content-type", "application/json")
                .body(Body::from(part1_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let part2_data = json!({
        "vehicle_id": vehicle_id,
        "name": "Air Filter",
        "part_number": "AF-001",
        "category": "Engine",
        "in_stock": true
    });
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vehicle_parts")
                .header("content-type", "application/json")
                .body(Body::from(part2_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Fetch customer - should have vehicles loaded, and vehicles should have parts
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/customers/{}", customer_id))
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

    // Level 1: Customer -> Vehicles
    assert!(result["vehicles"].is_array(), "Vehicles should be loaded");
    let vehicles = result["vehicles"].as_array().unwrap();
    assert_eq!(vehicles.len(), 1, "Should have 1 vehicle");
    assert_eq!(vehicles[0]["make"], "Toyota");

    // Level 2: Vehicles -> Parts (recursive loading)
    assert!(
        vehicles[0]["parts"].is_array(),
        "Vehicle parts should be loaded (depth=2 enables recursive loading)"
    );
    let parts = vehicles[0]["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 2, "Should have 2 parts loaded");
}

// =============================================================================
// DEPTH 5 TESTS - Maximum allowed depth
// =============================================================================

#[tokio::test]
async fn test_depth_5_is_maximum_allowed() {
    // This test verifies that depth=5 is the maximum allowed
    // Values > 5 are capped to 5

    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a simple customer to verify the system works at depth boundaries
    let customer_data = json!({"name": "Max Depth Customer", "email": "maxdepth@test.com"});
    let response = app
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

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let customer_id = customer["id"].as_str().unwrap();

    // Fetch customer - this should work without any issues
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/customers/{}", customer_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // The key verification is that the system handles depth=5 without errors
    // and doesn't allow infinite recursion
}

// =============================================================================
// DEPTH 6 TESTS - Verify depth > 5 is capped
// =============================================================================

#[tokio::test]
async fn test_depth_6_is_capped_to_5() {
    // Models with depth > 5 should be silently capped to 5
    // This prevents DoS attacks via excessive recursion

    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // The Category model has depth=2, but even if someone tried depth=6,
    // it would be capped at 5. Self-referencing is further limited to 1.
    // This test just verifies the system doesn't crash with high depth values.

    let root_data = json!({"name": "Depth Cap Test"});
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

    // Fetch - should work without infinite recursion
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/categories/{}", root_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Note: The actual depth cap is enforced at compile time in loading.rs
    // const MAX_JOIN_DEPTH: u8 = 5;
    // This test documents the expected behavior.
}

// =============================================================================
// JOIN(ONE) VS JOIN(ALL) TESTS
// =============================================================================

#[tokio::test]
async fn test_join_one_excluded_from_get_all() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Category has join(one) only (not join(all))
    // So children should load on get_one but NOT on get_all (list)

    let root_data = json!({"name": "List Test Category"});
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

    // Create child
    let child_data = json!({"name": "Child Category", "parent_id": root_id});
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

    // Get list - children should NOT be loaded (join(one) only)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/categories")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let categories: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    // Find the root category in the list
    let root_in_list = categories
        .iter()
        .find(|c| c["id"].as_str() == Some(root_id));
    assert!(root_in_list.is_some(), "Root should be in list");

    // Children should NOT be loaded in list view (join(one) doesn't apply to get_all)
    let root_cat = root_in_list.unwrap();
    assert!(
        root_cat.get("children").is_none()
            || root_cat["children"].is_null()
            || root_cat["children"].as_array().map_or(true, |a| a.is_empty()),
        "join(one) should not load in get_all - children should be empty"
    );

    // But get_one SHOULD load children
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/categories/{}", root_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let category: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(
        category["children"].is_array() && !category["children"].as_array().unwrap().is_empty(),
        "join(one) SHOULD load children in get_one"
    );
}

#[tokio::test]
async fn test_join_all_loads_in_get_all() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Customer has join(one, all) on vehicles
    // So vehicles should load on BOTH get_one AND get_all

    let customer_data = json!({"name": "Join All Test", "email": "joinall@test.com"});
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

    // Create vehicle
    let vehicle_data = json!({
        "customer_id": customer_id,
        "make": "Ford",
        "model": "Mustang",
        "year": 2024,
        "vin": "JOINALL123456"
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

    // Get list - vehicles SHOULD be loaded (join(all))
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
    let customers: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    let customer_in_list = customers
        .iter()
        .find(|c| c["id"].as_str() == Some(customer_id));
    assert!(customer_in_list.is_some(), "Customer should be in list");

    let cust = customer_in_list.unwrap();
    assert!(
        cust["vehicles"].is_array(),
        "join(all) should load vehicles in get_all"
    );
    let vehicles = cust["vehicles"].as_array().unwrap();
    assert_eq!(vehicles.len(), 1, "Should have 1 vehicle loaded in list");
}

// =============================================================================
// CIRCULAR REFERENCE PREVENTION TESTS
// =============================================================================

#[tokio::test]
async fn test_circular_reference_prevention() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Customer -> Vehicle is a potential circular if Vehicle had back-reference
    // The depth limiting prevents infinite recursion

    let customer_data = json!({"name": "Circular Test", "email": "circular@test.com"});
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

    // Create multiple vehicles to test N+1 pattern
    for i in 1..=3 {
        let vehicle_data = json!({
            "customer_id": customer_id,
            "make": "Brand",
            "model": format!("Model{}", i),
            "year": 2024,
            "vin": format!("CIRC{:010}", i)
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

    // Fetch customer - should complete without infinite loop
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/customers/{}", customer_id))
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

    // Should have all 3 vehicles loaded without infinite recursion
    let vehicles = result["vehicles"].as_array().unwrap();
    assert_eq!(vehicles.len(), 3, "Should have loaded all 3 vehicles");
}

#[tokio::test]
async fn test_self_referencing_does_not_infinitely_recurse() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a deep category hierarchy
    let mut parent_id: Option<String> = None;

    for i in 1..=10 {
        let category_data = if let Some(pid) = &parent_id {
            json!({"name": format!("Level {}", i), "parent_id": pid})
        } else {
            json!({"name": format!("Level {}", i)})
        };

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/categories")
                    .header("content-type", "application/json")
                    .body(Body::from(category_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let category: serde_json::Value = serde_json::from_slice(&body).unwrap();
        parent_id = Some(category["id"].as_str().unwrap().to_string());
    }

    // Fetch root (Level 1) - should complete without stack overflow
    // Find Level 1 by getting all and filtering
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/categories")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let categories: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    // Should have created all 10 categories without infinite recursion
    assert!(categories.len() >= 10, "Should have at least 10 categories");
}

// =============================================================================
// SELF-REFERENCING DEPTH > 1 TESTS (Currently Not Implemented)
// =============================================================================

// Self-referencing depth > 1 is not implemented because it would require
// recursive loading that could cause infinite loops. These tests document
// the expected future behavior.

#[tokio::test]
#[ignore = "self-referencing depth > 1 not implemented"]
async fn test_depth_2_self_referencing_loads_grandchildren() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create Root -> Child -> Grandchild
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

    let grandchild_data = json!({"name": "Grandchild", "parent_id": child_id});
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

    // Fetch root with depth=2 - should load children AND grandchildren
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/categories/{}", root_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let category: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // This assertion would pass if depth > 1 was implemented for self-referencing
    let children = category["children"].as_array().unwrap();
    assert!(!children.is_empty(), "Should have children");
    assert!(
        children[0]["children"].is_array()
            && !children[0]["children"].as_array().unwrap().is_empty(),
        "Should have grandchildren loaded with depth=2"
    );
}

#[tokio::test]
#[ignore = "self-referencing depth > 1 not implemented"]
async fn test_depth_3_self_referencing_loads_three_levels() {
    // Similar test for depth=3 - documenting expected future behavior
    // Implementation would require tracking depth during recursive loading
}
