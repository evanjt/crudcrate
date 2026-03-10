//! Tests for aggregate feature extensions:
//! - Custom aggregates (first/last) via `aggregates(avg, min, max, first, last)`
//! - Default aggregates when `aggregates(...)` omitted
//! - Timezone query parameter

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use crudcrate::aggregation::AggregateParams;
use tower::ServiceExt;

use common::{sensor_reading, sensor_reading_extended};

// --- Compile-time tests ---

#[test]
fn test_extended_aggregate_model_compiles() {
    // If this compiles, the aggregates(avg, min, max, first, last) attribute is parsed correctly
    let _api = sensor_reading_extended::SensorReadingExt;
}

#[tokio::test]
async fn test_extended_aggregate_query_callable() {
    let db = common::setup_sensor_ext_db().await.unwrap();

    let params = AggregateParams {
        interval: "1 hour".to_string(),
        start: None,
        end: None,
        filter: None,
        timezone: None,
    };

    // On SQLite time_bucket will fail, but the method should exist
    let result =
        sensor_reading_extended::SensorReadingExt::aggregate_query(&db, &params).await;
    assert!(
        result.is_err(),
        "Expected error on SQLite (no time_bucket support)"
    );
}

// --- Aggregate route tests for entity with first/last ---

fn setup_ext_app(db: &sea_orm::DatabaseConnection) -> Router {
    Router::new().nest(
        "/readings",
        sensor_reading_extended::SensorReadingExt::aggregate_router(db).into(),
    )
}

#[tokio::test]
async fn test_extended_aggregate_route_exists() {
    let db = common::setup_sensor_ext_db().await.unwrap();
    let app = setup_ext_app(&db);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/readings/aggregate?interval=1%20hour")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Not 404 means the route was mounted
    assert_ne!(
        response.status().as_u16(),
        404,
        "Aggregate route should be mounted for extended model"
    );
}

#[tokio::test]
async fn test_extended_aggregate_rejects_invalid_interval() {
    let db = common::setup_sensor_ext_db().await.unwrap();
    let app = setup_ext_app(&db);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/readings/aggregate?interval=5%20minutes")
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

// --- Default aggregates (sensor_reading has no aggregates(...) attr) ---

#[tokio::test]
async fn test_default_aggregates_still_work() {
    // SensorReading uses aggregate() without aggregates(...) → defaults to avg, min, max
    let db = common::setup_sensor_db().await.unwrap();

    let params = AggregateParams {
        interval: "1 hour".to_string(),
        start: None,
        end: None,
        filter: None,
        timezone: None,
    };

    // Should compile and be callable — the default aggregates are avg/min/max
    let result = sensor_reading::SensorReading::aggregate_query(&db, &params).await;
    // SQLite will fail on time_bucket, but the method compiles and runs
    assert!(result.is_err(), "Expected error on SQLite");
}

// --- Timezone parameter tests ---

fn setup_sensor_app(db: &sea_orm::DatabaseConnection) -> Router {
    Router::new().nest(
        "/sensor_readings",
        sensor_reading::SensorReading::router(db).into(),
    )
}

#[tokio::test]
async fn test_timezone_param_accepted() {
    let db = common::setup_sensor_db().await.unwrap();
    let app = setup_sensor_app(&db);

    // The timezone parameter should be accepted without a 400 error
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/sensor_readings/aggregate?interval=1%20hour&timezone=UTC")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status().as_u16();
    // Should not be 400 (bad request) or 404 (not found)
    // On SQLite: 500 (time_bucket doesn't exist), on TimescaleDB: 200
    assert_ne!(status, 400, "timezone=UTC should be accepted");
    assert_ne!(status, 404, "Route should be mounted");
}

#[tokio::test]
async fn test_timezone_param_none_works() {
    let db = common::setup_sensor_db().await.unwrap();
    let app = setup_sensor_app(&db);

    // Without timezone param should still work (uses regular time_bucket)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/sensor_readings/aggregate?interval=1%20day")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status().as_u16();
    assert_ne!(status, 400, "No timezone should work fine");
    assert_ne!(status, 404, "Route should be mounted");
}

#[tokio::test]
async fn test_aggregate_query_with_timezone() {
    let db = common::setup_sensor_db().await.unwrap();

    let params = AggregateParams {
        interval: "1 day".to_string(),
        start: Some("2024-01-01".to_string()),
        end: Some("2024-12-31".to_string()),
        filter: None,
        timezone: Some("US/Eastern".to_string()),
    };

    // Should compile and attempt to execute (will fail on SQLite)
    let result = sensor_reading::SensorReading::aggregate_query(&db, &params).await;
    assert!(result.is_err(), "Expected error on SQLite");
}
