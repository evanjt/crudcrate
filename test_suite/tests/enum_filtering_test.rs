// Enum Field Auto-Detection Tests
// Tests that crudcrate automatically detects Sea-ORM enum fields (types implementing
// ActiveEnum) at compile time using the inherent impl trick — NO explicit
// `#[crudcrate(enum_field)]` annotation required.
//
// Two independent enums are tested (FuelType and Transmission) to prove the
// detection is generic, not hardcoded for any specific type. Non-enum types
// (String, i32, Uuid, DateTime) are verified to NOT be detected as enums.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use crate::common::vehicle::{FuelType, Transmission, VehicleList};
use common::{create_test_customer, setup_test_app, setup_test_db};

fn encode_filter(filter: &serde_json::Value) -> String {
    url_escape::encode_component(&filter.to_string()).to_string()
}

async fn create_vehicle(app: &axum::Router, customer_id: &str, vin: &str, fuel_type: &str) {
    let data = json!({
        "customer_id": customer_id,
        "make": "TestMake",
        "model": "TestModel",
        "year": 2024,
        "vin": vin,
        "fuel_type": fuel_type
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/vehicles")
                .header("content-type", "application/json")
                .body(Body::from(data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED, "Failed to create vehicle with fuel_type={fuel_type}");
}

async fn get_vehicles(app: &axum::Router, filter: &serde_json::Value) -> Vec<VehicleList> {
    let encoded = encode_filter(filter);
    let uri = format!("/vehicles?filter={encoded}");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

/// Test that creating a vehicle with an enum value works and the value round-trips correctly
#[tokio::test]
async fn test_enum_field_create_and_read() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;

    create_vehicle(&app, &customer_id, "ENUM-RT-1", "Gasoline").await;
    create_vehicle(&app, &customer_id, "ENUM-RT-2", "Diesel").await;
    create_vehicle(&app, &customer_id, "ENUM-RT-3", "Electric").await;

    let vehicles = get_vehicles(&app, &json!({})).await;
    let fuel_types: Vec<&Option<FuelType>> = vehicles.iter().map(|v| &v.fuel_type).collect();
    assert!(fuel_types.contains(&&Some(FuelType::Gasoline)));
    assert!(fuel_types.contains(&&Some(FuelType::Diesel)));
    assert!(fuel_types.contains(&&Some(FuelType::Electric)));
}

/// Test exact-match filtering on an enum field
#[tokio::test]
async fn test_enum_field_filter_exact_match() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;

    create_vehicle(&app, &customer_id, "ENUM-EX-1", "Gasoline").await;
    create_vehicle(&app, &customer_id, "ENUM-EX-2", "Diesel").await;
    create_vehicle(&app, &customer_id, "ENUM-EX-3", "Electric").await;

    let vehicles = get_vehicles(&app, &json!({"fuel_type": "Diesel"})).await;
    assert_eq!(vehicles.len(), 1, "Should find exactly one Diesel vehicle");
    assert_eq!(vehicles[0].fuel_type, Some(FuelType::Diesel));
}

/// Test case-insensitive filtering on an enum field
/// Filtering by "gasoline" (lowercase) should match "Gasoline" in the DB
#[tokio::test]
async fn test_enum_field_filter_case_insensitive() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;

    create_vehicle(&app, &customer_id, "ENUM-CI-1", "Gasoline").await;
    create_vehicle(&app, &customer_id, "ENUM-CI-2", "Diesel").await;

    let vehicles = get_vehicles(&app, &json!({"fuel_type": "gasoline"})).await;
    assert_eq!(vehicles.len(), 1, "Case-insensitive enum filter should match");
    assert_eq!(vehicles[0].fuel_type, Some(FuelType::Gasoline));
}

/// Test array/IN filtering on an enum field — this is the code path we fixed
/// in process_array_filter (added CAST(col AS TEXT) + UPPER for enum fields)
#[tokio::test]
async fn test_enum_field_filter_array_in() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;

    create_vehicle(&app, &customer_id, "ENUM-IN-1", "Gasoline").await;
    create_vehicle(&app, &customer_id, "ENUM-IN-2", "Diesel").await;
    create_vehicle(&app, &customer_id, "ENUM-IN-3", "Electric").await;

    let vehicles = get_vehicles(&app, &json!({"fuel_type": ["Gasoline", "Electric"]})).await;
    assert_eq!(vehicles.len(), 2, "Should find Gasoline and Electric vehicles");
    let fuel_types: Vec<&Option<FuelType>> = vehicles.iter().map(|v| &v.fuel_type).collect();
    assert!(fuel_types.contains(&&Some(FuelType::Gasoline)));
    assert!(fuel_types.contains(&&Some(FuelType::Electric)));
    assert!(!fuel_types.contains(&&Some(FuelType::Diesel)));
}

