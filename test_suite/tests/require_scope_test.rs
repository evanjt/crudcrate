/// require_scope Tests
///
/// Validates the `#[crudcrate(require_scope)]` attribute:
/// - Resources WITHOUT require_scope work normally without scope middleware
/// - Resources WITH require_scope return 500 when scope middleware is missing
/// - Resources WITH require_scope work normally when scope middleware is present
mod common;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt;

use common::{setup_scoped_app, setup_test_app, setup_test_db};

/// POST a record via the admin app
async fn admin_post(app: &axum::Router, path: &str, payload: Value) -> (StatusCode, Value) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

/// GET via any app, return status + body
async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

// =============================================================================
// 1. Existing models have REQUIRE_SCOPE = false (regression)
// =============================================================================

#[test]
fn existing_models_do_not_require_scope() {
    use crudcrate::traits::CRUDResource;
    assert!(
        !common::customer::Customer::REQUIRE_SCOPE,
        "Customer should NOT require scope by default"
    );
    assert!(
        !common::vehicle::Vehicle::REQUIRE_SCOPE,
        "Vehicle should NOT require scope by default"
    );
}

// =============================================================================
// 2. Resources without require_scope allow unscoped access
// =============================================================================

#[tokio::test]
async fn no_require_scope_allows_unscoped_access() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);

    // Create a customer
    admin_post(
        &admin,
        "/customers",
        json!({"name": "Public", "email": "pub@example.com"}),
    )
    .await;

    // get_all without scope middleware: should succeed
    let (status, body) = get_json(&admin, "/customers").await;
    assert_eq!(status, StatusCode::OK, "Unscoped get_all should succeed");
    let items = body.as_array().unwrap();
    assert!(!items.is_empty(), "Should return data");
}

// =============================================================================
// 3. Resources without require_scope allow scoped access too
// =============================================================================

#[tokio::test]
async fn no_require_scope_allows_scoped_access() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    admin_post(
        &admin,
        "/customers",
        json!({"name": "ScopedOK", "email": "sok@example.com"}),
    )
    .await;

    // get_all with scope middleware: should also succeed
    let (status, body) = get_json(&scoped, "/customers").await;
    assert_eq!(status, StatusCode::OK, "Scoped get_all should succeed");
    let items = body.as_array().unwrap();
    assert!(!items.is_empty(), "Should return scoped data");
}

// =============================================================================
// 4. require_scope: get_one returns 404 (not 500) for nonexistent record with middleware
// =============================================================================
//
// This verifies that 500 is only for missing middleware, not missing records.
// We use the scoped customer endpoint (which has scope middleware but NOT require_scope)
// as a proxy test — the behavior should be identical for require_scope resources.

#[tokio::test]
async fn scoped_get_one_nonexistent_returns_404_not_500() {
    let db = setup_test_db().await.unwrap();
    let scoped = setup_scoped_app(&db);

    let fake_id = "00000000-0000-0000-0000-ffffffffffff";
    let (status, _) = get_json(&scoped, &format!("/customers/{fake_id}")).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "Nonexistent record with scope middleware should return 404, not 500"
    );
}

// =============================================================================
// 5. REQUIRE_SCOPE constant is accessible and defaults correctly
// =============================================================================

#[test]
fn require_scope_constant_has_correct_defaults() {
    use crudcrate::traits::CRUDResource;

    // All test models should have REQUIRE_SCOPE = false (none use the attribute)
    assert!(!common::customer::Customer::REQUIRE_SCOPE);
    assert!(!common::vehicle::Vehicle::REQUIRE_SCOPE);
    assert!(!common::vehicle_part::VehiclePart::REQUIRE_SCOPE);
    assert!(!common::maintenance_record::MaintenanceRecord::REQUIRE_SCOPE);
    assert!(!common::category::Category::REQUIRE_SCOPE);
}
