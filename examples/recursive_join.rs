/// Join Loading Example
///
/// Demonstrates automatic relationship loading with `#[crudcrate(join(one, all))]`
/// Customer → Vehicle relationships are loaded automatically in API responses.
///
/// Run with: `cargo run --example recursive_join`
use axum::Router;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, Set, entity::prelude::*,
};
use std::time::Duration;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

// Import shared models with join configuration
use crudcrate::traits::CRUDResource;
use shared_models::{
    Customer, CustomerEntity, CustomerColumn,
    Vehicle, VehicleEntity,
    VehiclePartEntity, MaintenanceRecordEntity,
    customer, vehicle, vehicle_part, maintenance_record
};

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
        Err(e) => println!("❌ Failed to create customers table: {e:?}"),
    }

    println!("Creating vehicles table...");
    let stmt = schema.create_table_from_entity(VehicleEntity);
    match db.execute(db.get_database_backend().build(&stmt)).await {
        Ok(_) => println!("✅ vehicles table created"),
        Err(e) => println!("❌ Failed to create vehicles table: {e:?}"),
    }

    println!("Creating vehicle_parts table...");
    let stmt = schema.create_table_from_entity(VehiclePartEntity);
    match db.execute(db.get_database_backend().build(&stmt)).await {
        Ok(_) => println!("✅ vehicle_parts table created"),
        Err(e) => println!("❌ Failed to create vehicle_parts table: {e:?}"),
    }

    println!("Creating maintenance_records table...");
    let stmt = schema.create_table_from_entity(MaintenanceRecordEntity);
    match db.execute(db.get_database_backend().build(&stmt)).await {
        Ok(_) => println!("✅ maintenance_records table created"),
        Err(e) => println!("❌ Failed to create maintenance_records table: {e:?}"),
    }
}

