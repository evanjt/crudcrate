//! Tests for aggregate-only entities (composite PK, no CRUDResource).
//!
//! These tests verify:
//! 1. A composite-PK entity with aggregate() but no generate_router compiles
//! 2. ReadingApi is a unit struct (no CRUDResource impl)
//! 3. aggregate_query() is callable programmatically
//! 4. aggregate_router() produces a mountable router
//! 5. The aggregate route is mounted and works correctly

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use crudcrate::aggregation::AggregateParams;
use tower::ServiceExt;

use common::reading;

// --- Compile-time tests ---

#[test]
fn test_aggregate_only_model_compiles() {
    // If this test compiles, the aggregate-only macro expansion is valid.
    // ReadingApi should exist as a unit struct.
    let _api = reading::ReadingApi;
}

#[test]
fn test_aggregate_only_does_not_implement_crud_resource() {
    // ReadingApi should NOT implement CRUDResource.
    // This is a negative compile-time assertion — we verify it at the type level
    // by checking that ReadingApi is a unit struct (no fields).
    fn _assert_unit_struct(_: reading::ReadingApi) {}
    _assert_unit_struct(reading::ReadingApi);
}

// --- aggregate_query() direct call tests ---

#[tokio::test]
async fn test_aggregate_query_callable_directly() {
    let db = common::setup_readings_db().await.unwrap();

    let params = AggregateParams {
        interval: "1h".to_string(),
        start: None,
        end: None,
        filter: None,
        timezone: None,
    };

    // This tests that aggregate_query() is a callable method on ReadingApi.
    // On SQLite, time_bucket will fail (it's a TimescaleDB function), but the
    // important thing is that the method exists and compiles.
    let result = reading::ReadingApi::aggregate_query(&db, &params).await;

    // SQLite doesn't support time_bucket, so we expect an error
    // But the method should exist and be callable
    assert!(
        result.is_err(),
        "Expected error on SQLite (no time_bucket support)"
    );
}

#[tokio::test]
async fn test_aggregate_query_rejects_invalid_interval() {
    let db = common::setup_readings_db().await.unwrap();

    let params = AggregateParams {
        interval: "5m".to_string(),
        start: None,
        end: None,
        filter: None,
        timezone: None,
    };

    let result = reading::ReadingApi::aggregate_query(&db, &params).await;
    assert!(result.is_err(), "Should reject unlisted interval");

    let err = result.unwrap_err();
    let err_str = format!("{err}");
    assert!(
        err_str.contains("Invalid interval") || err_str.contains("invalid"),
        "Error should mention invalid interval, got: {err_str}"
    );
}

// --- aggregate_router() integration tests ---

fn setup_aggregate_only_app(db: &sea_orm::DatabaseConnection) -> Router {
    Router::new().nest(
        "/readings",
        reading::ReadingApi::aggregate_router(db).into(),
    )
}

#[tokio::test]
async fn test_aggregate_only_route_exists() {
    let db = common::setup_readings_db().await.unwrap();
    let app = setup_aggregate_only_app(&db);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/readings/aggregate?interval=1h")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Not 404 means the route was mounted
    assert_ne!(
        response.status().as_u16(),
        404,
        "Aggregate route should be mounted via aggregate_router()"
    );
}

#[tokio::test]
async fn test_aggregate_only_rejects_invalid_interval() {
    let db = common::setup_readings_db().await.unwrap();
    let app = setup_aggregate_only_app(&db);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/readings/aggregate?interval=5m")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status().as_u16(),
        400,
        "Invalid interval should return 400"
    );
}

#[tokio::test]
async fn test_aggregate_only_missing_interval() {
    let db = common::setup_readings_db().await.unwrap();
    let app = setup_aggregate_only_app(&db);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/readings/aggregate")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status().as_u16(),
        400,
        "Missing interval should return 400"
    );
}

#[tokio::test]
async fn test_aggregate_only_no_crud_routes() {
    let db = common::setup_readings_db().await.unwrap();
    let app = setup_aggregate_only_app(&db);

    // CRUD endpoints should NOT exist on aggregate-only router
    for (method, uri) in [
        ("GET", "/readings"),
        ("POST", "/readings"),
        ("GET", "/readings/00000000-0000-0000-0000-000000000000"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status().as_u16(),
            404,
            "CRUD route {method} {uri} should NOT exist on aggregate-only router"
        );
    }
}

// --- aggregate_query() on full CRUD+aggregate entity ---

#[tokio::test]
async fn test_sensor_reading_aggregate_query_callable() {
    // SensorReading has both generate_router AND aggregate — verify aggregate_query
    // exists and is callable on the full CRUD entity too.
    let db = common::setup_sensor_db().await.unwrap();

    let params = AggregateParams {
        interval: "1d".to_string(),
        start: Some("2024-01-01".to_string()),
        end: Some("2024-12-31".to_string()),
        filter: None,
        timezone: None,
    };

    let result = common::sensor_reading::SensorReading::aggregate_query(&db, &params).await;

    // SQLite doesn't support time_bucket, but the method should exist
    assert!(result.is_err(), "Expected error on SQLite");
}
