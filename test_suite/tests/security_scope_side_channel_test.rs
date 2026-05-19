//! Issue 2: joined-filter scope side-channel.
//!
//! When a scoped (e.g. public) user filters parents via a joined-child column
//! (`?filter={"vehicles.make":"BMW"}`), the child sub-query may run without
//! its own scope condition. Even if the parent scope clause filters the final
//! result, the cardinality/timing of the response reveals the existence of
//! private parents matching the predicate.
//!
//! The fix routes the check through `SecurityProfile::scope_propagation_strict`.
//! Under `secure()` (strict), the handler refuses joined filters when scope is
//! active and the join target isn't known-scoped. Under `legacy()` (default),
//! the historical lenient behavior is preserved.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use crudcrate::SecurityProfile;
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_scoped_app, setup_test_db};

fn encode_filter(filter: &serde_json::Value) -> String {
    percent_encoding::utf8_percent_encode(&filter.to_string(), percent_encoding::NON_ALPHANUMERIC)
        .to_string()
}

#[tokio::test]
async fn legacy_profile_allows_joined_filter_under_scope() {
    let db = setup_test_db().await.expect("setup");
    let app = setup_scoped_app(&db); // default legacy() profile

    let filter = json!({"vehicles.make": "BMW"});
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/customers?filter={}", encode_filter(&filter)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "legacy profile preserves the historical behavior"
    );
}

#[tokio::test]
async fn secure_profile_rejects_joined_filter_under_scope() {
    let db = setup_test_db().await.expect("setup");
    let app = setup_scoped_app(&db).layer(axum::Extension(SecurityProfile::secure()));

    let filter = json!({"vehicles.make": "BMW"});
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/customers?filter={}", encode_filter(&filter)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "secure profile blocks joined filter under scope to prevent side-channel"
    );
}

#[tokio::test]
async fn secure_profile_allows_non_joined_filter_under_scope() {
    let db = setup_test_db().await.expect("setup");
    let app = setup_scoped_app(&db).layer(axum::Extension(SecurityProfile::secure()));

    // Filtering on a parent column (not joined) must still work under strict scope.
    let filter = json!({"name": "Alice"});
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/customers?filter={}", encode_filter(&filter)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "non-joined filters must still succeed under strict scope"
    );
}

#[tokio::test]
async fn secure_profile_allows_joined_filter_without_scope() {
    let db = setup_test_db().await.expect("setup");
    // Use the regular app (no scope middleware) — strict check only fires when
    // scope is active.
    let app = common::setup_test_app(&db).layer(axum::Extension(SecurityProfile::secure()));

    let filter = json!({"vehicles.make": "BMW"});
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/customers?filter={}", encode_filter(&filter)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "strict scope only fires when scope extension is present"
    );
}