#[allow(clippy::too_many_lines)]
async fn seed_data(db: &DatabaseConnection) {
    println!("🌱 Seeding comprehensive test data...");

    // Customer data
    let customers = [
        ("Alice Johnson", "alice@garage.com"),
        ("Bob Wilson", "bob@autocare.com"),
    ];

    // Vehicle configurations: (make, model, year, vin_suffix)
    let vehicles_per_customer = [
        vec![
            ("Toyota", "Camry", 2020, "001"),
            ("Honda", "Civic", 2019, "002"),
            ("Ford", "F-150", 2021, "003"),
        ],
        vec![
            ("BMW", "X5", 2022, "101"),
            ("Tesla", "Model 3", 2023, "102"),
        ],
    ];

    // Parts data: (name, part_number, category, price)
    let parts_configs = [
        ("Front Brake Pads", "BP-001", "Brakes", "89.99"),
        ("Rear Brake Pads", "BP-002", "Brakes", "79.99"),
        ("Oil Filter", "OF-001", "Engine", "24.99"),
        ("Air Filter", "AF-001", "Engine", "19.99"),
        ("Spark Plugs", "SP-001", "Engine", "45.99"),
        ("Transmission Fluid", "TF-001", "Transmission", "32.99"),
    ];

    // Maintenance configurations: (service_type, description, cost, mechanic)
    let maintenance_configs = [
        (
            "Oil Change",
            "Regular oil change with synthetic oil",
            "49.99",
            "Mike Johnson",
        ),
        (
            "Brake Service",
            "Complete brake pad replacement",
            "299.99",
            "Sarah Davis",
        ),
        (
            "Transmission Service",
            "Transmission fluid change",
            "149.99",
            "Tony Garcia",
        ),
        (
            "Engine Tune-up",
            "Complete engine maintenance",
            "399.99",
            "Lisa Chen",
        ),
        (
            "Tire Rotation",
            "Tire rotation and balance",
            "79.99",
            "Mike Johnson",
        ),
    ];

    for (customer_idx, (name, email)) in customers.iter().enumerate() {
        let customer_id = Uuid::new_v4();

        // Create customer
        customer::ActiveModel {
            id: Set(customer_id),
            name: Set((*name).to_owned()),
            email: Set((*email).to_owned()),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now())
        }
        .insert(db)
        .await
        .unwrap();

        // Create vehicles for this customer
        for (vehicle_idx, (make, model, year, vin_suffix)) in
            vehicles_per_customer[customer_idx].iter().enumerate()
        {
            let vehicle_id = Uuid::new_v4();

            vehicle::ActiveModel {
                id: Set(vehicle_id),
                customer_id: Set(customer_id),
                make: Set((*make).to_owned()),
                model: Set((*model).to_owned()),
                year: Set(*year),
                vin: Set(format!("1HGBH41JXMN10918{vin_suffix}")),
                created_at: Set(Utc::now()),
                updated_at: Set(Utc::now())
            }
            .insert(db)
            .await
            .unwrap();

            // Add 2-4 parts per vehicle
            let parts_count = 2 + (vehicle_idx % 3); // 2-4 parts
            for part_idx in 0..parts_count {
                let (name, part_number, category, price) =
                    parts_configs[part_idx % parts_configs.len()];

                vehicle_part::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    vehicle_id: Set(vehicle_id),
                    name: Set((*name).to_owned()),
                    part_number: Set(format!("{part_number}-{vehicle_idx}")),
                    category: Set((*category).to_owned()),
                    price: Set(Some(price.parse::<rust_decimal::Decimal>().unwrap())),
                    in_stock: Set(part_idx % 2 == 0), // Alternate stock status
                    created_at: Set(Utc::now()),
                    updated_at: Set(Utc::now())
                }
                .insert(db)
                .await
                .unwrap();
            }

            // Add 2-3 maintenance records per vehicle
            let maintenance_count = 2 + (vehicle_idx % 2); // 2-3 records
            for maintenance_idx in 0..maintenance_count {
                let (service_type, description, cost, mechanic) =
                    maintenance_configs[maintenance_idx % maintenance_configs.len()];

                maintenance_record::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    vehicle_id: Set(vehicle_id),
                    service_type: Set((*service_type).to_owned()),
                    description: Set((*description).to_owned()),
                    cost: Set(Some(cost.parse::<rust_decimal::Decimal>().unwrap())),
                    service_date: Set(Utc::now()),
                    mechanic_name: Set(Some((*mechanic).to_owned())),
                    completed: Set(maintenance_idx % 3 != 0), // Most completed, some pending
                    created_at: Set(Utc::now()),
                    updated_at: Set(Utc::now())
                }
                .insert(db)
                .await
                .unwrap();
            }
        }
    }

    println!(
        "✅ Seeded {} customers with vehicles, parts, and maintenance records",
        customers.len()
    );
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
        println!("🚀 Server running on http://localhost:3000");
        println!();
        println!("📊 Dataset Overview:");
        println!("  • {} customers", customers.len());
        println!("  • Alice: 3 vehicles (Toyota Camry, Honda Civic, Ford F-150)");
        println!("  • Bob: 2 vehicles (BMW X5, Tesla Model 3)");
        println!("  • Each vehicle: 2-4 parts + 2-3 maintenance records");
        println!("  • Total: 5 vehicles, ~15 parts, ~12 maintenance records");
        println!();
        println!("🧪 Test multi-level recursive joins:");
        println!(
            "  curl -s http://localhost:3000/customers | jq .    # All customers → vehicles → parts/maintenance"
        );
        println!(
            "  curl -s http://localhost:3000/customers/{} | jq . # Single customer (3-level deep)",
            customer.id
        );
        println!(
            "  curl -s http://localhost:3000/vehicles | jq .     # All vehicles → parts/maintenance"
        );
        println!();
    } else {
        println!("⚠️ No customers found in database");
    }

    let app = Router::new()
        .nest("/customers", Customer::router(&db).into())
        .nest("/vehicles", Vehicle::router(&db).into())
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
