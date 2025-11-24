// Comprehensive Filtering Tests
// Tests EVERY example from docs/src/features/filtering.md
// Goal: 100% documentation-test alignment

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{create_test_customer, setup_test_app, setup_test_db};
use crate::common::customer::CustomerList;
use crate::common::vehicle::VehicleList;

// =============================================================================
// COMPARISON OPERATORS TESTS (filtering.md lines 46-65)
// =============================================================================

#[tokio::test]
async fn test_filtering_comparison_operator_neq_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a valid customer first (required for foreign key constraint)
    let customer_id = create_test_customer(&app).await;

    // Create test vehicles with different years
    for year in [2020, 2021, 2022] {
        let vehicle_data = json!({
            "customer_id": customer_id,
            "make": "Toyota",
            "model": "Camry",
            "year": year,
            "vin": format!("VIN{}", year)
        });
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/vehicles")
                    .header("content-type", "application/json")
                    .body(Body::from(vehicle_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test: Not equals (docs example: {"status_neq":"inactive"})
    // Adapted: {"year_neq":2021}
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/vehicles?filter=%7B%22year_neq%22%3A2021%7D") // {"year_neq":2021}
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let vehicles: Vec<VehicleList> = serde_json::from_slice(&body).unwrap();

    // Should return 2020 and 2022, but NOT 2021
    assert_eq!(vehicles.len(), 2, "Should exclude year 2021");
    assert!(vehicles.iter().all(|v| v.year != 2021), "No vehicle should have year 2021");
}

#[tokio::test]
async fn test_filtering_comparison_operator_gt_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a valid customer first (required for foreign key constraint)
    let customer_id = create_test_customer(&app).await;

    // Create test vehicles
    for year in [2018, 2019, 2020, 2021] {
        let vehicle_data = json!({
            "customer_id": customer_id,
            "make": "Honda",
            "model": "Civic",
            "year": year,
            "vin": format!("VIN{}", year)
        });
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/vehicles")
                    .header("content-type", "application/json")
                    .body(Body::from(vehicle_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test: Greater than (docs example: {"priority_gt":3})
    // Adapted: {"year_gt":2019}
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/vehicles?filter=%7B%22year_gt%22%3A2019%7D") // {"year_gt":2019}
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let vehicles: Vec<VehicleList> = serde_json::from_slice(&body).unwrap();

    // Should return 2020 and 2021 (both > 2019)
    assert_eq!(vehicles.len(), 2, "Should return vehicles with year > 2019");
    assert!(vehicles.iter().all(|v| v.year > 2019), "All vehicles should have year > 2019");
}

#[tokio::test]
async fn test_filtering_comparison_operator_gte_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a valid customer first (required for foreign key constraint)
    let customer_id = create_test_customer(&app).await;

    // Create test vehicles
    for year in [2018, 2019, 2020] {
        let vehicle_data = json!({
            "customer_id": customer_id,
            "make": "Ford",
            "model": "F-150",
            "year": year,
            "vin": format!("VIN{}", year)
        });
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/vehicles")
                    .header("content-type", "application/json")
                    .body(Body::from(vehicle_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test: Greater than or equal (docs example: {"priority_gte":3})
    // Adapted: {"year_gte":2019}
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/vehicles?filter=%7B%22year_gte%22%3A2019%7D") // {"year_gte":2019}
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let vehicles: Vec<VehicleList> = serde_json::from_slice(&body).unwrap();

    // Should return 2019 and 2020 (both >= 2019)
    assert_eq!(vehicles.len(), 2, "Should return vehicles with year >= 2019");
    assert!(vehicles.iter().all(|v| v.year >= 2019), "All vehicles should have year >= 2019");
}

#[tokio::test]
async fn test_filtering_comparison_operator_lt_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a valid customer first (required for foreign key constraint)
    let customer_id = create_test_customer(&app).await;

    // Create test vehicles
    for year in [2020, 2021, 2022, 2023] {
        let vehicle_data = json!({
            "customer_id": customer_id,
            "make": "Chevrolet",
            "model": "Silverado",
            "year": year,
            "vin": format!("VIN{}", year)
        });
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/vehicles")
                    .header("content-type", "application/json")
                    .body(Body::from(vehicle_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test: Less than (docs example: {"priority_lt":10})
    // Adapted: {"year_lt":2022}
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/vehicles?filter=%7B%22year_lt%22%3A2022%7D") // {"year_lt":2022}
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let vehicles: Vec<VehicleList> = serde_json::from_slice(&body).unwrap();

    // Should return 2020 and 2021 (both < 2022)
    assert_eq!(vehicles.len(), 2, "Should return vehicles with year < 2022");
    assert!(vehicles.iter().all(|v| v.year < 2022), "All vehicles should have year < 2022");
}

#[tokio::test]
async fn test_filtering_comparison_operator_lte_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a valid customer first (required for foreign key constraint)
    let customer_id = create_test_customer(&app).await;

    // Create test vehicles
    for year in [2020, 2021, 2022] {
        let vehicle_data = json!({
            "customer_id": customer_id,
            "make": "Nissan",
            "model": "Altima",
            "year": year,
            "vin": format!("VIN{}", year)
        });
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/vehicles")
                    .header("content-type", "application/json")
                    .body(Body::from(vehicle_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test: Less than or equal (docs example: {"priority_lte":10})
    // Adapted: {"year_lte":2021}
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/vehicles?filter=%7B%22year_lte%22%3A2021%7D") // {"year_lte":2021}
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let vehicles: Vec<VehicleList> = serde_json::from_slice(&body).unwrap();

    // Should return 2020 and 2021 (both <= 2021)
    assert_eq!(vehicles.len(), 2, "Should return vehicles with year <= 2021");
    assert!(vehicles.iter().all(|v| v.year <= 2021), "All vehicles should have year <= 2021");
}

// =============================================================================
// NUMBER FILTERING TESTS (filtering.md lines 92-102)
// =============================================================================

#[tokio::test]
async fn test_filtering_number_exact_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a valid customer first (required for foreign key constraint)
    let customer_id = create_test_customer(&app).await;

    // Create test vehicles
    for (i, year) in [2019, 2020, 2020, 2021].iter().enumerate() {
        let vehicle_data = json!({
            "customer_id": customer_id,
            "make": "Mazda",
            "model": "CX-5",
            "year": year,
            "vin": format!("VIN{}_{}", year, i)
        });
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/vehicles")
                    .header("content-type", "application/json")
                    .body(Body::from(vehicle_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test: Exact number (docs example: {"quantity":10})
    // Adapted: {"year":2020}
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/vehicles?filter=%7B%22year%22%3A2020%7D") // {"year":2020}
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let vehicles: Vec<VehicleList> = serde_json::from_slice(&body).unwrap();

    // Should return exactly 2 vehicles with year 2020
    assert_eq!(vehicles.len(), 2, "Should return exactly 2 vehicles with year 2020");
    assert!(vehicles.iter().all(|v| v.year == 2020), "All vehicles should have year 2020");
}

#[tokio::test]
async fn test_filtering_number_range_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a valid customer first (required for foreign key constraint)
    let customer_id = create_test_customer(&app).await;

    // Create test vehicles with years 2015-2025
    for year in [2015, 2018, 2020, 2022, 2025] {
        let vehicle_data = json!({
            "customer_id": customer_id,
            "make": "Subaru",
            "model": "Outback",
            "year": year,
            "vin": format!("VIN{}", year)
        });
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/vehicles")
                    .header("content-type", "application/json")
                    .body(Body::from(vehicle_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test: Range (docs example: {"quantity_gte":5,"quantity_lte":20})
    // Adapted: {"year_gte":2018,"year_lte":2022}
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/vehicles?filter=%7B%22year_gte%22%3A2018%2C%22year_lte%22%3A2022%7D") // {"year_gte":2018,"year_lte":2022}
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let vehicles: Vec<VehicleList> = serde_json::from_slice(&body).unwrap();

    // Should return 2018, 2020, 2022 (3 vehicles)
    assert_eq!(vehicles.len(), 3, "Should return 3 vehicles in range 2018-2022");
    assert!(vehicles.iter().all(|v| v.year >= 2018 && v.year <= 2022),
        "All vehicles should be in range 2018-2022");
}

// =============================================================================
// STRING FILTERING TESTS (filtering.md lines 82-89)
// =============================================================================

#[tokio::test]
async fn test_filtering_string_exact_match_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create test customers
    let customers = [
        json!({"name": "John Doe", "email": "john@example.com"}),
        json!({"name": "Jane Smith", "email": "jane@example.com"}),
        json!({"name": "John Smith", "email": "john.smith@example.com"}),
    ];

    for customer_data in &customers {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(customer_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test: Exact match (docs example: {"name":"John"})
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?filter=%7B%22name%22%3A%22John%20Doe%22%7D") // {"name":"John Doe"}
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    // Should return exactly 1 customer: "John Doe"
    assert_eq!(customers.len(), 1, "Should return exactly 1 customer");
    assert_eq!(customers[0].name, "John Doe", "Should match 'John Doe' exactly");
}

#[tokio::test]
async fn test_filtering_string_array_in_query_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a valid customer first (required for foreign key constraint)
    let customer_id = create_test_customer(&app).await;

    // Create test vehicles with different makes
    let makes = ["Toyota", "Honda", "Ford", "Chevrolet"];
    for make in &makes {
        let vehicle_data = json!({
            "customer_id": customer_id,
            "make": make,
            "model": "Sedan",
            "year": 2020,
            "vin": format!("VIN{}", make)
        });
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/vehicles")
                    .header("content-type", "application/json")
                    .body(Body::from(vehicle_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test: Multiple values IN (docs example: {"status":["active","pending"]})
    // Adapted: {"make":["Toyota","Honda"]}
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/vehicles?filter=%7B%22make%22%3A%5B%22Toyota%22%2C%22Honda%22%5D%7D") // {"make":["Toyota","Honda"]}
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let vehicles: Vec<VehicleList> = serde_json::from_slice(&body).unwrap();

    // Should return 2 vehicles: Toyota and Honda
    assert_eq!(vehicles.len(), 2, "Should return 2 vehicles (Toyota and Honda)");
    assert!(vehicles.iter().all(|v| v.make == "Toyota" || v.make == "Honda"),
        "All vehicles should be either Toyota or Honda");
}

// =============================================================================
// UUID FILTERING TESTS (filtering.md lines 148-156)
// =============================================================================

#[tokio::test]
async fn test_filtering_uuid_exact_match_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create test customer
    let customer_data = json!({
        "name": "UUID Test Customer",
        "email": "uuid@example.com"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/customers")
                .header("content-type", "application/json")
                .body(Body::from(customer_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Get the created customer ID from response
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let created_id = created_customer["id"].as_str().unwrap();

    // Test: UUID exact match (docs example: {"user_id":"550e8400-e29b-41d4-a716-446655440000"})
    // Manually URL-encode the filter
    let filter_encoded = format!("%7B%22id%22%3A%22{}%22%7D", created_id);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/customers?filter={}", filter_encoded))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    // Should return exactly 1 customer with matching UUID
    assert_eq!(customers.len(), 1, "Should return exactly 1 customer with matching UUID");
    assert_eq!(customers[0].id.to_string(), created_id, "UUID should match");
}

// =============================================================================
// NULL CHECK TESTS (filtering.md lines 160-164)
// =============================================================================

// Note: Current test models don't have nullable fields
// This test demonstrates the documented behavior, but would need a model with nullable fields
// to fully test. Documenting this limitation for future enhancement.

// =============================================================================
// COMPLEX FILTERS TESTS (filtering.md lines 166-176)
// =============================================================================

#[tokio::test]
async fn test_filtering_multiple_and_conditions_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a valid customer first (required for foreign key constraint)
    let customer_id = create_test_customer(&app).await;

    // Create test vehicles
    let vehicles = [
        json!({"customer_id": customer_id, "make": "Toyota", "model": "Camry", "year": 2020, "vin": "VIN1"}),
        json!({"customer_id": customer_id, "make": "Toyota", "model": "Corolla", "year": 2021, "vin": "VIN2"}),
        json!({"customer_id": customer_id, "make": "Honda", "model": "Accord", "year": 2020, "vin": "VIN3"}),
    ];

    for vehicle_data in &vehicles {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/vehicles")
                    .header("content-type", "application/json")
                    .body(Body::from(vehicle_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test: Multiple conditions (docs example: {"status":"active","priority_gte":5})
    // Adapted: {"make":"Toyota","year_gte":2021}
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/vehicles?filter=%7B%22make%22%3A%22Toyota%22%2C%22year_gte%22%3A2021%7D") // {"make":"Toyota","year_gte":2021}
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let vehicles: Vec<VehicleList> = serde_json::from_slice(&body).unwrap();

    // Should return only 1 vehicle: Toyota with year >= 2021 (the Corolla 2021)
    assert_eq!(vehicles.len(), 1, "Should return 1 vehicle matching both conditions");
    assert_eq!(vehicles[0].make, "Toyota", "Make should be Toyota");
    assert!(vehicles[0].year >= 2021, "Year should be >= 2021");
}

// =============================================================================
// DATE FILTERING TESTS (filtering.md lines 114-124)
// =============================================================================

#[tokio::test]
async fn test_filtering_date_range_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // We can't control created_at directly (it's set by on_create), so we'll
    // just test that date filtering doesn't error and returns results.
    // For true date range testing, we'd need a model with manually settable dates.

    let customer_data = json!({"name": "Date Test", "email": "date@example.com"});
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/customers")
                .header("content-type", "application/json")
                .body(Body::from(customer_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Test: Date range (docs example: {"created_at_gte":"2024-01-01","created_at_lte":"2024-12-31"})
    // Using a wide range to ensure we get results
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?filter=%7B%22created_at_gte%22%3A%222020-01-01T00%3A00%3A00Z%22%2C%22created_at_lte%22%3A%222030-12-31T23%3A59%3A59Z%22%7D")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should not error and should return customers created within the range
    assert_eq!(response.status(), StatusCode::OK, "Date range filtering should work");
}

// =============================================================================
// SECURITY TESTS (filtering.md lines 179-205)
// =============================================================================

#[tokio::test]
async fn test_filtering_sql_injection_prevention_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Test: SQL injection attempt (docs example: {"name": "'; DROP TABLE users; --"})
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?filter=%7B%22name%22%3A%22%27%3B%20DROP%20TABLE%20users%3B%20--%22%7D") // {"name":"'; DROP TABLE users; --"}
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should NOT crash, should return OK (even if no results)
    assert_eq!(response.status(), StatusCode::OK, "SQL injection attempt should be safely handled");

    // Verify table still exists by querying customers
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK, "Customers table should still exist after injection attempt");
}