/// Test array/IN filtering with mixed case — should be case-insensitive
#[tokio::test]
async fn test_enum_field_filter_array_case_insensitive() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;

    create_vehicle(&app, &customer_id, "ENUM-ACI-1", "Gasoline").await;
    create_vehicle(&app, &customer_id, "ENUM-ACI-2", "Diesel").await;
    create_vehicle(&app, &customer_id, "ENUM-ACI-3", "Electric").await;

    let vehicles = get_vehicles(&app, &json!({"fuel_type": ["gasoline", "diesel"]})).await;
    assert_eq!(vehicles.len(), 2, "Case-insensitive array filter should match");
}

/// Test sorting by enum field
#[tokio::test]
async fn test_enum_field_sort() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;

    create_vehicle(&app, &customer_id, "ENUM-SORT-1", "Gasoline").await;
    create_vehicle(&app, &customer_id, "ENUM-SORT-2", "Diesel").await;
    create_vehicle(&app, &customer_id, "ENUM-SORT-3", "Electric").await;

    let sort = url_escape::encode_component(r#"["fuel_type","ASC"]"#);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/vehicles?sort={sort}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "Sorting by enum field should not error");
}

/// Test that the OpenAPI documentation endpoint serves correctly at runtime,
/// including the enum type schema. This hits the actual /api-docs endpoint
/// through the router — the same path a real client would use.
#[tokio::test]
async fn test_enum_field_openapi_docs_endpoint() {
    let db = setup_test_db().await.expect("Failed to setup test database");

    // Build the app WITH the OpenAPI docs endpoint (like a real server would)
    let (router, openapi) = utoipa_axum::router::OpenApiRouter::new()
        .nest("/vehicles", common::vehicle::Vehicle::router(&db))
        .split_for_parts();

    // Serve the OpenAPI JSON at /api-docs
    let app = router.route(
        "/api-docs",
        axum::routing::get(move || {
            let doc = openapi.clone();
            async move { axum::Json(doc) }
        }),
    );

    // Hit the docs endpoint via HTTP — this is what utoipa renders at runtime
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api-docs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK, "OpenAPI docs endpoint should return 200");

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&body)
        .expect("OpenAPI docs should be valid JSON");

    // Verify the FuelType enum schema is present with correct variants
    let schemas = &doc["components"]["schemas"];
    let fuel_type_schema = &schemas["FuelType"];
    assert!(
        fuel_type_schema.is_object(),
        "FuelType schema should exist in OpenAPI doc. Available schemas: {:?}",
        schemas.as_object().map(|m| m.keys().collect::<Vec<_>>())
    );

    let enum_values = fuel_type_schema["enum"]
        .as_array()
        .expect("FuelType should have enum values");
    let values: Vec<&str> = enum_values.iter().filter_map(|v| v.as_str()).collect();
    assert!(values.contains(&"Gasoline"), "Schema should contain Gasoline");
    assert!(values.contains(&"Diesel"), "Schema should contain Diesel");
    assert!(values.contains(&"Electric"), "Schema should contain Electric");

    // Verify VehicleCreate schema references FuelType
    let create_props = &schemas["VehicleCreate"]["properties"];
    assert!(
        create_props["fuel_type"].is_object(),
        "VehicleCreate should have fuel_type property"
    );

    // Verify VehicleList schema also references FuelType
    let list_props = &schemas["VehicleList"]["properties"];
    assert!(
        list_props["fuel_type"].is_object(),
        "VehicleList should have fuel_type property"
    );
}

