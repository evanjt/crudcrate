// Comprehensive Sorting Tests
// Tests EVERY example from docs/src/features/sorting.md
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
// JSON ARRAY FORMAT TESTS (sorting.md lines 28-36)
// =============================================================================

#[tokio::test]
async fn test_sorting_json_array_with_direction_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customers with specific names for sorting
    let names = ["Zara", "Alice", "Mike", "Bob"];
    for name in &names {
        let customer_data = json!({"name": name, "email": format!("{}@example.com", name.to_lowercase())});
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

    // Test: JSON array format with direction (docs example: sort=["created_at","DESC"])
    // Adapted: sort=["name","DESC"]
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?sort=%5B%22name%22%2C%22DESC%22%5D") // sort=["name","DESC"]
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

    assert!(customers.len() >= 4, "Should return at least 4 customers");

    // Verify descending order (Z to A)
    for i in 0..customers.len()-1 {
        assert!(customers[i].name >= customers[i+1].name,
            "Customers should be in descending order: {} should be >= {}",
            customers[i].name, customers[i+1].name);
    }
}

#[tokio::test]
async fn test_sorting_json_array_default_asc_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customers
    let names = ["Zara", "Alice", "Mike"];
    for name in &names {
        let customer_data = json!({"name": name, "email": format!("{}@example.com", name.to_lowercase())});
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

    // Test: JSON array format without direction (docs example: sort=["name"])
    // Default order should be ASC
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?sort=%5B%22name%22%5D") // sort=["name"]
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

    assert!(customers.len() >= 3, "Should return at least 3 customers");

    // Verify ascending order (A to Z) - default when direction omitted
    for i in 0..customers.len()-1 {
        assert!(customers[i].name <= customers[i+1].name,
            "Customers should be in ascending order (default): {} should be <= {}",
            customers[i].name, customers[i+1].name);
    }
}

// =============================================================================
// REST QUERY PARAMETER TESTS (sorting.md lines 39-54)
// =============================================================================

