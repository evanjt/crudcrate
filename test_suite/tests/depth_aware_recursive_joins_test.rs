/// Depth-aware recursive joins test
/// 
/// Tests the core depth functionality using existing shared_models:
/// 1. Customer (depth=2) should load Vehicle → Parts + Maintenance 
/// 2. Validates that depth parameter triggers recursive loading beyond join(all) scope

use crudcrate::traits::CRUDResource;
use sea_orm::{entity::prelude::*, Database, DatabaseConnection, Schema, Set};
use sea_orm_migration::sea_query::SqliteQueryBuilder;
use shared_models::*;
use uuid::Uuid;

async fn setup_test_database() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to test database");

    let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);

    // Create tables using shared_models
    let statements = [
        schema.create_table_from_entity(CustomerEntity),
        schema.create_table_from_entity(VehicleEntity),
        schema.create_table_from_entity(VehiclePartEntity),
        schema.create_table_from_entity(MaintenanceRecordEntity),
    ];

    for statement in statements {
        db.execute_unprepared(&statement.to_string(SqliteQueryBuilder))
            .await
            .expect("Failed to create table");
    }

    db
}

async fn seed_comprehensive_test_data(db: &DatabaseConnection) -> (Uuid, Uuid, Uuid, Uuid) {
    let customer_id = Uuid::new_v4();
    let vehicle_id = Uuid::new_v4();
    let part_id = Uuid::new_v4();
    let maintenance_id = Uuid::new_v4();

    // Create customer
    let customer = CustomerActiveModel {
        id: Set(customer_id),
        name: Set("Depth Test Customer".to_string()),
        email: Set("depth@example.com".to_string()),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
    };
    customer.insert(db).await.expect("Failed to insert customer");

    // Create vehicle
    let vehicle = VehicleActiveModel {
        id: Set(vehicle_id),
        customer_id: Set(customer_id),
        make: Set("Toyota".to_string()),
        model: Set("Camry".to_string()),
        year: Set(2020),
        vin: Set("TEST123456789".to_string()),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
    };
    vehicle.insert(db).await.expect("Failed to insert vehicle");

    // Create vehicle part
    let part = VehiclePartActiveModel {
        id: Set(part_id),
        vehicle_id: Set(vehicle_id),
        name: Set("Engine Block".to_string()),
        part_number: Set("ENG-001".to_string()),
        category: Set("Engine Components".to_string()),
        price: Set(Some(rust_decimal::Decimal::new(1500, 0))),
        in_stock: Set(true),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
    };
    part.insert(db).await.expect("Failed to insert part");

    // Create maintenance record
    let maintenance = MaintenanceRecordActiveModel {
        id: Set(maintenance_id),
        vehicle_id: Set(vehicle_id),
        service_type: Set("Maintenance".to_string()),
        description: Set("Oil Change Service".to_string()),
        cost: Set(Some(rust_decimal::Decimal::new(75, 0))),
        service_date: Set(chrono::Utc::now()),
        mechanic_name: Set(Some("John Mechanic".to_string())),
        completed: Set(true),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
    };
    maintenance.insert(db).await.expect("Failed to insert maintenance record");

    (customer_id, vehicle_id, part_id, maintenance_id)
}

#[tokio::test]
async fn test_baseline_customer_loads_vehicles() {
    let db = setup_test_database().await;
    let (customer_id, vehicle_id, _part_id, _maintenance_id) = seed_comprehensive_test_data(&db).await;

    // This should work - Customer has join(one, all, depth=2) for vehicles
    let customer = Customer::get_one(&db, customer_id).await
        .expect("Failed to load customer");

    // Level 1: Customer should load vehicles  
    assert_eq!(customer.vehicles.len(), 1);
    assert_eq!(customer.vehicles[0].id, vehicle_id);

    // SUCCESS: With depth-aware recursive loading implemented, Vehicle parts/maintenance should now be loaded!
    assert_eq!(customer.vehicles[0].parts.len(), 1, 
        "Parts should be loaded due to Customer's depth=2 recursive loading");
    assert_eq!(customer.vehicles[0].maintenance_records.len(), 1,
        "Maintenance records should be loaded due to Customer's depth=2 recursive loading");
}

