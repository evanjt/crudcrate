//! A single array-valued filter must not fan out into an unbounded `IN (...)` list.
//! `MAX_FILTER_CLAUSES` caps the number of filter keys, not the length of any one
//! array, and this path is reachable over GET, whose query string is not covered by
//! `DefaultBodyLimit`. An over-cap array returns `400 Bad Request` (reject, don't
//! silently drop), not a 500 from a bind-parameter overflow.

use test_suite as common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

use common::{setup_test_app, setup_test_db};

fn encode_filter(filter: &serde_json::Value) -> String {
    percent_encoding::utf8_percent_encode(&filter.to_string(), percent_encoding::NON_ALPHANUMERIC)
        .to_string()
}

async fn get_status(app: &axum::Router, uri: &str) -> StatusCode {
    app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

/// An array of 1000 elements (the cap) is accepted and processed normally.
#[tokio::test]
async fn array_filter_at_cap_is_accepted() {
    let db = setup_test_db().await.expect("setup");
    let app = setup_test_app(&db);

    let makes: Vec<String> = (0..1000).map(|i| format!("make-{i}")).collect();
    let filter = json!({ "make": makes });
    let status = get_status(
        &app,
        &format!("/vehicles?filter={}", encode_filter(&filter)),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "an array at the element cap must be accepted"
    );
}

/// One element over the cap (1000) is rejected up front with 400 rather than fanning
/// out into an oversized `IN (...)`. Unbounded, a large array would overflow every
/// backend's bind-parameter ceiling and 500. (Stays just over the cap so the URI
/// itself remains within the `http` crate's own length limit, which is a separate,
/// coarser protection.)
#[tokio::test]
async fn oversized_array_filter_is_rejected_not_500() {
    let db = setup_test_db().await.expect("setup");
    let app = setup_test_app(&db);

    let makes: Vec<String> = (0..1001).map(|i| format!("make-{i}")).collect();
    let filter = json!({ "make": makes });
    let status = get_status(
        &app,
        &format!("/vehicles?filter={}", encode_filter(&filter)),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "an array over the cap must be rejected with 400, not 500"
    );
}
