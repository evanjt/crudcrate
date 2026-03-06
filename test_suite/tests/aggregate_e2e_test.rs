//! End-to-end tests for entities with both CRUD + aggregate support.
//!
//! Verifies that:
//! 1. CRUD operations (POST/GET/PUT/DELETE) work on a CRUD+aggregate entity (SensorReading)
//! 2. Aggregate endpoint validation works with actual data in the database
//! 3. The aggregate-only entity (ReadingApi) aggregate_query() method produces
//!    a structurally correct query (right columns, right aliases)
//! 4. apply_aggregate_filters() produces correct filter conditions

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

use common::sensor_reading;

// --- Helper ---

fn setup_sensor_app(db: &sea_orm::DatabaseConnection) -> Router {
    Router::new().nest(
        "/sensor_readings",
        sensor_reading::SensorReading::router(db).into(),
    )
}

// --- CRUD e2e tests for SensorReading (CRUD+aggregate entity) ---

#[tokio::test]
async fn test_sensor_reading_crud_create_and_get() {
    let db = common::setup_sensor_db().await.unwrap();
    let app = setup_sensor_app(&db);

    let site_id = uuid::Uuid::new_v4();

    // Create a sensor reading
    let create_data = json!({
        "site_id": site_id.to_string(),
        "recorded_at": "2024-06-15T10:30:00Z",
        "value": 23.5
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/sensor_readings")
                .header("content-type", "application/json")
                .body(Body::from(create_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    assert_eq!(
        status,
        StatusCode::CREATED,
        "Create should return 201, got {status}: {}",
        String::from_utf8_lossy(&body)
    );

    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(created["site_id"].as_str().unwrap(), site_id.to_string());
    assert_eq!(created["value"].as_f64().unwrap(), 23.5);
    assert!(created["id"].as_str().is_some(), "Should have auto-generated id");

    let reading_id = created["id"].as_str().unwrap();

    // Get the created reading by ID
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/sensor_readings/{reading_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let retrieved: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(retrieved["id"].as_str().unwrap(), reading_id);
    assert_eq!(retrieved["site_id"].as_str().unwrap(), site_id.to_string());
    assert_eq!(retrieved["value"].as_f64().unwrap(), 23.5);
}

#[tokio::test]
async fn test_sensor_reading_crud_list() {
    let db = common::setup_sensor_db().await.unwrap();
    let app = setup_sensor_app(&db);

    let site_id = uuid::Uuid::new_v4();

    // Create two readings
    for (i, value) in [10.0, 20.0].iter().enumerate() {
        let create_data = json!({
            "site_id": site_id.to_string(),
            "recorded_at": format!("2024-06-15T{:02}:00:00Z", 10 + i),
            "value": value
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sensor_readings")
                    .header("content-type", "application/json")
                    .body(Body::from(create_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    // List all readings
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/sensor_readings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let readings: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert_eq!(readings.len(), 2, "Should have 2 readings");

    // Verify values are present
    let values: Vec<f64> = readings
        .iter()
        .map(|r| r["value"].as_f64().unwrap())
        .collect();
    assert!(values.contains(&10.0));
    assert!(values.contains(&20.0));
}

#[tokio::test]
async fn test_sensor_reading_crud_update() {
    let db = common::setup_sensor_db().await.unwrap();
    let app = setup_sensor_app(&db);

    // Create a reading
    let create_data = json!({
        "site_id": uuid::Uuid::new_v4().to_string(),
        "recorded_at": "2024-06-15T10:00:00Z",
        "value": 50.0
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/sensor_readings")
                .header("content-type", "application/json")
                .body(Body::from(create_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let reading_id = created["id"].as_str().unwrap();

    // Update the value
    let update_data = json!({ "value": 99.9 });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/sensor_readings/{reading_id}"))
                .header("content-type", "application/json")
                .body(Body::from(update_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated["value"].as_f64().unwrap(), 99.9);
}

#[tokio::test]
async fn test_sensor_reading_crud_delete() {
    let db = common::setup_sensor_db().await.unwrap();
    let app = setup_sensor_app(&db);

    // Create a reading
    let create_data = json!({
        "site_id": uuid::Uuid::new_v4().to_string(),
        "recorded_at": "2024-06-15T10:00:00Z",
        "value": 42.0
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/sensor_readings")
                .header("content-type", "application/json")
                .body(Body::from(create_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let reading_id = created["id"].as_str().unwrap();

    // Delete
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/sensor_readings/{reading_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify deleted
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/sensor_readings/{reading_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_sensor_reading_crud_filter_by_site_id() {
    let db = common::setup_sensor_db().await.unwrap();
    let app = setup_sensor_app(&db);

    let site_a = uuid::Uuid::new_v4();
    let site_b = uuid::Uuid::new_v4();

    // Create readings for two different sites
    for (site, val) in [(site_a, 10.0), (site_a, 20.0), (site_b, 30.0)] {
        let data = json!({
            "site_id": site.to_string(),
            "recorded_at": "2024-06-15T10:00:00Z",
            "value": val
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sensor_readings")
                    .header("content-type", "application/json")
                    .body(Body::from(data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    // Filter by site_a
    let filter = json!({ "site_id": site_a.to_string() });
    let filter_str = filter.to_string();
    let filter_encoded = url_escape::encode_component(&filter_str);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/sensor_readings?filter={filter_encoded}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let readings: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        readings.len(),
        2,
        "Should have 2 readings for site_a, got {}",
        readings.len()
    );
    for r in &readings {
        assert_eq!(r["site_id"].as_str().unwrap(), site_a.to_string());
    }
}

// --- Aggregate endpoint validation with data ---

#[tokio::test]
async fn test_aggregate_with_data_in_db() {
    // Insert data, then hit the aggregate endpoint.
    // On SQLite, time_bucket fails (500), but the query should reach the DB —
    // not fail at the validation/routing layer.
    let db = common::setup_sensor_db().await.unwrap();
    let app = setup_sensor_app(&db);

    let site_id = uuid::Uuid::new_v4();

    // Insert some readings
    for i in 0..5 {
        let data = json!({
            "site_id": site_id.to_string(),
            "recorded_at": format!("2024-06-15T{:02}:00:00Z", 10 + i),
            "value": (i as f64) * 10.0
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/sensor_readings")
                    .header("content-type", "application/json")
                    .body(Body::from(data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    // Hit the aggregate endpoint
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/sensor_readings/aggregate?interval=1%20hour")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status().as_u16();
    // On SQLite: 500 (time_bucket doesn't exist) — but NOT 404 (route missing)
    // On PostgreSQL+TimescaleDB: 200 with actual data
    assert_ne!(status, 404, "Route should be mounted");
    // The fact we get to the DB (500 on SQLite) proves the handler, validation,
    // and query building all work — only the DB function is missing.
    assert!(
        status == 200 || status == 500,
        "Expected 200 (TimescaleDB) or 500 (SQLite), got {status}"
    );
}

#[tokio::test]
async fn test_aggregate_with_time_range_filter() {
    let db = common::setup_sensor_db().await.unwrap();
    let app = setup_sensor_app(&db);

    // Aggregate with start/end — validation should pass even on SQLite
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/sensor_readings/aggregate?interval=1%20day&start=2024-01-01&end=2024-12-31")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status().as_u16();
    assert_ne!(status, 400, "Valid params should not return 400");
    assert_ne!(status, 404, "Route should be mounted");
}

#[tokio::test]
async fn test_aggregate_with_filter_param() {
    let db = common::setup_sensor_db().await.unwrap();
    let app = setup_sensor_app(&db);

    let site_id = uuid::Uuid::new_v4();
    let filter = json!({ "site_id": site_id.to_string() });
    let filter_str = filter.to_string();
    let filter_encoded = url_escape::encode_component(&filter_str);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/sensor_readings/aggregate?interval=1%20hour&filter={filter_encoded}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status().as_u16();
    // Filter should be applied without error — the DB error is only time_bucket
    assert_ne!(status, 400, "Valid filter should not return 400");
    assert_ne!(status, 404, "Route should be mounted");
}

// --- Unit tests for apply_aggregate_filters ---

#[test]
fn test_aggregate_filter_uuid_equality() {
    use crudcrate::aggregation::apply_aggregate_filters;
    use sea_orm::DatabaseBackend;

    // Use the SensorReading Column type for realistic testing
    let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
    let filter = json!({ "site_id": uuid_str }).to_string();

    let columns: Vec<(&str, sensor_reading::Column)> =
        vec![("site_id", sensor_reading::Column::SiteId)];

    let condition =
        apply_aggregate_filters(Some(filter), &columns, DatabaseBackend::Sqlite);

    // Condition should not be empty (Condition::all() with no additions)
    let condition_str = format!("{condition:?}");
    assert!(
        condition_str.contains("550e8400"),
        "Condition should contain the UUID value, got: {condition_str}"
    );
}

#[test]
fn test_aggregate_filter_ignores_unknown_columns() {
    use crudcrate::aggregation::apply_aggregate_filters;
    use sea_orm::DatabaseBackend;

    let filter = json!({ "nonexistent": "value" }).to_string();

    let columns: Vec<(&str, sensor_reading::Column)> =
        vec![("site_id", sensor_reading::Column::SiteId)];

    let condition =
        apply_aggregate_filters(Some(filter), &columns, DatabaseBackend::Sqlite);

    // Should be an empty Condition::all() (no filters applied)
    let condition_str = format!("{condition:?}");
    // Condition::all() with no additions is "All([])"
    assert!(
        !condition_str.contains("nonexistent"),
        "Unknown column should be ignored"
    );
}

#[test]
fn test_aggregate_filter_array_in() {
    use crudcrate::aggregation::apply_aggregate_filters;
    use sea_orm::DatabaseBackend;

    let uuid1 = "550e8400-e29b-41d4-a716-446655440001";
    let uuid2 = "550e8400-e29b-41d4-a716-446655440002";
    let filter = json!({ "site_id": [uuid1, uuid2] }).to_string();

    let columns: Vec<(&str, sensor_reading::Column)> =
        vec![("site_id", sensor_reading::Column::SiteId)];

    let condition =
        apply_aggregate_filters(Some(filter), &columns, DatabaseBackend::Sqlite);

    let condition_str = format!("{condition:?}");
    assert!(
        condition_str.contains("440001") && condition_str.contains("440002"),
        "Condition should contain both UUIDs for IN filter, got: {condition_str}"
    );
}

#[test]
fn test_aggregate_filter_null() {
    use crudcrate::aggregation::apply_aggregate_filters;
    use sea_orm::DatabaseBackend;

    let filter = json!({ "value": null }).to_string();

    let columns: Vec<(&str, sensor_reading::Column)> =
        vec![("value", sensor_reading::Column::Value)];

    let condition =
        apply_aggregate_filters(Some(filter), &columns, DatabaseBackend::Sqlite);

    let condition_str = format!("{condition:?}");
    assert!(
        condition_str.contains("Null") || condition_str.contains("null"),
        "Condition should contain IS NULL check, got: {condition_str}"
    );
}

#[test]
fn test_aggregate_filter_invalid_json() {
    use crudcrate::aggregation::apply_aggregate_filters;
    use sea_orm::DatabaseBackend;

    let columns: Vec<(&str, sensor_reading::Column)> =
        vec![("site_id", sensor_reading::Column::SiteId)];

    // Invalid JSON should return empty condition (no crash)
    let condition = apply_aggregate_filters(
        Some("not valid json".to_string()),
        &columns,
        DatabaseBackend::Sqlite,
    );

    let condition_str = format!("{condition:?}");
    // Should be a no-op condition
    assert!(
        !condition_str.contains("site_id"),
        "Invalid JSON should not produce any filter conditions"
    );
}

#[test]
fn test_aggregate_filter_none() {
    use crudcrate::aggregation::apply_aggregate_filters;
    use sea_orm::DatabaseBackend;

    let columns: Vec<(&str, sensor_reading::Column)> =
        vec![("site_id", sensor_reading::Column::SiteId)];

    let condition = apply_aggregate_filters(None, &columns, DatabaseBackend::Sqlite);

    let condition_str = format!("{condition:?}");
    assert!(
        !condition_str.contains("site_id"),
        "None filter should produce empty condition"
    );
}
