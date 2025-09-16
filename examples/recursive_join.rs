/// Join Loading Example
///
/// Demonstrates automatic relationship loading with #[crudcrate(join(one, all))]
/// Customer → Vehicle relationships are loaded automatically in API responses.
///
/// Run with: cargo run --example recursive_join
use axum::Router;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, Set, entity::prelude::*,
};
use std::time::Duration;
use tokio;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

// Import shared models with join configuration
use crudcrate::traits::CRUDResource;
use shared_models::*;

// ============================================================================
// DATABASE SETUP
// ============================================================================

async fn setup_database() -> DatabaseConnection {
    let mut opt = ConnectOptions::new("sqlite::memory:".to_owned());
    // Force single connection to ensure in-memory DB stays alive
    opt.max_connections(1)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(30))
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(300)) // Keep connection alive longer
        .max_lifetime(Duration::from_secs(3600)) // 1 hour
        .sqlx_logging(false);

    let db = Database::connect(opt).await.unwrap();

    // Create tables
    create_tables(&db).await;
    seed_data(&db).await;

    db
}

async fn create_tables(db: &DatabaseConnection) {
    use sea_orm::Schema;

    let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);

    // Create all tables
    println!("Creating customers table...");
    let stmt = schema.create_table_from_entity(CustomerEntity);
    match db.execute(db.get_database_backend().build(&stmt)).await {
        Ok(_) => println!("✅ customers table created"),
        Err(e) => println!("❌ Failed to create customers table: {:?}", e),
    }

    println!("Creating vehicles table...");
    let stmt = schema.create_table_from_entity(VehicleEntity);
    match db.execute(db.get_database_backend().build(&stmt)).await {
        Ok(_) => println!("✅ vehicles table created"),
        Err(e) => println!("❌ Failed to create vehicles table: {:?}", e),
    }

    println!("Creating vehicle_parts table...");
    let stmt = schema.create_table_from_entity(VehiclePartEntity);
    match db.execute(db.get_database_backend().build(&stmt)).await {
        Ok(_) => println!("✅ vehicle_parts table created"),
        Err(e) => println!("❌ Failed to create vehicle_parts table: {:?}", e),
    }

    println!("Creating maintenance_records table...");
    let stmt = schema.create_table_from_entity(MaintenanceRecordEntity);
    match db.execute(db.get_database_backend().build(&stmt)).await {
        Ok(_) => println!("✅ maintenance_records table created"),
        Err(e) => println!("❌ Failed to create maintenance_records table: {:?}", e),
    }
}

async fn seed_data(db: &DatabaseConnection) {
    let customer_id = Uuid::new_v4();
    let vehicle1_id = Uuid::new_v4();
    let vehicle2_id = Uuid::new_v4();

    // Create customer
    let customer = customer::ActiveModel {
        id: Set(customer_id),
        name: Set("John Smith".to_owned()),
        email: Set("john@example.com".to_owned()),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        ..Default::default()
    };
    println!("Creating customer...");
    match customer.insert(db).await {
        Ok(_) => println!("✅ Customer created"),
        Err(e) => println!("❌ Failed to create customer: {:?}", e),
    }

    // Create vehicles
    let vehicle1 = vehicle::ActiveModel {
        id: Set(vehicle1_id),
        customer_id: Set(customer_id),
        make: Set("Toyota".to_owned()),
        model: Set("Camry".to_owned()),
        year: Set(2020),
        vin: Set("1HGBH41JXMN109186".to_owned()),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        ..Default::default()
    };
    vehicle1.insert(db).await.unwrap();

    let vehicle2 = vehicle::ActiveModel {
        id: Set(vehicle2_id),
        customer_id: Set(customer_id),
        make: Set("Honda".to_owned()),
        model: Set("Civic".to_owned()),
        year: Set(2019),
        vin: Set("2HGBH41JXMN109187".to_owned()),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        ..Default::default()
    };
    vehicle2.insert(db).await.unwrap();

    // Create parts for vehicle1
    let part1 = vehicle_part::ActiveModel {
        id: Set(Uuid::new_v4()),
        vehicle_id: Set(vehicle1_id),
        name: Set("Front Brake Pads".to_owned()),
        part_number: Set("BP-001".to_owned()),
        category: Set("Brakes".to_owned()),
        price: Set(Some("89.99".parse::<rust_decimal::Decimal>().unwrap())),
        in_stock: Set(true),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        ..Default::default()
    };
    part1.insert(db).await.unwrap();

    // Create maintenance record for vehicle1
    let maintenance1 = maintenance_record::ActiveModel {
        id: Set(Uuid::new_v4()),
        vehicle_id: Set(vehicle1_id),
        service_type: Set("Oil Change".to_owned()),
        description: Set("Regular oil change with synthetic oil".to_owned()),
        cost: Set(Some("49.99".parse::<rust_decimal::Decimal>().unwrap())),
        service_date: Set(Utc::now()),
        mechanic_name: Set(Some("Mike Johnson".to_owned())),
        completed: Set(true),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        ..Default::default()
    };
    maintenance1.insert(db).await.unwrap();
}

// ============================================================================
// MAIN APPLICATION - Using generated CRUD router
// ============================================================================

#[tokio::main]
async fn main() {
    let db = setup_database().await;
    
    // Get the customer ID for testing
    use sea_orm::{Condition, Order};
    let condition = Condition::all();
    let customers = Customer::get_all(&db, &condition, CustomerColumn::Name, Order::Asc, 0, 100)
        .await
        .unwrap();
    
    if let Some(customer) = customers.first() {
        println!("Server running on http://localhost:3000");
        println!();
        println!("🧪 Test endpoints:");
        println!("  curl http://localhost:3000/customers");
        println!("  curl http://localhost:3000/customers/{}", customer.id);
        println!();
    } else {
        println!("⚠️ No customers found in database");
    }

    let app = Router::new()
        .merge(Customer::router(&db))
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
