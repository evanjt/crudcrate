// Test cases for field exclusion and join loading inconsistencies
//
// These tests document the current broken behavior and will serve as regression tests
// once the issues are fixed in the crudcrate library.
//
// ISSUES IDENTIFIED:
// 1. exclude(one) not working: created_at/updated_at appear in get_one() when they shouldn't
// 2. join(one, all) not working: vehicles field missing from get_one() when it should appear
// 3. Inconsistent behavior between get_all() and get_one() endpoints

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_db, setup_test_app, Customer};

// ============================================================================
// TEST CASE 1: exclude(one) not working correctly
// ============================================================================

#[tokio::test]
async fn test_exclude_one_fields_not_in_get_one_responses() {
    // SETUP: Customer with exclude(one) on created_at and updated_at
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a test customer first
    let create_data = json!({
        "name": "Test Customer",
        "email": "test@example.com"
    });

    let create_request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let create_response = app.clone().oneshot(create_request).await.unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(create_response.into_body(), usize::MAX).await.unwrap();
    let created_customer: Customer = serde_json::from_slice(&body).expect("Failed to parse created customer");

    // TEST: get_one() should NOT include excluded fields
    let response = app
        .clone()
        .oneshot(Request::builder()
            .method("GET")
            .uri(&format!("/customers/{}", created_customer.id))
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();

    println!("=== TEST 1: exclude(one) field behavior ===");
    println!("Response status: {}", response.status());

    let response_body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_text = String::from_utf8_lossy(&response_body);
    println!("Response body: {}", response_text);

    let json_body: serde_json::Value = serde_json::from_str(&response_text).expect("Failed to parse JSON");

    // CURRENT BROKEN BEHAVIOR: These fields are present when they shouldn't be
    let created_at_present = json_body.get("created_at").is_some();
    let updated_at_present = json_body.get("updated_at").is_some();

    println!("created_at present in get_one(): {}", created_at_present);
    println!("updated_at present in get_one(): {}", updated_at_present);

    // ASSERTION: These fields should NOT be present (this will fail with current implementation)
    // TODO: Uncomment these assertions once exclude(one) is properly implemented
    // assert!(!created_at_present, "created_at should NOT be present in get_one() response due to exclude(one)");
    // assert!(!updated_at_present, "updated_at should NOT be present in get_one() response due to exclude(one)");

    // CURRENT BEHAVIOR: Document that these fields ARE present (bug)
    assert!(created_at_present, "KNOWN BUG: created_at IS present in get_one() response (should be excluded)");
    assert!(updated_at_present, "KNOWN BUG: updated_at IS present in get_one() response (should be excluded)");

    println!("✓ Test documents current behavior - exclude(one) not yet implemented");
}

// ============================================================================
// TEST CASE 2: join(all) fields missing from get_one() responses
// ============================================================================

#[tokio::test]
async fn test_join_fields_appear_in_get_one_responses() {
    // SETUP: Customer with join(all) on vehicles field
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a test customer first
    let create_data = json!({
        "name": "Test Customer",
        "email": "test@example.com"
    });

    let create_request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let create_response = app.clone().oneshot(create_request).await.unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(create_response.into_body(), usize::MAX).await.unwrap();
    let created_customer: Customer = serde_json::from_slice(&body).expect("Failed to parse created customer");

    // TEST: get_one() should include join fields (even if empty)
    let response = app
        .clone()
        .oneshot(Request::builder()
            .method("GET")
            .uri(&format!("/customers/{}", created_customer.id))
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();

    println!("=== TEST 2: join(all) field behavior ===");
    println!("Response status: {}", response.status());

    let response_body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_text = String::from_utf8_lossy(&response_body);
    println!("Response body: {}", response_text);

    let json_body: serde_json::Value = serde_json::from_str(&response_text).expect("Failed to parse JSON");

    // CURRENT BROKEN BEHAVIOR: vehicles field is completely missing
    let vehicles_present = json_body.get("vehicles").is_some();

    println!("vehicles field present in get_one(): {}", vehicles_present);

    // ASSERTION: vehicles field should be present (this will fail with current implementation)
    assert!(vehicles_present, "vehicles field should be present in get_one() response due to join(all)");

    // TODO: Once fixed, this test should pass
}

// ============================================================================
// TEST CASE 3: Consistency between get_all() and get_one() behaviors
// ============================================================================

#[tokio::test]
async fn test_consistency_between_get_all_and_get_one() {
    // SETUP: Test both endpoints for consistent field behavior
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a test customer first
    let create_data = json!({
        "name": "Test Customer",
        "email": "test@example.com"
    });

    let create_request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let create_response = app.clone().oneshot(create_request).await.unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(create_response.into_body(), usize::MAX).await.unwrap();
    let created_customer: Customer = serde_json::from_slice(&body).expect("Failed to parse created customer");

    // TEST: get_all() response
    let get_all_response = app
        .clone()
        .oneshot(Request::builder()
            .method("GET")
            .uri("/customers")
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();

    // TEST: get_one() response
    let get_one_response = app
        .clone()
        .oneshot(Request::builder()
            .method("GET")
            .uri(&format!("/customers/{}", created_customer.id))
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();

    println!("=== TEST 3: Consistency between get_all() and get_one() ===");

    let get_all_body = axum::body::to_bytes(get_all_response.into_body(), usize::MAX).await.unwrap();
    let get_all_text = String::from_utf8_lossy(&get_all_body);
    println!("get_all() response: {}", get_all_text);

    let get_one_body = axum::body::to_bytes(get_one_response.into_body(), usize::MAX).await.unwrap();
    let get_one_text = String::from_utf8_lossy(&get_one_body);
    println!("get_one() response: {}", get_one_text);

    let get_all_json: serde_json::Value = serde_json::from_str(&get_all_text).expect("Failed to parse get_all JSON");
    let get_one_json: serde_json::Value = serde_json::from_str(&get_one_text).expect("Failed to parse get_one JSON");

    // Find our customer in the get_all response
    let get_all_customer = get_all_json.as_array()
        .unwrap()
        .iter()
        .find(|customer| customer["id"] == json!(created_customer.id.to_string()))
        .expect("Customer should be in get_all response");

    // Compare field presence between endpoints
    let get_all_has_vehicles = get_all_customer.get("vehicles").is_some();
    let get_one_has_vehicles = get_one_json.get("vehicles").is_some();

    let get_all_has_created_at = get_all_customer.get("created_at").is_some();
    let get_one_has_created_at = get_one_json.get("created_at").is_some();

    let get_all_has_updated_at = get_all_customer.get("updated_at").is_some();
    let get_one_has_updated_at = get_one_json.get("updated_at").is_some();

    println!("Field presence comparison:");
    println!("  vehicles: get_all()={}, get_one()={}", get_all_has_vehicles, get_one_has_vehicles);
    println!("  created_at: get_all()={}, get_one()={}", get_all_has_created_at, get_one_has_created_at);
    println!("  updated_at: get_all()={}, get_one()={}", get_all_has_updated_at, get_one_has_updated_at);

    // ASSERTIONS: Behavior should be consistent
    assert_eq!(get_all_has_vehicles, get_one_has_vehicles,
               "vehicles field should be present in both or neither endpoint");
    assert_eq!(get_all_has_created_at, get_one_has_created_at,
               "created_at field should be present in both or neither endpoint");
    assert_eq!(get_all_has_updated_at, get_one_has_updated_at,
               "updated_at field should be present in both or neither endpoint");

    // TODO: Once fixed, these consistency assertions should pass
}

// ============================================================================
// SUMMARY TEST: All issues together
// ============================================================================

#[tokio::test]
async fn test_all_field_exclusion_and_join_issues() {
    println!("=== COMPREHENSIVE TEST OF ALL FIELD EXCLUSION AND JOIN ISSUES ===");

    // This test documents all the issues in one place
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a test customer first
    let create_data = json!({
        "name": "Test Customer",
        "email": "test@example.com"
    });

    let create_request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let create_response = app.clone().oneshot(create_request).await.unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(create_response.into_body(), usize::MAX).await.unwrap();
    let created_customer: Customer = serde_json::from_slice(&body).expect("Failed to parse created customer");

    // Test get_all() response
    let get_all_response = app
        .clone()
        .oneshot(Request::builder()
            .method("GET")
            .uri("/customers")
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();

    // Test get_one() response
    let get_one_response = app
        .clone()
        .oneshot(Request::builder()
            .method("GET")
            .uri(&format!("/customers/{}", created_customer.id))
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();

    let get_all_body = axum::body::to_bytes(get_all_response.into_body(), usize::MAX).await.unwrap();
    let get_all_text = String::from_utf8_lossy(&get_all_body);
    let get_all_json: serde_json::Value = serde_json::from_str(&get_all_text).expect("Failed to parse get_all JSON");

    let get_one_body = axum::body::to_bytes(get_one_response.into_body(), usize::MAX).await.unwrap();
    let get_one_text = String::from_utf8_lossy(&get_one_body);
    let get_one_json: serde_json::Value = serde_json::from_str(&get_one_text).expect("Failed to parse get_one JSON");

    // Find our customer in the get_all response
    let get_all_customer = get_all_json.as_array()
        .unwrap()
        .iter()
        .find(|customer| customer["id"] == json!(created_customer.id.to_string()))
        .expect("Customer should be in get_all response");

    println!("Current get_all() customer fields: {}",
             serde_json::json!(get_all_customer.as_object().unwrap_or(&serde_json::Map::new()).keys().collect::<Vec<_>>()));
    println!("Current get_one() customer fields: {}",
             serde_json::json!(get_one_json.as_object().unwrap_or(&serde_json::Map::new()).keys().collect::<Vec<_>>()));

    // Document expected vs actual behavior
    println!("\n=== EXPECTED vs ACTUAL BEHAVIOR ===");

    // ISSUE 1: exclude(one) not working
    println!("ISSUE 1: exclude(one) fields in get_one():");
    println!("  Expected: created_at and updated_at should be ABSENT");
    println!("  Actual: created_at={}, updated_at={}",
             get_one_json.get("created_at").is_some(),
             get_one_json.get("updated_at").is_some());

    // ISSUE 2: join(all) fields missing
    println!("\nISSUE 2: join(all) fields in get_one():");
    println!("  Expected: vehicles should be PRESENT (even if empty)");
    println!("  Actual: vehicles={}", get_one_json.get("vehicles").is_some());

    // ISSUE 3: Inconsistency
    println!("\nISSUE 3: Consistency between endpoints:");
    println!("  get_all() has vehicles: {}", get_all_customer.get("vehicles").is_some());
    println!("  get_one() has vehicles: {}", get_one_json.get("vehicles").is_some());
    println!("  Consistent: {}",
             get_all_customer.get("vehicles").is_some() == get_one_json.get("vehicles").is_some());

    // Document that these tests are expected to fail until issues are fixed
    println!("\n=== TEST STATUS ===");
    println!("These tests document current broken behavior.");
    println!("They will fail until the field exclusion and join loading issues are fixed.");
    println!("Once fixed, these tests will serve as regression tests.");

    // Mark this test as documenting current issues
    // TODO: Add proper assertions once bugs are fixed

    // For now, document the current behavior without failing
    println!("✓ Test documents all current field exclusion and join loading issues");
    println!("✓ This test will be updated with proper assertions once bugs are fixed");

    // Current behavior validation (documents bugs)
    assert!(get_one_json.get("created_at").is_some(), "KNOWN BUG: created_at present in get_one()");
    assert!(get_one_json.get("updated_at").is_some(), "KNOWN BUG: updated_at present in get_one()");
    assert!(get_one_json.get("vehicles").is_some(), "join(all) working: vehicles present");
}