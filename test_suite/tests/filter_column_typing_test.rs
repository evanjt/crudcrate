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

mod common;
use common::{create_test_customer, setup_test_app, setup_test_db};

async fn post(app: &axum::Router, uri: &str, payload: &Value) -> (StatusCode, Value) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

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

async fn create_vehicle(app: &axum::Router, customer_id: &str, year: i32) -> String {
    let (status, v) = post(
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
    let (status, v) = post(
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
