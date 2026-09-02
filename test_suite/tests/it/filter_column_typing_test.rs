//! Filter values are bound with the column type (timestamps, integers) rather than compared as text.

// Regression coverage for typed filter comparisons.
//
// A comparison filter on a non-text column (a date/timestamp, integer, etc.) sent
// as a JSON string used to be wrapped in `UPPER(col)`. On Postgres that errors
// (`function upper(timestamp with time zone) does not exist`); on SQLite it happened
// to work by loose typing but compared lexically, which is wrong for numeric order.
// These tests drive a real date range and an integer-as-string filter end to end and
// assert typed, correct results on whichever backend DATABASE_URL points at.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::{Value, json};
use tower::ServiceExt;

use common::{create_test_customer, setup_test_app, setup_test_db};
use test_suite as common;
use test_suite::http;

async fn get_list(app: &axum::Router, base: &str, filter: &Value) -> (StatusCode, Vec<Value>) {
    let encoded = utf8_percent_encode(&filter.to_string(), NON_ALPHANUMERIC).to_string();
    let uri = format!("{base}?filter={encoded}");
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
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let rows = serde_json::from_slice::<Value>(&bytes)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    (status, rows)
}

async fn get_raw(app: &axum::Router, base: &str, filter: &Value) -> (StatusCode, String) {
    let encoded = utf8_percent_encode(&filter.to_string(), NON_ALPHANUMERIC).to_string();
    let uri = format!("{base}?filter={encoded}");
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
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn create_vehicle(app: &axum::Router, customer_id: &str, year: i32) -> String {
    let (status, v) = http::post(
        app,
        "/vehicles",
        &json!({
            "customer_id": customer_id,
            "make": "Toyota",
            "model": "Camry",
            "year": year,
            "vin": format!("VIN{year}"),
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create vehicle: {v}");
    v["id"].as_str().unwrap().to_string()
}

async fn create_maintenance(app: &axum::Router, vehicle_id: &str, service_date: &str) {
    let (status, v) = http::post(
        app,
        "/maintenance_records",
        &json!({
            "vehicle_id": vehicle_id,
            "service_type": "oil_change",
            "description": format!("service at {service_date}"),
            "service_date": service_date,
            "completed": false,
        }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "create maintenance_record: {v}"
    );
}

#[tokio::test]
async fn timestamptz_gte_filter_returns_records_on_or_after() {
    let db = setup_test_db().await.expect("db");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;
    let vehicle_id = create_vehicle(&app, &customer_id, 2020).await;

    for date in [
        "2020-01-15T00:00:00Z",
        "2021-06-01T00:00:00Z",
        "2022-03-10T00:00:00Z",
    ] {
        create_maintenance(&app, &vehicle_id, date).await;
    }

    let (status, rows) = get_list(
        &app,
        "/maintenance_records",
        &json!({ "service_date_gte": "2021-01-01T00:00:00Z" }),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "date range filter must not error on the backend"
    );
    assert_eq!(
        rows.len(),
        2,
        "expected the 2021 and 2022 records, got {rows:?}"
    );
}

#[tokio::test]
async fn timestamptz_range_filter_selects_single_year() {
    let db = setup_test_db().await.expect("db");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;
    let vehicle_id = create_vehicle(&app, &customer_id, 2020).await;

    for date in [
        "2020-01-15T00:00:00Z",
        "2021-06-01T00:00:00Z",
        "2022-03-10T00:00:00Z",
    ] {
        create_maintenance(&app, &vehicle_id, date).await;
    }

    let (status, rows) = get_list(
        &app,
        "/maintenance_records",
        &json!({
            "service_date_gte": "2021-01-01T00:00:00Z",
            "service_date_lte": "2021-12-31T23:59:59Z",
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        rows.len(),
        1,
        "only the 2021 record is in range, got {rows:?}"
    );
}

#[tokio::test]
async fn integer_filter_sent_as_string_compares_numerically() {
    let db = setup_test_db().await.expect("db");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;
    for year in [2018, 2019, 2020, 2021] {
        create_vehicle(&app, &customer_id, year).await;
    }

    // A stringified number must compare as a number, not lexically ('9' > '10').
    let (status, rows) = get_list(&app, "/vehicles", &json!({ "year_gte": "2020" })).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "integer filter as string must not error"
    );
    assert_eq!(rows.len(), 2, "expected years 2020 and 2021, got {rows:?}");
}

#[tokio::test]
async fn timestamptz_array_filter_selects_listed_instants() {
    let db = setup_test_db().await.expect("db");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;
    let vehicle_id = create_vehicle(&app, &customer_id, 2020).await;

    for date in [
        "2020-01-15T00:00:00Z",
        "2021-06-01T00:00:00Z",
        "2022-03-10T00:00:00Z",
    ] {
        create_maintenance(&app, &vehicle_id, date).await;
    }

    let (status, rows) = get_list(
        &app,
        "/maintenance_records",
        &json!({ "service_date": ["2020-01-15T00:00:00Z", "2022-03-10T00:00:00Z"] }),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "an IN list over a timestamptz column must bind typed values"
    );
    assert_eq!(
        rows.len(),
        2,
        "expected the 2020 and 2022 records, got {rows:?}"
    );
}

#[tokio::test]
async fn timestamptz_array_filter_with_unparseable_element_is_rejected() {
    let db = setup_test_db().await.expect("db");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;
    let vehicle_id = create_vehicle(&app, &customer_id, 2020).await;
    create_maintenance(&app, &vehicle_id, "2020-01-15T00:00:00Z").await;

    let (status, body) = get_raw(
        &app,
        "/maintenance_records",
        &json!({ "service_date": ["2020-01-15T00:00:00Z", "not-a-date"] }),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "dropping the clause would return every record: {body}"
    );
    assert!(
        !body.to_lowercase().contains("timestamp"),
        "the error must not disclose the column's SQL type: {body}"
    );
}

#[tokio::test]
async fn integer_array_filter_sent_as_strings_matches() {
    let db = setup_test_db().await.expect("db");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;
    for year in [2018, 2019, 2020, 2021] {
        create_vehicle(&app, &customer_id, year).await;
    }

    let (status, rows) = get_list(&app, "/vehicles", &json!({ "year": ["2018", "2021"] })).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "an IN list of stringified numbers must bind integers"
    );
    assert_eq!(rows.len(), 2, "expected 2018 and 2021, got {rows:?}");
}

#[tokio::test]
async fn uuid_array_filter_still_matches() {
    let db = setup_test_db().await.expect("db");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;
    let vehicle_id = create_vehicle(&app, &customer_id, 2020).await;
    create_maintenance(&app, &vehicle_id, "2020-01-15T00:00:00Z").await;

    let (status, rows) = get_list(
        &app,
        "/maintenance_records",
        &json!({ "vehicle_id": [vehicle_id] }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(rows.len(), 1, "expected the one record, got {rows:?}");
}

#[tokio::test]
async fn enum_array_filter_still_matches() {
    let db = setup_test_db().await.expect("db");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;
    for (year, fuel) in [(2018, "Gasoline"), (2019, "Diesel"), (2020, "Electric")] {
        let (status, v) = http::post(
            &app,
            "/vehicles",
            &json!({
                "customer_id": customer_id,
                "make": "Toyota",
                "model": "Camry",
                "year": year,
                "vin": format!("VIN{year}"),
                "fuel_type": fuel,
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "create vehicle: {v}");
    }

    let (status, rows) = get_list(
        &app,
        "/vehicles",
        &json!({ "fuel_type": ["Gasoline", "Electric"] }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        rows.len(),
        2,
        "enum IN lists keep the upper-cased text path, got {rows:?}"
    );
}

#[tokio::test]
async fn joined_integer_array_filter_selects_parents() {
    let db = setup_test_db().await.expect("db");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;
    for year in [2018, 2019, 2021] {
        create_vehicle(&app, &customer_id, year).await;
    }

    let (status, rows) = get_list(&app, "/customers", &json!({ "vehicles.year": ["2018"] })).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "joined filters take the same typed path"
    );
    assert_eq!(rows.len(), 1, "expected the owning customer, got {rows:?}");
}

/// The whitelist check runs before any expression is built, so an unknown joined
/// column is still skipped silently rather than confirming its absence with a 400.
#[tokio::test]
async fn unknown_joined_filter_column_still_skips_silently() {
    let db = setup_test_db().await.expect("db");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;
    create_vehicle(&app, &customer_id, 2018).await;

    let (status, body) = get_raw(
        &app,
        "/customers",
        &json!({ "vehicles.nonexistent_column": ["not-a-date"] }),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "an unknown joined column must not become an error oracle: {body}"
    );
}

/// Scenario: a client sends an empty IN list, `{"year": []}`.
/// Expected behaviour: an empty result set. Returning every vehicle would be the
/// opposite of the filter, and on a scoped endpoint it would leak rows.
#[tokio::test]
async fn empty_array_filter_matches_no_rows() {
    let db = setup_test_db().await.expect("db");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;
    for year in [2018, 2019, 2020] {
        create_vehicle(&app, &customer_id, year).await;
    }

    let (status, rows) = get_list(&app, "/vehicles", &json!({ "year": [] })).await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        rows.is_empty(),
        "an empty IN list must match nothing, got {rows:?}"
    );
}

/// A number sent against a timestamptz column is a comparison no backend accepts.
/// The clause is dropped, so the request succeeds rather than returning a 500.
#[tokio::test]
async fn number_against_timestamptz_column_does_not_error() {
    let db = setup_test_db().await.expect("db");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;
    let vehicle_id = create_vehicle(&app, &customer_id, 2020).await;
    create_maintenance(&app, &vehicle_id, "2020-01-15T00:00:00Z").await;

    let (status, body) = get_raw(
        &app,
        "/maintenance_records",
        &json!({ "service_date_gte": 5 }),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "a numeric bound on a timestamptz column must not reach the backend: {body}"
    );
}
