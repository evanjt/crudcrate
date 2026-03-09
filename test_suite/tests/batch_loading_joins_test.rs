//! Tests for batch loading optimization in join queries
//!
//! These tests verify that the join loading mechanism correctly loads related
//! entities, both with the current N+1 implementation and the future batch
//! loading optimization.
//!
//! The N+1 problem occurs when:
//! 1. We load N parent entities
//! 2. For each parent, we execute a separate query for related entities
//! Result: 1 + N queries (instead of 2 queries with batch loading)
//!
//! Batch loading solution:
//! 1. Load N parent entities (1 query)
//! 2. Collect all parent IDs
//! 3. Load all related entities for all parents in one query (1 query)
//! 4. Group related entities by parent ID
//! Result: 2 queries total (regardless of N)
//!
//! These tests ensure that:
//! 1. The current implementation returns correct data
//! 2. After optimization, the same data is returned
//! 3. Edge cases are handled correctly

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;
use url_escape;

mod common;
use common::{setup_test_app, setup_test_db};

use crate::common::customer::CustomerList;
use crate::common::models::vehicle::VehicleList;

// =============================================================================
// CORRECTNESS TESTS - Verify data is loaded correctly
// =============================================================================

/// Test that get_all correctly loads vehicles for multiple customers
/// This is the core test for batch loading - with N+1, this does 1 + N queries
/// With batch loading, this should do 2 queries
#[tokio::test]
async fn test_get_all_loads_related_entities_for_multiple_parents() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create 3 customers, each with different numbers of vehicles
    let customers_data = vec![
        (
            "Customer A",
            "a@test.com",
            vec!["Toyota Camry", "Honda Accord"],
        ),
        ("Customer B", "b@test.com", vec!["Ford F-150"]),
        (
            "Customer C",
            "c@test.com",
            vec!["Tesla Model 3", "BMW 3 Series", "Audi A4"],
        ),
    ];

    let mut created_customers = Vec::new();

    for (name, email, vehicles) in &customers_data {
        // Create customer
        let customer_json = json!({
            "name": name,
            "email": email
        });

        let request = Request::builder()
            .method("POST")
            .uri("/customers")
            .header("content-type", "application/json")
            .body(Body::from(customer_json.to_string()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let customer_id = customer["id"].as_str().unwrap().to_string();

        // Create vehicles for this customer
        for (i, vehicle_name) in vehicles.iter().enumerate() {
            let parts: Vec<&str> = vehicle_name.split(' ').collect();
            let make = parts[0];
            let model = parts.get(1).unwrap_or(&"");

            let vehicle_json = json!({
                "customer_id": customer_id,
                "make": make,
                "model": model,
                "year": 2020 + i,
                "vin": format!("VIN{}{}_{}", name.chars().next().unwrap(), i, i)
            });

            let request = Request::builder()
                .method("POST")
                .uri("/vehicles")
                .header("content-type", "application/json")
                .body(Body::from(vehicle_json.to_string()))
                .unwrap();

            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::CREATED);
        }

        created_customers.push((customer_id, vehicles.len()));
    }

    // Now fetch all customers and verify vehicles are loaded correctly
    let request = Request::builder()
        .method("GET")
        .uri("/customers")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    // Verify each customer has the correct number of vehicles
    for (customer_id, expected_vehicle_count) in &created_customers {
        let customer = customers
            .iter()
            .find(|c| c.id.to_string() == *customer_id)
            .expect("Customer should be in list");

        assert_eq!(
            customer.vehicles.len(),
            *expected_vehicle_count,
            "Customer {} should have {} vehicles, got {}",
            customer.name,
            expected_vehicle_count,
            customer.vehicles.len()
        );
    }
}