#[tokio::test]
async fn test_direct_vehicle_query_loads_parts_and_maintenance() {
    let db = setup_test_database().await;
    let (_customer_id, vehicle_id, part_id, maintenance_id) = seed_comprehensive_test_data(&db).await;

    // Direct vehicle query should load parts and maintenance (both have join(all))
    let vehicle = Vehicle::get_one(&db, vehicle_id).await
        .expect("Failed to load vehicle directly");

    // Direct vehicle query should load parts and maintenance via join(all)
    assert_eq!(vehicle.parts.len(), 1, "Direct vehicle query should load parts via join(all)");
    assert_eq!(vehicle.parts[0].id, part_id);

    assert_eq!(vehicle.maintenance_records.len(), 1, "Direct vehicle query should load maintenance via join(all)");
    assert_eq!(vehicle.maintenance_records[0].id, maintenance_id);
}

#[tokio::test]
async fn test_depth_2_recursive_loading_target_behavior() {
    let db = setup_test_database().await;
    let (customer_id, vehicle_id, part_id, maintenance_id) = seed_comprehensive_test_data(&db).await;

    // THIS TEST WILL FAIL INITIALLY - this is our target behavior
    // Customer has depth=2, so it should:
    // Level 1: Load Customer → Vehicle  
    // Level 2: Load Vehicle → Parts + Maintenance (even though they only have join(all))
    let customer = Customer::get_one(&db, customer_id).await
        .expect("Failed to load customer");

    // Level 1: Customer → Vehicle (should work)
    assert_eq!(customer.vehicles.len(), 1);
    let vehicle = &customer.vehicles[0];
    assert_eq!(vehicle.id, vehicle_id);

    // Level 2: Vehicle → Parts + Maintenance (SHOULD be loaded due to Customer's depth=2)
    // This will fail until we implement depth-aware recursive loading
    assert_eq!(vehicle.parts.len(), 1,
        "Customer depth=2 should trigger Vehicle parts loading even though Vehicle only has join(all)");
    assert_eq!(vehicle.parts[0].id, part_id);
    assert_eq!(vehicle.parts[0].name, "Engine Block");

    assert_eq!(vehicle.maintenance_records.len(), 1,
        "Customer depth=2 should trigger Vehicle maintenance loading even though Vehicle only has join(all)");
    assert_eq!(vehicle.maintenance_records[0].id, maintenance_id);
    assert_eq!(vehicle.maintenance_records[0].description, "Oil Change Service");
}

#[tokio::test]
async fn test_get_all_respects_depth_parameter() {
    let db = setup_test_database().await;
    let (customer_id, vehicle_id, part_id, maintenance_id) = seed_comprehensive_test_data(&db).await;

    // get_all should also respect depth parameter
    let customers = Customer::get_all(
        &db,
        &sea_orm::Condition::all(),
        shared_models::CustomerColumn::Name,
        sea_orm::Order::Asc,
        0,
        10,
    ).await.expect("Failed to load customers");

    assert_eq!(customers.len(), 1);
    let customer = &customers[0];

    // Same depth=2 behavior should work in get_all
    assert_eq!(customer.vehicles.len(), 1);
    let vehicle = &customer.vehicles[0];

    // This will fail until depth-aware loading is implemented
    assert_eq!(vehicle.parts.len(), 1,
        "get_all should respect depth=2 for parts loading");
    assert_eq!(vehicle.maintenance_records.len(), 1,
        "get_all should respect depth=2 for maintenance loading");
}

/// This test verifies that without depth specification, normal join(all) behavior works
#[tokio::test]
async fn test_no_depth_means_no_recursive_triggering() {
    let db = setup_test_database().await;
    let (_customer_id, vehicle_id, _part_id, _maintenance_id) = seed_comprehensive_test_data(&db).await;

    // Vehicle has join(all) for parts/maintenance but no depth specification
    // So when Vehicle is loaded directly, it should load its joins
    // But if Vehicle were loaded as a nested entity from something else without depth, it shouldn't
    let vehicle = Vehicle::get_one(&db, vehicle_id).await
        .expect("Failed to load vehicle");

    // Direct query should load immediate joins
    assert_eq!(vehicle.parts.len(), 1, "Direct vehicle query loads parts");
    assert_eq!(vehicle.maintenance_records.len(), 1, "Direct vehicle query loads maintenance");
    
    // This establishes the baseline: joins work when queried directly
}