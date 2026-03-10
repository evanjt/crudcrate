//! Tests for the CrudCrate aggregation feature.
//!
//! These tests verify:
//! 1. The `aggregate(...)` macro attribute compiles correctly
//! 2. Runtime helpers (validate_interval, parse_datetime) work correctly
//! 3. The aggregate route is mounted and rejects invalid intervals
//!
//! Note: Actual time_bucket SQL execution requires TimescaleDB (PostgreSQL extension).
//! These tests run against SQLite and focus on the framework layer.

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use crudcrate::aggregation;
use tower::ServiceExt;

use common::sensor_reading;

// --- Compile-time test: the aggregate macro expands and SensorReading compiles ---

#[test]
fn test_aggregate_model_compiles() {
    // If this test compiles, the aggregate macro expansion is valid
    fn _assert_crud_resource<T: crudcrate::traits::CRUDResource>() {}
    _assert_crud_resource::<sensor_reading::SensorReading>();
}

// --- Runtime helper tests ---

#[test]
fn test_validate_interval_exact_match() {
    let allowed = &["1h", "1d", "1w"];
    assert!(aggregation::validate_interval("1h", allowed).is_ok());
    assert_eq!(
        aggregation::validate_interval("1h", allowed).unwrap(),
        "1h"
    );
}

#[test]
fn test_validate_interval_short_format_match() {
    let allowed = &["1h", "1d", "1w"];
    // "1h" should match "1h" by parsing both
    assert!(aggregation::validate_interval("1h", allowed).is_ok());
    assert_eq!(
        aggregation::validate_interval("1h", allowed).unwrap(),
        "1h"
    );
}

#[test]
fn test_validate_interval_rejects_unlisted() {
    let allowed = &["1h", "1d"];
    let result = aggregation::validate_interval("5m", allowed);
    assert!(result.is_err());
}

#[test]
fn test_validate_interval_rejects_garbage() {
    let allowed = &["1h", "1d"];
    let result = aggregation::validate_interval("foobar", allowed);
    assert!(result.is_err());
}

#[test]
fn test_parse_datetime_rfc3339() {
    let dt = aggregation::parse_datetime("2024-01-15T10:30:00Z").unwrap();
    assert_eq!(dt.to_rfc3339(), "2024-01-15T10:30:00+00:00");
}

#[test]
fn test_parse_datetime_with_offset() {
    let dt = aggregation::parse_datetime("2024-01-15T10:30:00+05:00").unwrap();
    // Should convert to UTC
    assert_eq!(dt.to_rfc3339(), "2024-01-15T05:30:00+00:00");
}

#[test]
fn test_parse_datetime_naive_format() {
    let dt = aggregation::parse_datetime("2024-01-15T10:30:00").unwrap();
    assert_eq!(dt.to_rfc3339(), "2024-01-15T10:30:00+00:00");
}

#[test]
fn test_parse_datetime_date_only() {
    let dt = aggregation::parse_datetime("2024-01-15").unwrap();
    assert_eq!(dt.to_rfc3339(), "2024-01-15T00:00:00+00:00");
}

#[test]
fn test_parse_datetime_invalid() {
    assert!(aggregation::parse_datetime("not-a-date").is_err());
    assert!(aggregation::parse_datetime("").is_err());
}

// --- Integration tests: aggregate route is mounted ---

fn setup_aggregate_test_app(db: &sea_orm::DatabaseConnection) -> Router {
    Router::new().nest(
        "/sensor_readings",
        sensor_reading::SensorReading::router(db).into(),
    )
}

#[tokio::test]
async fn test_aggregate_route_exists() {
    let db = common::setup_sensor_db().await.unwrap();
    let app = setup_aggregate_test_app(&db);

    // The aggregate route should respond (not 404)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/sensor_readings/aggregate?interval=1h")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // We expect either 200 or a DB error (since SQLite doesn't have time_bucket),
    // but NOT 404 (which would mean the route wasn't mounted)
    assert_ne!(
        response.status().as_u16(),
        404,
        "Aggregate route should be mounted"
    );
}

#[tokio::test]
async fn test_aggregate_rejects_invalid_interval() {
    let db = common::setup_sensor_db().await.unwrap();
    let app = setup_aggregate_test_app(&db);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/sensor_readings/aggregate?interval=5m")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // 5m is not in the allowed intervals list, should get 400
    assert_eq!(
        response.status().as_u16(),
        400,
        "Invalid interval should return 400"
    );
}

#[tokio::test]
async fn test_aggregate_missing_interval_param() {
    let db = common::setup_sensor_db().await.unwrap();
    let app = setup_aggregate_test_app(&db);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/sensor_readings/aggregate")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Missing required 'interval' param should fail
    assert_eq!(
        response.status().as_u16(),
        400,
        "Missing interval should return 400"
    );
}