/// Test that batch loading works correctly with pagination
/// When loading page 2 of customers, only those customers' vehicles should be loaded
#[tokio::test]
async fn test_batch_loading_respects_pagination() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create 5 customers with 1 vehicle each
    for i in 0..5 {
        let customer_json = json!({
            "name": format!("Customer {}", i),
            "email": format!("customer{}@test.com", i)
        });

        let request = Request::builder()
            .method("POST")
            .uri("/customers")
            .header("content-type", "application/json")
            .body(Body::from(customer_json.to_string()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let customer_id = customer["id"].as_str().unwrap();

        let vehicle_json = json!({
            "customer_id": customer_id,
            "make": "Make",
            "model": format!("Model{}", i),
            "year": 2020 + i,
            "vin": format!("VINPAGE{}", i)
        });

        let request = Request::builder()
            .method("POST")
            .uri("/vehicles")
            .header("content-type", "application/json")
            .body(Body::from(vehicle_json.to_string()))
            .unwrap();

        let _ = app.clone().oneshot(request).await.unwrap();
    }

    // Fetch first page (2 customers)
    let request = Request::builder()
        .method("GET")
        .uri("/customers?range=[0,1]")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let page1_customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    assert_eq!(page1_customers.len(), 2, "Page 1 should have 2 customers");

    // Each customer should have exactly 1 vehicle
    for customer in &page1_customers {
        assert_eq!(
            customer.vehicles.len(),
            1,
            "Each customer should have 1 vehicle"
        );
    }

    // Fetch second page
    let request = Request::builder()
        .method("GET")
        .uri("/customers?range=[2,3]")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let page2_customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    assert_eq!(page2_customers.len(), 2, "Page 2 should have 2 customers");

    for customer in &page2_customers {
        assert_eq!(
            customer.vehicles.len(),
            1,
            "Each customer should have 1 vehicle"
        );
    }
}

/// Test that customers with no vehicles get empty arrays (not null)
#[tokio::test]
async fn test_batch_loading_handles_empty_relationships() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create 3 customers: one with vehicles, one without, one with vehicles
    let customer1_json = json!({
        "name": "Has Vehicles",
        "email": "has@test.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(customer1_json.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer1: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let customer1_id = customer1["id"].as_str().unwrap();

    // Add vehicle to customer1
    let vehicle_json = json!({
        "customer_id": customer1_id,
        "make": "Toyota",
        "model": "Corolla",
        "year": 2023,
        "vin": "VINEMPTY1"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/vehicles")
        .header("content-type", "application/json")
        .body(Body::from(vehicle_json.to_string()))
        .unwrap();

    let _ = app.clone().oneshot(request).await.unwrap();

    // Customer 2 - no vehicles
    let customer2_json = json!({
        "name": "No Vehicles",
        "email": "no@test.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(customer2_json.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer2: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let customer2_id = customer2["id"].as_str().unwrap().to_string();

    // Customer 3 - has vehicles
    let customer3_json = json!({
        "name": "Also Has Vehicles",
        "email": "also@test.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(customer3_json.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer3: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let customer3_id = customer3["id"].as_str().unwrap();

    let vehicle_json = json!({
        "customer_id": customer3_id,
        "make": "Honda",
        "model": "Civic",
        "year": 2023,
        "vin": "VINEMPTY2"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/vehicles")
        .header("content-type", "application/json")
        .body(Body::from(vehicle_json.to_string()))
        .unwrap();

    let _ = app.clone().oneshot(request).await.unwrap();

    // Fetch all customers
    let request = Request::builder()
        .method("GET")
        .uri("/customers")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    // Find customer without vehicles
    let no_vehicle_customer = customers
        .iter()
        .find(|c| c.id.to_string() == customer2_id)
        .expect("Customer without vehicles should be in list");

    // Should have empty array, not null/missing
    assert!(
        no_vehicle_customer.vehicles.is_empty(),
        "Customer without vehicles should have empty array"
    );

    // Other customers should have vehicles
    let has_vehicle_customers: Vec<_> = customers
        .iter()
        .filter(|c| c.id.to_string() != customer2_id)
        .collect();

    for customer in has_vehicle_customers {
        assert!(
            !customer.vehicles.is_empty(),
            "Customer with vehicles should have non-empty array"
        );
    }
}

/// Test batch loading with many-to-many-like structure (vehicles -> parts)
#[tokio::test]
async fn test_batch_loading_with_nested_relationships() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer with 2 vehicles, each with 2 parts
    let customer_json = json!({
        "name": "Nested Test Customer",
        "email": "nested@test.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(customer_json.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let customer_id = customer["id"].as_str().unwrap();

    let mut vehicle_ids = Vec::new();

    // Create 2 vehicles
    for i in 0..2 {
        let vehicle_json = json!({
            "customer_id": customer_id,
            "make": format!("Make{}", i),
            "model": format!("Model{}", i),
            "year": 2023,
            "vin": format!("VINNESTED{}", i)
        });

        let request = Request::builder()
            .method("POST")
            .uri("/vehicles")
            .header("content-type", "application/json")
            .body(Body::from(vehicle_json.to_string()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let vehicle: serde_json::Value = serde_json::from_slice(&body).unwrap();
        vehicle_ids.push(vehicle["id"].as_str().unwrap().to_string());
    }

    // Create 2 parts for each vehicle
    for (v_idx, vehicle_id) in vehicle_ids.iter().enumerate() {
        for p_idx in 0..2 {
            let part_json = json!({
                "vehicle_id": vehicle_id,
                "name": format!("Part V{} P{}", v_idx, p_idx),
                "part_number": format!("PN-{}-{}", v_idx, p_idx),
                "category": "Test",
                "in_stock": true
            });

            let request = Request::builder()
                .method("POST")
                .uri("/vehicle_parts")
                .header("content-type", "application/json")
                .body(Body::from(part_json.to_string()))
                .unwrap();

            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::CREATED);
        }
    }

    // Fetch all vehicles and verify nested parts are loaded
    let request = Request::builder()
        .method("GET")
        .uri("/vehicles")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let vehicles: Vec<VehicleList> = serde_json::from_slice(&body).unwrap();

    // Each vehicle should have 2 parts
    for vehicle_id in &vehicle_ids {
        let vehicle = vehicles
            .iter()
            .find(|v| v.id.to_string() == *vehicle_id)
            .expect("Vehicle should be in list");

        assert_eq!(
            vehicle.parts.len(),
            2,
            "Vehicle {} should have 2 parts, got {}",
            vehicle_id,
            vehicle.parts.len()
        );
    }
}

/// Test batch loading with filtering
/// When filtering customers, only matching customers' vehicles should be loaded
#[tokio::test]
async fn test_batch_loading_with_filtering() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customers with distinct names for filtering
    // Using unique email prefix to filter by exact match
    let customers_data = vec![
        ("Alice FilterTest", "filtersmith1@test.com"),
        ("Bob FilterTest", "filtersmith2@test.com"),
        ("Charlie Jones", "filterjones@test.com"),
    ];

    for (name, email) in &customers_data {
        let customer_json = json!({
            "name": name,
            "email": email
        });

        let request = Request::builder()
            .method("POST")
            .uri("/customers")
            .header("content-type", "application/json")
            .body(Body::from(customer_json.to_string()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let customer_id = customer["id"].as_str().unwrap();

        // Add a vehicle for each customer
        let vehicle_json = json!({
            "customer_id": customer_id,
            "make": "Toyota",
            "model": "Camry",
            "year": 2023,
            "vin": format!("VINFILTER{}", name.chars().next().unwrap())
        });

        let request = Request::builder()
            .method("POST")
            .uri("/vehicles")
            .header("content-type", "application/json")
            .body(Body::from(vehicle_json.to_string()))
            .unwrap();

        let _ = app.clone().oneshot(request).await.unwrap();
    }

    // Filter customers by exact name match
    let filter = json!({"name": "Charlie Jones"});
    let filter_str = filter.to_string();
    let encoded_filter = url_escape::encode_component(&filter_str);
    let request = Request::builder()
        .method("GET")
        .uri(format!("/customers?filter={}", encoded_filter))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let filtered_customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    // Should only get the Jones customer
    assert_eq!(filtered_customers.len(), 1, "Should have 1 Jones customer");

    // Should have their vehicles loaded
    let customer = &filtered_customers[0];
    assert_eq!(
        customer.name, "Charlie Jones",
        "Filtered customer should be Jones"
    );
    assert_eq!(
        customer.vehicles.len(),
        1,
        "Filtered customer should have 1 vehicle"
    );
}

/// Test that batch loading works with sorting
#[tokio::test]
async fn test_batch_loading_with_sorting() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customers
    let names = vec!["Zack", "Alice", "Mike"];

    for name in &names {
        let customer_json = json!({
            "name": name,
            "email": format!("{}@test.com", name.to_lowercase())
        });

        let request = Request::builder()
            .method("POST")
            .uri("/customers")
            .header("content-type", "application/json")
            .body(Body::from(customer_json.to_string()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let customer_id = customer["id"].as_str().unwrap();

        // Add vehicle
        let vehicle_json = json!({
            "customer_id": customer_id,
            "make": format!("{}'s Car", name),
            "model": "Sedan",
            "year": 2023,
            "vin": format!("VINSORT{}", name)
        });

        let request = Request::builder()
            .method("POST")
            .uri("/vehicles")
            .header("content-type", "application/json")
            .body(Body::from(vehicle_json.to_string()))
            .unwrap();

        let _ = app.clone().oneshot(request).await.unwrap();
    }

    // Get customers sorted by name ascending
    let request = Request::builder()
        .method("GET")
        .uri("/customers?sort=name&order=asc")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    // Verify sorted order
    let sorted_names: Vec<&str> = customers.iter().map(|c| c.name.as_str()).collect();
    assert!(
        sorted_names.windows(2).all(|w| w[0] <= w[1]),
        "Customers should be sorted by name ascending"
    );

    // Verify each customer has their vehicle
    for customer in &customers {
        assert_eq!(
            customer.vehicles.len(),
            1,
            "Each customer should have 1 vehicle"
        );
        assert!(
            customer.vehicles[0].make.contains(&customer.name),
            "Vehicle make should contain customer name"
        );
    }
}

// =============================================================================
// EDGE CASE TESTS
// =============================================================================

/// Test batch loading with a single parent entity
#[tokio::test]
async fn test_batch_loading_single_parent() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create just one customer with multiple vehicles
    let customer_json = json!({
        "name": "Solo Customer",
        "email": "solo@test.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(customer_json.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let customer_id = customer["id"].as_str().unwrap();

    // Add 3 vehicles
    for i in 0..3 {
        let vehicle_json = json!({
            "customer_id": customer_id,
            "make": format!("Make{}", i),
            "model": format!("Model{}", i),
            "year": 2020 + i,
            "vin": format!("VINSOLO{}", i)
        });

        let request = Request::builder()
            .method("POST")
            .uri("/vehicles")
            .header("content-type", "application/json")
            .body(Body::from(vehicle_json.to_string()))
            .unwrap();

        let _ = app.clone().oneshot(request).await.unwrap();
    }

    // Fetch all customers (should be just one)
    let request = Request::builder()
        .method("GET")
        .uri("/customers")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    assert_eq!(customers.len(), 1, "Should have 1 customer");
    assert_eq!(
        customers[0].vehicles.len(),
        3,
        "Customer should have 3 vehicles"
    );
}

/// Test batch loading with all parents having no children
#[tokio::test]
async fn test_batch_loading_all_empty_relationships() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create 3 customers with no vehicles
    for i in 0..3 {
        let customer_json = json!({
            "name": format!("Empty Customer {}", i),
            "email": format!("empty{}@test.com", i)
        });

        let request = Request::builder()
            .method("POST")
            .uri("/customers")
            .header("content-type", "application/json")
            .body(Body::from(customer_json.to_string()))
            .unwrap();

        let _ = app.clone().oneshot(request).await.unwrap();
    }

    // Fetch all customers
    let request = Request::builder()
        .method("GET")
        .uri("/customers")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    assert_eq!(customers.len(), 3, "Should have 3 customers");

    for customer in &customers {
        assert!(
            customer.vehicles.is_empty(),
            "All customers should have empty vehicles array"
        );
    }
}

/// Test batch loading with large number of parents (stress test)
#[tokio::test]
async fn test_batch_loading_many_parents() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    let num_customers = 20;

    // Create many customers, each with 1 vehicle
    for i in 0..num_customers {
        let customer_json = json!({
            "name": format!("Batch Customer {}", i),
            "email": format!("batch{}@test.com", i)
        });

        let request = Request::builder()
            .method("POST")
            .uri("/customers")
            .header("content-type", "application/json")
            .body(Body::from(customer_json.to_string()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let customer_id = customer["id"].as_str().unwrap();

        let vehicle_json = json!({
            "customer_id": customer_id,
            "make": "BatchMake",
            "model": format!("Model{}", i),
            "year": 2020,
            "vin": format!("VINBATCH{:03}", i)
        });

        let request = Request::builder()
            .method("POST")
            .uri("/vehicles")
            .header("content-type", "application/json")
            .body(Body::from(vehicle_json.to_string()))
            .unwrap();

        let _ = app.clone().oneshot(request).await.unwrap();
    }

    // Fetch all customers with explicit pagination to get all items
    // Default pagination limit is 10, so we need to request a larger range
    let request = Request::builder()
        .method("GET")
        .uri("/customers?range=[0,99]")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        customers.len(),
        num_customers,
        "Should have all {} customers",
        num_customers
    );

    // Every customer should have exactly 1 vehicle
    for customer in &customers {
        assert_eq!(
            customer.vehicles.len(),
            1,
            "Each customer should have 1 vehicle"
        );
    }
}

/// Test that get_one still works correctly (should not be affected by batch loading)
#[tokio::test]
async fn test_get_one_unaffected_by_batch_loading() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a customer with vehicles
    let customer_json = json!({
        "name": "GetOne Test",
        "email": "getone@test.com"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/customers")
        .header("content-type", "application/json")
        .body(Body::from(customer_json.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let customer_id = customer["id"].as_str().unwrap();

    // Add 2 vehicles
    for i in 0..2 {
        let vehicle_json = json!({
            "customer_id": customer_id,
            "make": format!("GetOneMake{}", i),
            "model": "Model",
            "year": 2023,
            "vin": format!("VINGETONE{}", i)
        });

        let request = Request::builder()
            .method("POST")
            .uri("/vehicles")
            .header("content-type", "application/json")
            .body(Body::from(vehicle_json.to_string()))
            .unwrap();

        let _ = app.clone().oneshot(request).await.unwrap();
    }

    // Fetch single customer by ID
    let request = Request::builder()
        .method("GET")
        .uri(format!("/customers/{}", customer_id))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    // Use serde_json::Value to check vehicles field
    let customer_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify vehicles are loaded in get_one response
    let vehicles = customer_response["vehicles"]
        .as_array()
        .expect("vehicles should be an array");
    assert_eq!(
        vehicles.len(),
        2,
        "get_one should return customer with 2 vehicles"
    );
}