/// Test _neq operator on enum field
#[tokio::test]
async fn test_enum_field_filter_neq() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;

    create_vehicle(&app, &customer_id, "ENUM-NEQ-1", "Gasoline").await;
    create_vehicle(&app, &customer_id, "ENUM-NEQ-2", "Diesel").await;
    create_vehicle(&app, &customer_id, "ENUM-NEQ-3", "Electric").await;

    let vehicles = get_vehicles(&app, &json!({"fuel_type_neq": "Gasoline"})).await;
    assert_eq!(vehicles.len(), 2, "Should find all non-Gasoline vehicles");
    assert!(vehicles.iter().all(|v| v.fuel_type != Some(FuelType::Gasoline)));
}

// =============================================================================
// AUTO-DETECTION ROBUSTNESS TESTS
// Proves the compile-time enum detection is generic, not specific to FuelType
// =============================================================================

/// Directly assert is_enum_field() returns correct values for EVERY field type
/// in the Vehicle model. This is the core proof that auto-detection is generic:
/// - DeriveActiveEnum types (FuelType, Transmission) → true
/// - String, i32, Uuid, DateTime, bool → false
#[test]
fn test_enum_auto_detection_all_field_types() {
    use crudcrate::traits::CRUDResource;
    use common::vehicle::Vehicle;

    // Enum types — should be auto-detected as true
    assert!(Vehicle::is_enum_field("fuel_type"), "FuelType (DeriveActiveEnum) should be detected");
    assert!(Vehicle::is_enum_field("transmission"), "Transmission (DeriveActiveEnum) should be detected");

    // Non-enum types — must be false
    assert!(!Vehicle::is_enum_field("id"), "Uuid should NOT be detected as enum");
    assert!(!Vehicle::is_enum_field("customer_id"), "Uuid should NOT be detected as enum");
    assert!(!Vehicle::is_enum_field("make"), "String should NOT be detected as enum");
    assert!(!Vehicle::is_enum_field("model"), "String should NOT be detected as enum");
    assert!(!Vehicle::is_enum_field("year"), "i32 should NOT be detected as enum");
    assert!(!Vehicle::is_enum_field("vin"), "String should NOT be detected as enum");
    assert!(!Vehicle::is_enum_field("created_at"), "DateTime<Utc> should NOT be detected as enum");
    assert!(!Vehicle::is_enum_field("updated_at"), "DateTime<Utc> should NOT be detected as enum");

    // Unknown fields — must be false
    assert!(!Vehicle::is_enum_field("nonexistent"), "Unknown fields should return false");
}

/// Integration test: the SECOND enum (Transmission) also works for filtering
/// without any enum_field annotation — proves detection is truly generic
#[tokio::test]
async fn test_second_enum_filter_works_without_annotation() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);
    let customer_id = create_test_customer(&app).await;

    // Create vehicles with different transmissions
    for (vin, transmission) in [("TRANS-1", "Manual"), ("TRANS-2", "Automatic"), ("TRANS-3", "Cvt")] {
        let data = json!({
            "customer_id": customer_id,
            "make": "TestMake",
            "model": "TestModel",
            "year": 2024,
            "vin": vin,
            "transmission": transmission,
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/vehicles")
                    .header("content-type", "application/json")
                    .body(Body::from(data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let response_status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            response_status, StatusCode::CREATED,
            "Failed to create vehicle with transmission={transmission}: {}",
            String::from_utf8_lossy(&body)
        );
    }

    // Single-value filter — case-insensitive: "manual" matches DB value "Manual"
    let vehicles = get_vehicles(&app, &json!({"transmission": "manual"})).await;
    assert_eq!(vehicles.len(), 1, "Case-insensitive single filter on Transmission should work");
    assert_eq!(vehicles[0].transmission, Some(Transmission::Manual));

    // Array/IN filter — case-insensitive: "cvt" matches DB value "CVT", "automatic" matches "Automatic"
    let vehicles = get_vehicles(&app, &json!({"transmission": ["cvt", "automatic"]})).await;
    assert_eq!(vehicles.len(), 2, "Case-insensitive array filter on Transmission should work");
    let types: Vec<_> = vehicles.iter().map(|v| v.transmission.clone()).collect();
    assert!(types.contains(&Some(Transmission::Cvt)));
    assert!(types.contains(&Some(Transmission::Automatic)));
    assert!(!types.contains(&Some(Transmission::Manual)));
}
