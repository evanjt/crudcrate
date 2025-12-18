//! Tests for partial success in batch operations
//!
//! Currently, batch operations in CRUDCrate are all-or-nothing:
//! - If any item fails, the entire batch fails
//! - No partial results are returned
//!
//! This test file documents the expected behavior for a future partial success feature:
//! - Batch operations can succeed partially
//! - Response includes both successful results and errors
//! - Successful items are committed even if some fail
//!
//! ## Proposed Response Format
//!
//! ```json
//! {
//!   "succeeded": [
//!     { "id": "uuid-1", "name": "Item 1", ... },
//!     { "id": "uuid-2", "name": "Item 2", ... }
//!   ],
//!   "failed": [
//!     { "index": 2, "error": "Validation failed: name cannot be empty" },
//!     { "index": 4, "error": "Foreign key constraint violation" }
//!   ]
//! }
//! ```
//!
//! ## Current Behavior Tests
//!
//! These tests document the CURRENT all-or-nothing behavior.
//! When partial success is implemented, these will be updated or replaced.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_app, setup_test_db};

use crate::common::customer::CustomerList;

// =============================================================================
// CURRENT BEHAVIOR TESTS - Document all-or-nothing semantics
// =============================================================================

/// Test that batch create succeeds when all items are valid
#[tokio::test]
async fn test_batch_create_all_valid_succeeds() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create multiple valid customers
    let customers = json!([
        {"name": "Valid Customer 1", "email": "valid1@test.com"},
        {"name": "Valid Customer 2", "email": "valid2@test.com"},
        {"name": "Valid Customer 3", "email": "valid3@test.com"}
    ]);

    let request = Request::builder()
        .method("POST")
        .uri("/customers/batch")
        .header("content-type", "application/json")
        .body(Body::from(customers.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "Batch create with all valid items should succeed"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert_eq!(created.len(), 3, "Should create all 3 customers");
}

/// Test that batch update succeeds when all items are valid
#[tokio::test]
async fn test_batch_update_all_valid_succeeds() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // First create some customers
    let mut customer_ids = Vec::new();
    for i in 0..3 {
        let customer_json = json!({
            "name": format!("Update Test {}", i),
            "email": format!("update{}@test.com", i)
        });

        let request = Request::builder()
            .method("POST")
            .uri("/customers")
            .header("content-type", "application/json")
            .body(Body::from(customer_json.to_string()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
        customer_ids.push(customer["id"].as_str().unwrap().to_string());
    }

    // Update all customers
    let updates: Vec<serde_json::Value> = customer_ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            json!({
                "id": id,
                "name": format!("Updated Name {}", i)
            })
        })
        .collect();

    let request = Request::builder()
        .method("PATCH")
        .uri("/customers/batch")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&updates).unwrap()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Batch update with all valid items should succeed"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert_eq!(updated.len(), 3, "Should update all 3 customers");
}

/// Test that batch delete succeeds when all items exist
#[tokio::test]
async fn test_batch_delete_all_valid_succeeds() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // First create some customers
    let mut customer_ids = Vec::new();
    for i in 0..3 {
        let customer_json = json!({
            "name": format!("Delete Test {}", i),
            "email": format!("delete{}@test.com", i)
        });

        let request = Request::builder()
            .method("POST")
            .uri("/customers")
            .header("content-type", "application/json")
            .body(Body::from(customer_json.to_string()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
        customer_ids.push(customer["id"].as_str().unwrap().to_string());
    }

    // Delete all customers
    let request = Request::builder()
        .method("DELETE")
        .uri("/customers/batch")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&customer_ids).unwrap()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Batch delete with all valid IDs should succeed"
    );

    // Verify customers are deleted
    for id in &customer_ids {
        let request = Request::builder()
            .method("GET")
            .uri(format!("/customers/{}", id))
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Deleted customer should not be found"
        );
    }
}

// =============================================================================
// CURRENT FAILURE BEHAVIOR TESTS
// These document the all-or-nothing failure behavior
// =============================================================================