#[tokio::test]
async fn test_sorting_rest_sort_by_and_order_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a valid customer first (required for foreign key constraint)
    let customer_id = create_test_customer(&app).await;

    // Create vehicles with different years
    for year in [2018, 2020, 2019, 2021] {
        let vehicle_data = json!({
            "customer_id": customer_id,
            "make": "Honda",
            "model": "Accord",
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

    // Test: REST format sort_by + order (docs example: sort_by=created_at&order=DESC)
    // Adapted: sort_by=year&order=DESC
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/vehicles?sort_by=year&order=DESC")
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

    assert!(vehicles.len() >= 4, "Should return at least 4 vehicles");

    // Verify descending order
    for i in 0..vehicles.len()-1 {
        assert!(vehicles[i].year >= vehicles[i+1].year,
            "Vehicles should be sorted by year descending");
    }
}

#[tokio::test]
async fn test_sorting_rest_sort_and_order_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a valid customer first (required for foreign key constraint)
    let customer_id = create_test_customer(&app).await;

    // Create vehicles
    for year in [2020, 2018, 2022, 2019] {
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

    // Test: REST format sort + order (docs example: sort=created_at&order=DESC)
    // Adapted: sort=year&order=ASC
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/vehicles?sort=year&order=ASC")
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

    assert!(vehicles.len() >= 4, "Should return at least 4 vehicles");

    // Verify ascending order
    for i in 0..vehicles.len()-1 {
        assert!(vehicles[i].year <= vehicles[i+1].year,
            "Vehicles should be sorted by year ascending");
    }
}

#[tokio::test]
async fn test_sorting_rest_default_asc_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customers
    let names = ["Delta", "Alpha", "Charlie", "Bravo"];
    for name in &names {
        let customer_data = json!({"name": name, "email": format!("{}@example.com", name.to_lowercase())});
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

    // Test: REST format without order (docs example: sort_by=name)
    // Default order should be ASC
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?sort_by=name")
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

    assert!(customers.len() >= 4, "Should return at least 4 customers");

    // Verify ascending order (default when order omitted)
    for i in 0..customers.len()-1 {
        assert!(customers[i].name <= customers[i+1].name,
            "Customers should be in ascending order (default)");
    }
}

// =============================================================================
// CASE-INSENSITIVE DIRECTION TESTS (sorting.md lines 56-62)
// =============================================================================

#[tokio::test]
async fn test_sorting_case_insensitive_asc_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a valid customer first (required for foreign key constraint)
    let customer_id = create_test_customer(&app).await;

    // Create vehicles
    for year in [2021, 2019, 2020] {
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

    // Test: Case insensitive "asc" (docs: ASC, asc, Asc all work)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/vehicles?sort_by=year&order=asc") // lowercase "asc"
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

    assert!(vehicles.len() >= 3, "Should return at least 3 vehicles");

    // Verify ascending order works with lowercase "asc"
    for i in 0..vehicles.len()-1 {
        assert!(vehicles[i].year <= vehicles[i+1].year,
            "Lowercase 'asc' should work for ascending sort");
    }
}

#[tokio::test]
async fn test_sorting_case_insensitive_desc_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a valid customer first (required for foreign key constraint)
    let customer_id = create_test_customer(&app).await;

    // Create vehicles
    for year in [2019, 2021, 2020] {
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

    // Test: Case insensitive "desc" (docs: DESC, desc, Desc all work)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/vehicles?sort_by=year&order=desc") // lowercase "desc"
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

    assert!(vehicles.len() >= 3, "Should return at least 3 vehicles");

    // Verify descending order works with lowercase "desc"
    for i in 0..vehicles.len()-1 {
        assert!(vehicles[i].year >= vehicles[i+1].year,
            "Lowercase 'desc' should work for descending sort");
    }
}

#[tokio::test]
async fn test_sorting_mixed_case_direction_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customers
    let names = ["Yankee", "Alpha", "Mike"];
    for name in &names {
        let customer_data = json!({"name": name, "email": format!("{}@example.com", name.to_lowercase())});
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

    // Test: Mixed case "Asc" (docs: ASC, asc, Asc all work)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?sort_by=name&order=Asc") // mixed case "Asc"
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

    assert!(customers.len() >= 3, "Should return at least 3 customers");

    // Verify ascending order works with mixed case "Asc"
    for i in 0..customers.len()-1 {
        assert!(customers[i].name <= customers[i+1].name,
            "Mixed case 'Asc' should work for ascending sort");
    }
}

// =============================================================================
// SORT BY TYPE TESTS (sorting.md lines 66-105)
// =============================================================================

#[tokio::test]
async fn test_sorting_strings_asc_and_desc_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customers with names for A-Z and Z-A testing
    let names = ["Alice", "Bob", "Charlie", "Diana"];
    for name in &names {
        let customer_data = json!({"name": name, "email": format!("{}@example.com", name.to_lowercase())});
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

    // Test: Strings A to Z (docs example: sort=["name","ASC"])
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?sort=%5B%22name%22%2C%22ASC%22%5D") // sort=["name","ASC"]
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

    // Verify A to Z order
    for i in 0..customers.len()-1 {
        assert!(customers[i].name <= customers[i+1].name, "Should be A to Z");
    }

    // Test: Strings Z to A (docs example: sort=["name","DESC"])
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?sort=%5B%22name%22%2C%22DESC%22%5D") // sort=["name","DESC"]
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

    // Verify Z to A order
    for i in 0..customers.len()-1 {
        assert!(customers[i].name >= customers[i+1].name, "Should be Z to A");
    }
}

#[tokio::test]
async fn test_sorting_numbers_asc_and_desc_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a valid customer first (required for foreign key constraint)
    let customer_id = create_test_customer(&app).await;

    // Create vehicles with different years (numbers)
    for year in [2015, 2020, 2018, 2022] {
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

    // Test: Numbers lowest to highest (docs example: sort=["priority","ASC"])
    // Adapted: sort=["year","ASC"]
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/vehicles?sort=%5B%22year%22%2C%22ASC%22%5D") // sort=["year","ASC"]
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

    // Verify lowest to highest
    for i in 0..vehicles.len()-1 {
        assert!(vehicles[i].year <= vehicles[i+1].year, "Should be lowest to highest");
    }

    // Test: Numbers highest to lowest (docs example: sort=["priority","DESC"])
    // Adapted: sort=["year","DESC"]
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/vehicles?sort=%5B%22year%22%2C%22DESC%22%5D") // sort=["year","DESC"]
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

    // Verify highest to lowest
    for i in 0..vehicles.len()-1 {
        assert!(vehicles[i].year >= vehicles[i+1].year, "Should be highest to lowest");
    }
}

#[tokio::test]
async fn test_sorting_dates_oldest_and_newest_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customers (created_at will be auto-set)
    for i in 0..4 {
        let customer_data = json!({"name": format!("Customer {}", i), "email": format!("customer{}@example.com", i)});
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
        // Small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Test: Dates oldest first (docs example: sort=["created_at","ASC"])
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?sort=%5B%22created_at%22%2C%22ASC%22%5D") // sort=["created_at","ASC"]
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

    // Verify oldest first
    for i in 0..customers.len()-1 {
        assert!(customers[i].created_at <= customers[i+1].created_at, "Should be oldest first");
    }

    // Test: Dates newest first (docs example: sort=["created_at","DESC"])
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?sort=%5B%22created_at%22%2C%22DESC%22%5D") // sort=["created_at","DESC"]
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

    // Verify newest first
    for i in 0..customers.len()-1 {
        assert!(customers[i].created_at >= customers[i+1].created_at, "Should be newest first");
    }
}

// =============================================================================
// DEFAULT SORT BEHAVIOR TESTS (sorting.md lines 107-116)
// =============================================================================

#[tokio::test]
async fn test_sorting_default_behavior_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customers
    for i in 0..3 {
        let customer_data = json!({"name": format!("Customer {}", i), "email": format!("c{}@example.com", i)});
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

    // Test: No sort specified (docs: Default: ORDER BY id ASC)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers") // No sort parameter
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should not error, should return results
    assert_eq!(response.status(), StatusCode::OK, "Default sort should work without errors");
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    assert!(customers.len() >= 3, "Should return customers with default sort");
}

// =============================================================================
// INVALID SORT FALLBACK TESTS (sorting.md lines 277-288)
// =============================================================================

#[tokio::test]
async fn test_sorting_invalid_field_fallback_as_documented() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customers
    let customer_data = json!({"name": "Test Customer", "email": "test@example.com"});
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

    // Test: Invalid field (docs example: sort=["nonexistent","ASC"])
    // Should fall back to default sort
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?sort=%5B%22nonexistent%22%2C%22ASC%22%5D") // sort=["nonexistent","ASC"]
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should not error, should fall back to default
    assert_eq!(response.status(), StatusCode::OK, "Invalid sort field should fall back to default");

    // Test: Invalid format (docs example: sort=not-an-array)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?sort=not-an-array")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should not error, should fall back to default
    assert_eq!(response.status(), StatusCode::OK, "Invalid sort format should fall back to default");
}
