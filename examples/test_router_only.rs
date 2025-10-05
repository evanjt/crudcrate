/// Test to isolate whether stack overflow happens during router creation or runtime
use axum::Router;
use sea_orm::{Database, DatabaseConnection};
use std::time::Duration;
use tower_http::cors::CorsLayer;

// Import local models
mod models;
use models::{Customer, Vehicle};
use crudcrate::traits::CRUDResource;

async fn setup_database() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    db
}

#[tokio::main]
async fn main() {
    println!("Testing database setup...");

    // Test 1: Can we create database connection?
    let db = setup_database().await;
    println!("✅ Database connection created");

    // Test 2: Can we create individual routers?
    println!("Testing router creation...");

    // This should trigger the stack overflow if it's utoipa-related
    let customer_router = Customer::router(&db);
    println!("✅ Customer router created successfully");

    let vehicle_router = Vehicle::router(&db);
    println!("✅ Vehicle router created successfully");

    // Test 3: Can we create the combined app?
    let app = Router::new()
        .nest("/customers", customer_router.into())
        .nest("/vehicles", vehicle_router.into())
        .layer(CorsLayer::default());

    println!("✅ Combined app router created successfully");
    println!("Stack overflow is NOT caused by utoipa schema generation");
}