/// Test current behavior: batch update fails entirely when one item doesn't exist
/// Future: This should return partial success
#[tokio::test]
async fn test_batch_update_nonexistent_fails_all_currently() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create one valid customer
    let customer_json = json!({
        "name": "Existing Customer",
        "email": "existing@test.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(customer_json.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let existing_id = customer["id"].as_str().unwrap().to_string();

    // Try to update both existing and non-existing customer
    let updates = json!([
        {"id": existing_id, "name": "Updated Existing"},
        {"id": "00000000-0000-0000-0000-000000000000", "name": "Non-existent"}
    ]);

    let request = Request::builder()
        .method("PATCH")
        .uri("/customers/batch")
        .header("content-type", "application/json")
        .body(Body::from(updates.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Current behavior: entire batch fails
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Current behavior: batch update fails when any item not found"
    );

    // Verify existing customer was NOT updated (all-or-nothing)
    let request = Request::builder()
        .method("GET")
        .uri(format!("/customers/{}", existing_id))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        customer["name"].as_str().unwrap(),
        "Existing Customer",
        "Existing customer should NOT be updated in all-or-nothing mode"
    );
}

// =============================================================================
// FUTURE PARTIAL SUCCESS TESTS
// These tests will pass once partial success is implemented
// Currently marked as ignored
// =============================================================================

/// Future behavior: batch update returns partial success
/// Some items succeed, some fail, with detailed error info
#[tokio::test]
#[ignore = "Partial success not yet implemented"]
async fn test_batch_update_partial_success_future() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create two valid customers
    let mut customer_ids = Vec::new();
    for i in 0..2 {
        let customer_json = json!({
            "name": format!("Partial Test {}", i),
            "email": format!("partial{}@test.com", i)
        });

        let request = Request::builder()
            .method("POST")
            .uri("/customers")
            .header("content-type", "application/json")
            .body(Body::from(customer_json.to_string()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
        customer_ids.push(customer["id"].as_str().unwrap().to_string());
    }

    // Try to update: 2 existing + 1 non-existing
    let updates = json!([
        {"id": customer_ids[0], "name": "Updated 0"},
        {"id": "00000000-0000-0000-0000-000000000000", "name": "Non-existent"},
        {"id": customer_ids[1], "name": "Updated 1"}
    ]);

    let request = Request::builder()
        .method("PATCH")
        .uri("/customers/batch?partial=true")  // Enable partial success mode
        .header("content-type", "application/json")
        .body(Body::from(updates.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Should return 207 Multi-Status for partial success
    assert_eq!(
        response.status(),
        StatusCode::MULTI_STATUS,
        "Partial success should return 207"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Check succeeded items
    let succeeded = result["succeeded"].as_array().expect("Should have succeeded array");
    assert_eq!(succeeded.len(), 2, "Should have 2 successful updates");

    // Check failed items
    let failed = result["failed"].as_array().expect("Should have failed array");
    assert_eq!(failed.len(), 1, "Should have 1 failed update");
    assert_eq!(failed[0]["index"].as_u64().unwrap(), 1, "Failed item should be at index 1");

    // Verify successful updates were applied
    let request = Request::builder()
        .method("GET")
        .uri(format!("/customers/{}", customer_ids[0]))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        customer["name"].as_str().unwrap(),
        "Updated 0",
        "First customer should be updated"
    );
}

/// Future behavior: batch create with validation errors returns partial success
#[tokio::test]
#[ignore = "Partial success not yet implemented"]
async fn test_batch_create_validation_partial_success_future() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create batch with some invalid items (assuming name validation exists)
    let customers = json!([
        {"name": "Valid Customer 1", "email": "valid1@test.com"},
        {"name": "", "email": "invalid@test.com"},  // Invalid: empty name
        {"name": "Valid Customer 2", "email": "valid2@test.com"},
        {"name": "   ", "email": "whitespace@test.com"}  // Invalid: whitespace only
    ]);

    let request = Request::builder()
        .method("POST")
        .uri("/customers/batch?partial=true")
        .header("content-type", "application/json")
        .body(Body::from(customers.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Should return 207 Multi-Status for partial success
    assert_eq!(
        response.status(),
        StatusCode::MULTI_STATUS,
        "Partial success should return 207"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Check succeeded items
    let succeeded = result["succeeded"].as_array().expect("Should have succeeded array");
    assert_eq!(succeeded.len(), 2, "Should have 2 successful creates");

    // Check failed items
    let failed = result["failed"].as_array().expect("Should have failed array");
    assert_eq!(failed.len(), 2, "Should have 2 failed creates");

    // Verify error messages
    for failure in failed {
        assert!(
            failure["error"].as_str().unwrap().contains("validation") ||
            failure["error"].as_str().unwrap().contains("name"),
            "Error should mention validation issue"
        );
    }
}

/// Future behavior: batch delete with mixed existing/nonexisting IDs
#[tokio::test]
#[ignore = "Partial success not yet implemented"]
async fn test_batch_delete_partial_success_future() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create two customers
    let mut customer_ids = Vec::new();
    for i in 0..2 {
        let customer_json = json!({
            "name": format!("Delete Partial {}", i),
            "email": format!("deletepartial{}@test.com", i)
        });

        let request = Request::builder()
            .method("POST")
            .uri("/customers")
            .header("content-type", "application/json")
            .body(Body::from(customer_json.to_string()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
        customer_ids.push(customer["id"].as_str().unwrap().to_string());
    }

    // Try to delete: 2 existing + 1 non-existing
    let delete_ids = json!([
        customer_ids[0],
        "00000000-0000-0000-0000-000000000000",  // Non-existent
        customer_ids[1]
    ]);

    let request = Request::builder()
        .method("DELETE")
        .uri("/customers/batch?partial=true")
        .header("content-type", "application/json")
        .body(Body::from(delete_ids.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::MULTI_STATUS,
        "Partial success should return 207"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // 2 should succeed, 1 should fail
    let succeeded = result["succeeded"].as_array().expect("Should have succeeded array");
    assert_eq!(succeeded.len(), 2, "Should have 2 successful deletes");

    let failed = result["failed"].as_array().expect("Should have failed array");
    assert_eq!(failed.len(), 1, "Should have 1 failed delete");

    // Verify deleted customers are gone
    for id in &customer_ids {
        let request = Request::builder()
            .method("GET")
            .uri(format!("/customers/{}", id))
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Deleted customer should not be found"
        );
    }
}

// =============================================================================
// EDGE CASE TESTS
// =============================================================================

/// Test that empty batch operations work correctly
#[tokio::test]
async fn test_batch_operations_empty_input() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Empty batch create
    let request = Request::builder()
        .method("POST")
        .uri("/customers/batch")
        .header("content-type", "application/json")
        .body(Body::from("[]"))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "Empty batch create should succeed"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(created.is_empty(), "Empty batch should return empty array");
}

/// Test batch size limit enforcement
#[tokio::test]
async fn test_batch_size_limit_enforced() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Try to create more than the default batch limit (100)
    let customers: Vec<serde_json::Value> = (0..101)
        .map(|i| {
            json!({
                "name": format!("Batch Limit Test {}", i),
                "email": format!("batchlimit{}@test.com", i)
            })
        })
        .collect();

    let request = Request::builder()
        .method("POST")
        .uri("/customers/batch")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&customers).unwrap()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "Batch exceeding limit should be rejected"
    );
}

/// Test that batch operations maintain data consistency
#[tokio::test]
async fn test_batch_create_consistency() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a batch of customers
    let customers = json!([
        {"name": "Consistency Test 1", "email": "consistency1@test.com"},
        {"name": "Consistency Test 2", "email": "consistency2@test.com"},
        {"name": "Consistency Test 3", "email": "consistency3@test.com"}
    ]);

    let request = Request::builder()
        .method("POST")
        .uri("/customers/batch")
        .header("content-type", "application/json")
        .body(Body::from(customers.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    // Verify all customers can be retrieved
    for customer in &created {
        let id = customer["id"].as_str().unwrap();
        let request = Request::builder()
            .method("GET")
            .uri(format!("/customers/{}", id))
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Created customer should be retrievable"
        );
    }

    // Verify they appear in the list
    let request = Request::builder()
        .method("GET")
        .uri("/customers?range=[0,99]")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let all_customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    // All 3 created customers should be in the list
    let created_ids: Vec<String> = created
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();

    for id in &created_ids {
        assert!(
            all_customers.iter().any(|c| c.id.to_string() == *id),
            "Created customer {} should be in list",
            id
        );
    }
}
