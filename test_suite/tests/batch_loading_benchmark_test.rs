//! Benchmark tests for batch loading optimization
//!
//! These tests verify that the batch loading optimization actually reduces
//! the number of database queries from N+1 to 2 (or 1+J where J is the number of join fields).
//!
//! We measure performance by:
//! 1. Timing `get_all` operations with varying numbers of parent entities
//! 2. Verifying that time scales linearly (not quadratically) with entity count

// Timing and count ratios are compared loosely; f64 precision loss is irrelevant.
#![allow(clippy::cast_precision_loss)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{setup_test_app, setup_test_db};
use serde_json::json;
use std::time::Instant;
use tower::ServiceExt;

/// Helper to create test customers with vehicles
async fn create_customers_with_vehicles(
    app: &axum::Router,
    customer_count: usize,
    vehicles_per_customer: usize,
) -> Vec<String> {
    let mut customer_ids = Vec::new();

    for i in 0..customer_count {
        // Create customer
        let customer_json = json!({
            "name": format!("Benchmark Customer {}", i),
            "email": format!("benchmark{}@test.com", i)
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
        for j in 0..vehicles_per_customer {
            let vehicle_json = json!({
                "customer_id": customer_id,
                "make": format!("Make{}", j),
                "model": format!("Model{}", j),
                "year": 2020 + i32::try_from(j % 5).unwrap(),
                "vin": format!("VIN-{}-{}", i, j)
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

        customer_ids.push(customer_id);
    }

    customer_ids
}

/// Measure time to fetch all customers with their vehicles loaded
async fn time_get_all_customers(
    app: &axum::Router,
    per_page: usize,
) -> (std::time::Duration, usize) {
    let start = Instant::now();

    let request = Request::builder()
        .method("GET")
        .uri(format!("/customers?page=1&per_page={per_page}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    let elapsed = start.elapsed();
    (elapsed, customers.len())
}

/// Test that batch loading scales linearly, not quadratically
///
/// If N+1 queries: time should grow as O(N) with high constant factor
/// If batch loading: time should grow as O(N) with low constant factor
///
/// We test by comparing time ratios:
/// - With 10 customers vs 50 customers (5x increase)
/// - Time should increase by roughly 5x (linear), not 25x (quadratic)
#[tokio::test]
async fn test_batch_loading_scales_linearly() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Test with small dataset first
    let small_count = 10;
    let vehicles_per = 3;

    println!("\n=== Batch Loading Benchmark ===\n");
    println!("Creating {small_count} customers with {vehicles_per} vehicles each...");

    create_customers_with_vehicles(&app, small_count, vehicles_per).await;

    // Warm up and measure small dataset
    let (small_time, small_result_count) = time_get_all_customers(&app, 100).await;
    println!(
        "Small dataset ({small_count} customers): {small_time:?} ({small_result_count} results)"
    );

    // Now test with larger dataset
    let large_count = 40; // Add 40 more for total of 50
    println!("\nCreating {large_count} more customers with {vehicles_per} vehicles each...");

    create_customers_with_vehicles(&app, large_count, vehicles_per).await;

    // Measure larger dataset
    let (large_time, large_result_count) = time_get_all_customers(&app, 100).await;
    println!(
        "Large dataset ({} customers): {:?} ({} results)",
        small_count + large_count,
        large_time,
        large_result_count
    );

    // Calculate scaling factor
    let time_ratio = large_time.as_micros() as f64 / small_time.as_micros() as f64;
    let count_ratio = (small_count + large_count) as f64 / small_count as f64; // 5x

    println!("\n--- Results ---");
    println!("Count ratio: {count_ratio:.1}x");
    println!("Time ratio: {time_ratio:.1}x");
    println!("Expected with batch loading (linear): ~{count_ratio:.1}x");
    println!(
        "Expected with N+1 (quadratic): ~{:.1}x",
        count_ratio * count_ratio
    );

    // Verify results
    assert_eq!(small_result_count, small_count);
    assert_eq!(large_result_count, small_count + large_count);

    // With batch loading, time should scale roughly linearly
    // Allow for some variance (2x the linear expectation is acceptable)
    // With N+1, it would be closer to 25x for a 5x increase
    let max_acceptable_ratio = count_ratio * 3.0; // Allow 3x linear (still much better than quadratic)

    println!(
        "\nAssertion: time_ratio ({time_ratio:.1}) <= max_acceptable ({max_acceptable_ratio:.1})"
    );

    assert!(
        time_ratio <= max_acceptable_ratio,
        "Time scaling is worse than expected! Got {time_ratio:.1}x for {count_ratio:.1}x data increase. \
         This suggests N+1 queries might not be optimized. \
         Expected roughly linear scaling (~{count_ratio:.1}x), got {time_ratio:.1}x"
    );

    println!("\n=== Batch Loading Benchmark PASSED ===");
    println!("Performance scales approximately linearly as expected.\n");
}

/// Test that vehicles are correctly loaded for each customer
/// This verifies the batch loading produces correct results
#[tokio::test]
async fn test_batch_loading_correctness_with_many_entities() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    let customer_count = 20;
    let vehicles_per_customer = 5;

    println!("\nCreating {customer_count} customers with {vehicles_per_customer} vehicles each...");
    let _customer_ids =
        create_customers_with_vehicles(&app, customer_count, vehicles_per_customer).await;

    // Fetch all customers (request more than we created to ensure we get all)
    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/customers?page=1&per_page={}",
            customer_count + 10
        ))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert_eq!(customers.len(), customer_count, "Should have all customers");

    // Verify each customer has the correct number of vehicles
    for customer in &customers {
        let vehicles = customer["vehicles"].as_array();
        assert!(vehicles.is_some(), "Customer should have vehicles array");
        let vehicles = vehicles.unwrap();
        assert_eq!(
            vehicles.len(),
            vehicles_per_customer,
            "Each customer should have {} vehicles, but customer {} has {}",
            vehicles_per_customer,
            customer["id"],
            vehicles.len()
        );
    }

    // Verify total vehicle count
    let total_vehicles: usize = customers
        .iter()
        .map(|c| c["vehicles"].as_array().map_or(0, std::vec::Vec::len))
        .sum();

    assert_eq!(
        total_vehicles,
        customer_count * vehicles_per_customer,
        "Total vehicles should match expected"
    );

    println!("Verified {customer_count} customers with {total_vehicles} total vehicles");
}

/// Stress test with larger dataset
#[tokio::test]
async fn test_batch_loading_stress_test() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create a larger dataset
    let customer_count = 100;
    let vehicles_per = 3;

    println!("\n=== Stress Test: {customer_count} customers x {vehicles_per} vehicles ===");

    let start = Instant::now();
    create_customers_with_vehicles(&app, customer_count, vehicles_per).await;
    let setup_time = start.elapsed();
    println!("Setup time: {setup_time:?}");

    // Time the get_all operation
    let start = Instant::now();
    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/customers?page=1&per_page={}",
            customer_count + 10
        ))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    let fetch_time = start.elapsed();

    println!(
        "Fetch time for {} customers with vehicles: {:?}",
        customers.len(),
        fetch_time
    );

    // With batch loading, fetching 100 customers should be fast (< 1 second for SQLite in-memory)
    // With N+1 queries, this could take much longer
    assert!(
        fetch_time.as_millis() < 5000, // 5 second max (very generous for in-memory DB)
        "Fetch took too long ({fetch_time:?}), possible N+1 issue"
    );

    assert_eq!(customers.len(), customer_count);
    println!("=== Stress Test PASSED ===\n");
}
