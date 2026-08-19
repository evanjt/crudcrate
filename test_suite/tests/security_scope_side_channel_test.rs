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
async fn legacy_profile_allows_unscoped_joined_filter_under_scope() {
    let db = setup_test_db().await.expect("setup");
    // 0.9.0: default is secure(). Opt explicitly into legacy() to verify the
    // historical lenient behavior is still available.
    let app = setup_scoped_app(&db).layer(axum::Extension(SecurityProfile::legacy()));

    // Vehicle is scoped (has is_private) but VehiclePart is NOT scoped: filtering
    // vehicles by parts.name is the side-channel scenario. Legacy preserves the
    // historical lenient behavior.
    let filter = json!({"parts.name": "brake"});
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/vehicles?filter={}", encode_filter(&filter)))
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
async fn secure_profile_rejects_joined_filter_on_unscoped_child() {
    let db = setup_test_db().await.expect("setup");
    let app = setup_scoped_app(&db).layer(axum::Extension(SecurityProfile::secure()));

    // VehiclePart has no `exclude(scoped)` fields, so the child sub-query would
    // run unscoped, the existence side-channel the strict mode is designed to
    // block.
    let filter = json!({"parts.name": "brake"});
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/vehicles?filter={}", encode_filter(&filter)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "secure profile blocks joined filter on unscoped child"
    );
}

#[tokio::test]
async fn secure_profile_allows_joined_filter_on_scoped_child() {
    let db = setup_test_db().await.expect("setup");
    let app = setup_scoped_app(&db).layer(axum::Extension(SecurityProfile::secure()));

    // Customer→Vehicle: both have `exclude(scoped)`, so the child sub-query is
    // scope-restricted. Strict mode allows this: the derive macro reports
    // joined_field_has_scope("vehicles") = true.
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
        "secure profile allows joined filter when child entity is scoped"
    );
}

#[tokio::test]
async fn secure_profile_allows_non_joined_filter_under_scope() {
    let db = setup_test_db().await.expect("setup");
    let app = setup_scoped_app(&db).layer(axum::Extension(SecurityProfile::secure()));

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
    // No scope middleware: strict check only fires when scope is active.
    let app = common::setup_test_app(&db).layer(axum::Extension(SecurityProfile::secure()));

    let filter = json!({"parts.name": "brake"});
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/vehicles?filter={}", encode_filter(&filter)))
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
