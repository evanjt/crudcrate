//! Issue 1: JSON body bomb DoS via batch endpoints.
//!
//! Without an explicit `DefaultBodyLimit` layer, the generated router would rely on
//! Axum's per-handler 2 MiB default, but a batch resource with no override could
//! accept arbitrary-size requests if anyone wired `.layer(DefaultBodyLimit::disable())`
//! up the tree. `generate_crud_router!` now applies a `DefaultBodyLimit::max(...)`
//! layer derived from `CRUDResource::security_profile().max_request_body_bytes` so the
//! ceiling is explicit, configurable per resource, and not silently inherited.

use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

mod common;
use common::{setup_test_app, setup_test_db};

/// Build a JSON array body of roughly `size` bytes: one customer with a long name.
fn oversized_customer_batch_body(size: usize) -> String {
    let padding = "x".repeat(size);
    format!(r#"[{{"name":"{padding}","email":"a@b.c","is_private":false}}]"#)
}

#[tokio::test]
async fn batch_create_rejects_payload_over_profile_limit() {
    let db = setup_test_db().await.expect("setup");
    // Disable Axum's built-in 2 MiB default at the outer layer so the test can only
    // pass if crudcrate's per-resource `DefaultBodyLimit::max(...)` layer is also
    // present (inner layer overrides outer). Without crudcrate's layer, the
    // outer `disable()` would let the 3 MiB body through and the test would fail.
    let app = setup_test_app(&db).layer(DefaultBodyLimit::disable());

    let body = oversized_customer_batch_body(3 * 1024 * 1024);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/customers/batch")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "3 MiB body must be rejected by crudcrate's 2 MiB SecurityProfile layer even when the outer app disables Axum's default"
    );
}

#[tokio::test]
async fn batch_create_accepts_payload_under_profile_limit() {
    let db = setup_test_db().await.expect("setup");
    let app = setup_test_app(&db);

    // 100 KiB body is well under the 2 MiB default, so it must not be rejected for size.
    let body = oversized_customer_batch_body(100 * 1024);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/customers/batch")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_ne!(
        response.status(),
        StatusCode::PAYLOAD_TOO_LARGE,
        "100 KiB body must not be rejected for size"
    );
}
