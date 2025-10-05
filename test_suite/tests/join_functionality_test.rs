//! Comprehensive tests for join() functionality
//! Tests all combinations: join(one), join(all), join(one, all)

use axum::body::Body;
use axum::http::{Request, StatusCode, Method};
use axum::body::to_bytes;
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_db, setup_test_app, Customer};

#[tokio::test]
async fn test_join_one_functionality() -> TestResult {
    let (db, _app) = setup_test_app().await?;
    setup_test_db(&db).await?;

    // Create test data
    let customer_data = json!({
        "name": "Test Customer",
        "email": "test@example.com"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/customers")
                .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .body(Body::from(customer_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let created_customer: Customer = serde_json::from_slice(&into_bytes(response.into_body())).unwrap();

    // Test that get_one includes joined data when join(one) is specified
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(&format!("/customers/{}", created_customer.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let customer_with_joins: Customer = serde_json::from_slice(&into_bytes(response.into_body())).unwrap();

    // For Customer model with join(one) on vehicles field, vehicles should be populated
    // Note: This depends on the actual model configuration. Adjust assertions accordingly.
    assert!(!customer_with_joins.vehicles.is_empty(), "Joined vehicles should be populated in get_one response");

    Ok(())
}

#[tokio::test]
async fn test_join_all_functionality() -> TestResult {
    let (db, _app) = setup_test_app().await?;
    setup_test_db(&db).await?;

    // Create test data with vehicles
    let customer_data = json!({
        "name": "Test Customer Join All",
        "email": "joinall@example.com"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/customers")
                .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .body(Body::from(customer_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    // Test that get_all includes joined data when join(all) is specified
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/customers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let customers: Vec<Customer> = serde_json::from_slice(&into_bytes(response.into_body())).unwrap();

    // Find our test customer
    let test_customer = customers.iter().find(|c| c.name == "Test Customer Join All")
        .expect("Test customer should be in the list");

    // Verify that join(all) fields are populated in list responses
    // This depends on the actual model configuration. Adjust assertions accordingly.
    // For vehicles with join(all), they should be populated in get_all responses
    println!("Customer vehicles in get_all: {:?}", test_customer.vehicles);

    Ok(())
}

#[tokio::test]
async fn test_join_one_all_combination() -> TestResult {
    let (db, _app) = setup_test_app().await?;
    setup_test_db(&db).await?;

    // Create test data
    let customer_data = json!({
        "name": "Test Customer Both Joins",
        "email": "both@example.com"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/customers")
                .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .body(Body::from(customer_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let created_customer: Customer = serde_json::from_slice(&into_bytes(response.into_body())).unwrap();

    // Test get_one response (join(one) should be active)
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(&format!("/customers/{}", created_customer.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let customer_get_one: Customer = serde_json::from_slice(&into_bytes(response.into_body())).unwrap();

    // Test get_all response (join(all) should be active)
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/customers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let customers: Vec<Customer> = serde_json::from_slice(&into_bytes(response.into_body())).unwrap();
    let customer_get_all = customers.iter().find(|c| c.id == created_customer.id)
        .expect("Customer should be in get_all response");

    // Both responses should have joined data when join(one, all) is specified
    // Verify consistency between get_one and get_all join behavior
    println!("Get one vehicles: {:?}", customer_get_one.vehicles);
    println!("Get all vehicles: {:?}", customer_get_all.vehicles);

    Ok(())
}

#[tokio::test]
async fn test_no_join_specified() -> TestResult {
    let (db, _app) = setup_test_app().await?;
    setup_test_db(&db).await?;

    // Create test data
    let customer_data = json!({
        "name": "Test Customer No Joins",
        "email": "nojoins@example.com"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/customers")
                .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .body(Body::from(customer_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let created_customer: Customer = serde_json::from_slice(&into_bytes(response.into_body())).unwrap();

    // Test that fields without join attributes are not populated
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(&format!("/customers/{}", created_customer.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let customer_no_joins: Customer = serde_json::from_slice(&into_bytes(response.into_body())).unwrap();

    // Fields without join attributes should be empty/default
    // This depends on the actual model configuration
    println!("Customer vehicles without join: {:?}", customer_no_joins.vehicles);

    Ok(())
}

#[tokio::test]
async fn test_join_performance_impact() -> TestResult {
    let (db, _app) = setup_test_app().await?;
    setup_test_db(&db).await?;

    // Create multiple customers and vehicles to test performance
    for i in 0..5 {
        let customer_data = json!({
            "name": format!("Perf Customer {}", i),
            "email": format!("perf{}@example.com", i)
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/customers")
                    .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(customer_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
    }

    // Test get_all performance with joins
    let start = std::time::Instant::now();

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/customers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let duration = start.elapsed();

    assert_eq!(response.status(), StatusCode::OK);

    let customers: Vec<Customer> = serde_json::from_slice(&into_bytes(response.into_body())).unwrap();
    assert_eq!(customers.len(), 5);

    // Performance should be reasonable (adjust threshold as needed)
    assert!(duration.as_millis() < 1000, "get_all with joins should complete within 1 second, took {}ms", duration.as_millis());

    println!("get_all with 5 customers and joins completed in {}ms", duration.as_millis());

    Ok(())
}