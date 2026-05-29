//! Issue 3: invalid JSON in `?filter=` was silently ignored, returning an
//! unfiltered result instead of a 400. The fix routes the behavior through
//! `SecurityProfile::strict_filter_parsing` — `legacy()` preserves the historical
//! lenient behavior (so existing consumers don't break on bump), `secure()`
//! returns `400 Bad Request` so probing for unfiltered responses isn't free.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use crudcrate::SecurityProfile;
use tower::ServiceExt;

mod common;
use common::{setup_test_app, setup_test_db};

async fn get_with_raw_filter(app: axum::Router, filter: &str) -> StatusCode {
    let uri = format!("/customers?filter={filter}");
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    response.status()
}

#[tokio::test]
async fn legacy_profile_silently_ignores_malformed_filter() {
    let db = setup_test_db().await.expect("setup");
    // Explicit legacy() — 0.9.0 flipped the default to secure().
    let app = setup_test_app(&db).layer(axum::Extension(SecurityProfile::legacy()));
    let status = get_with_raw_filter(app, "garbage").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn secure_profile_rejects_malformed_filter() {
    let db = setup_test_db().await.expect("setup");
    let app = setup_test_app(&db).layer(axum::Extension(SecurityProfile::secure()));
    let status = get_with_raw_filter(app, "garbage").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn secure_profile_accepts_valid_filter() {
    let db = setup_test_db().await.expect("setup");
    let app = setup_test_app(&db).layer(axum::Extension(SecurityProfile::secure()));
    // Valid JSON, even if the filter matches nothing, must still be 200.
    let status = get_with_raw_filter(app, "%7B%22name%22%3A%22nobody%22%7D").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn secure_profile_accepts_empty_filter_param() {
    let db = setup_test_db().await.expect("setup");
    let app = setup_test_app(&db).layer(axum::Extension(SecurityProfile::secure()));
    // No filter param at all — not malformed, must be 200.
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
}

#[tokio::test]
async fn react_admin_profile_silently_ignores_malformed_filter() {
    let db = setup_test_db().await.expect("setup");
    let app = setup_test_app(&db).layer(axum::Extension(SecurityProfile::react_admin()));
    // react-admin presets are deliberately lenient — its filter components emit
    // partial JSON during user input and we don't want to 400 on every keystroke.
    let status = get_with_raw_filter(app, "garbage").await;
    assert_eq!(status, StatusCode::OK);
}
