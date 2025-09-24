use axum_test::TestServer;
use sea_orm::{Database, EntityTrait, Set};
use shared_models::{
    customer::{self, Entity as CustomerEntity, Model as CustomerModel},
    vehicle::{self, Entity as VehicleEntity, Model as VehicleModel}, 
    vehicle_part::{self, Entity as VehiclePartEntity, Model as VehiclePartModel},
    maintenance_record::{self, Entity as MaintenanceRecordEntity, Model as MaintenanceRecordModel},
    Customer, Vehicle, VehiclePart, MaintenanceRecord
};
use crudcrate::traits::CRUDResource;
use uuid::Uuid;

/// Test that proves our join loading is NOT hardcoded to Customer->Vehicle
/// These tests WILL FAIL initially because our implementation is hardcoded
/// They should pass once we implement generic join loading

#[tokio::test]
async fn test_generic_join_vehicle_to_parts() {
    // Setup database
    let db = Database::connect("sqlite::memory:").await.unwrap();
    shared_models::create_tables(&db).await.unwrap();

    // Create test data: Customer -> Vehicle -> Parts
    let customer_id = Uuid::new_v4();
    let vehicle_id = Uuid::new_v4();
    let part1_id = Uuid::new_v4();
    let part2_id = Uuid::new_v4();

    // Insert customer
    let customer = customer::ActiveModel {
        id: Set(customer_id),
        name: Set("Test Customer".to_string()),
        email: Set("test@example.com".to_string()),
        ..Default::default()
    };
    CustomerEntity::insert(customer).exec(&db).await.unwrap();

    // Insert vehicle
    let vehicle = vehicle::ActiveModel {
        id: Set(vehicle_id),
        customer_id: Set(customer_id),
        make: Set("Toyota".to_string()),
        model: Set("Camry".to_string()),
        year: Set(2020),
        vin: Set("TEST123456789".to_string()),
        ..Default::default()
    };
    VehicleEntity::insert(vehicle).exec(&db).await.unwrap();

    // Insert vehicle parts
    let part1 = vehicle_part::ActiveModel {
        id: Set(part1_id),
        vehicle_id: Set(vehicle_id),
        part_name: Set("Engine".to_string()),
        part_number: Set("ENG001".to_string()),
        ..Default::default()
    };
    VehiclePartEntity::insert(part1).exec(&db).await.unwrap();

    let part2 = vehicle_part::ActiveModel {
        id: Set(part2_id),
        vehicle_id: Set(vehicle_id),
        part_name: Set("Brake Pad".to_string()),
        part_number: Set("BRK001".to_string()),
        ..Default::default()
    };
    VehiclePartEntity::insert(part2).exec(&db).await.unwrap();

    // Create test server
    let app = Vehicle::router(&db);
    let server = TestServer::new(app).unwrap();

    // Test 1: Vehicle list should load parts (join(all) on parts field)
    println!("🧪 Testing Vehicle GET /vehicles - should load parts via join(all)");
    let response = server.get("/vehicles").await;
    
    // This WILL FAIL because our implementation only works for "vehicles" field, not "parts"
    response.assert_status_ok();
    let vehicles: Vec<Vehicle> = response.json();
    
    assert_eq!(vehicles.len(), 1);
    let vehicle = &vehicles[0];
    assert_eq!(vehicle.id, vehicle_id);
    assert_eq!(vehicle.parts.len(), 2); // This will FAIL - parts will be empty due to hardcoding
    
    // Verify parts are loaded correctly
    assert!(vehicle.parts.iter().any(|p| p.part_name == "Engine"));
    assert!(vehicle.parts.iter().any(|p| p.part_name == "Brake Pad"));
}

#[tokio::test]  
async fn test_generic_join_vehicle_to_maintenance_records() {
    // Setup database
    let db = Database::connect("sqlite::memory:").await.unwrap();
    shared_models::create_tables(&db).await.unwrap();

    // Create test data
    let customer_id = Uuid::new_v4();
    let vehicle_id = Uuid::new_v4();
    let maintenance1_id = Uuid::new_v4();
    let maintenance2_id = Uuid::new_v4();

    // Insert customer
    let customer = customer::ActiveModel {
        id: Set(customer_id),
        name: Set("Test Customer".to_string()),
        email: Set("test@example.com".to_string()),
        ..Default::default()
    };
    CustomerEntity::insert(customer).exec(&db).await.unwrap();

    // Insert vehicle
    let vehicle = vehicle::ActiveModel {
        id: Set(vehicle_id),
        customer_id: Set(customer_id),
        make: Set("Honda".to_string()),
        model: Set("Civic".to_string()),
        year: Set(2019),
        vin: Set("TEST987654321".to_string()),
        ..Default::default()
    };
    VehicleEntity::insert(vehicle).exec(&db).await.unwrap();

    // Insert maintenance records
    let maintenance1 = maintenance_record::ActiveModel {
        id: Set(maintenance1_id),
        vehicle_id: Set(vehicle_id),
        maintenance_type: Set("Oil Change".to_string()),
        description: Set("Regular oil change".to_string()),
        ..Default::default()
    };
    MaintenanceRecordEntity::insert(maintenance1).exec(&db).await.unwrap();

    let maintenance2 = maintenance_record::ActiveModel {
        id: Set(maintenance2_id),
        vehicle_id: Set(vehicle_id),
        maintenance_type: Set("Brake Service".to_string()),
        description: Set("Brake pad replacement".to_string()),
        ..Default::default()
    };
    MaintenanceRecordEntity::insert(maintenance2).exec(&db).await.unwrap();

    // Create test server
    let app = Vehicle::router(&db);
    let server = TestServer::new(app).unwrap();

    // Test 2: Vehicle list should load maintenance_records (join(all) on maintenance_records field)  
    println!("🧪 Testing Vehicle GET /vehicles - should load maintenance_records via join(all)");
    let response = server.get("/vehicles").await;
    
    // This WILL FAIL because our implementation only works for "vehicles" field, not "maintenance_records"
    response.assert_status_ok();
    let vehicles: Vec<Vehicle> = response.json();
    
    assert_eq!(vehicles.len(), 1);
    let vehicle = &vehicles[0];
    assert_eq!(vehicle.id, vehicle_id);
    assert_eq!(vehicle.maintenance_records.len(), 2); // This will FAIL - maintenance_records will be empty
    
    // Verify maintenance records are loaded correctly
    assert!(vehicle.maintenance_records.iter().any(|m| m.maintenance_type == "Oil Change"));
    assert!(vehicle.maintenance_records.iter().any(|m| m.maintenance_type == "Brake Service"));
}

#[tokio::test]
async fn test_recursive_joins_depth_2() {
    // Setup database
    let db = Database::connect("sqlite::memory:").await.unwrap();
    shared_models::create_tables(&db).await.unwrap();

    // Create test data hierarchy: Customer -> Vehicle -> Parts + Maintenance
    let customer_id = Uuid::new_v4();
    let vehicle_id = Uuid::new_v4();
    let part_id = Uuid::new_v4();
    let maintenance_id = Uuid::new_v4();

    // Insert customer
    let customer = customer::ActiveModel {
        id: Set(customer_id),
        name: Set("Recursive Test Customer".to_string()),
        email: Set("recursive@example.com".to_string()),
        ..Default::default()
    };
    CustomerEntity::insert(customer).exec(&db).await.unwrap();

    // Insert vehicle  
    let vehicle = vehicle::ActiveModel {
        id: Set(vehicle_id),
        customer_id: Set(customer_id),
        make: Set("Ford".to_string()),
        model: Set("F150".to_string()),
        year: Set(2021),
        vin: Set("RECURSIVE123456".to_string()),
        ..Default::default()
    };
    VehicleEntity::insert(vehicle).exec(&db).await.unwrap();

    // Insert vehicle part
    let part = vehicle_part::ActiveModel {
        id: Set(part_id),
        vehicle_id: Set(vehicle_id),
        part_name: Set("Transmission".to_string()),
        part_number: Set("TRANS001".to_string()),
        ..Default::default()
    };
    VehiclePartEntity::insert(part).exec(&db).await.unwrap();

    // Insert maintenance record
    let maintenance = maintenance_record::ActiveModel {
        id: Set(maintenance_id),
        vehicle_id: Set(vehicle_id),
        maintenance_type: Set("Transmission Service".to_string()),
        description: Set("Transmission fluid change".to_string()),
        ..Default::default()
    };
    MaintenanceRecordEntity::insert(maintenance).exec(&db).await.unwrap();

    // Create test server for Customer
    let app = Customer::router(&db);
    let server = TestServer::new(app).unwrap();

    // Test 3: Customer list should recursively load Vehicle -> Parts + Maintenance (depth=2)
    println!("🧪 Testing Customer GET /customers - should recursively load Vehicle->Parts+Maintenance (depth=2)");
    let response = server.get("/customers").await;
    
    response.assert_status_ok();
    let customers: Vec<Customer> = response.json();
    
    assert_eq!(customers.len(), 1);
    let customer = &customers[0];
    assert_eq!(customer.id, customer_id);
    
    // Level 1: Customer should have vehicles loaded (this currently works)
    assert_eq!(customer.vehicles.len(), 1);
    let vehicle = &customer.vehicles[0];
    assert_eq!(vehicle.id, vehicle_id);
    
    // Level 2: Vehicle should have parts and maintenance_records loaded (this WILL FAIL)
    // Currently vehicle.parts and vehicle.maintenance_records will be empty because:
    // 1. Vehicle join loading is not implemented (only Customer->Vehicle works)
    // 2. No recursive depth handling
    assert_eq!(vehicle.parts.len(), 1, "Vehicle should have parts loaded via recursive join");
    assert_eq!(vehicle.maintenance_records.len(), 1, "Vehicle should have maintenance records loaded via recursive join");
    
    // Verify nested data
    let part = &vehicle.parts[0];
    assert_eq!(part.part_name, "Transmission");
    
    let maintenance = &vehicle.maintenance_records[0];
    assert_eq!(maintenance.maintenance_type, "Transmission Service");
}

#[tokio::test]
async fn test_multiple_entity_types_generic_joins() {
    // This test proves that ALL entity types should support joins, not just Customer
    let db = Database::connect("sqlite::memory:").await.unwrap();
    shared_models::create_tables(&db).await.unwrap();

    // Test data setup
    let customer_id = Uuid::new_v4();
    let vehicle_id = Uuid::new_v4();
    let part_id = Uuid::new_v4();

    // Insert test data
    let customer = customer::ActiveModel {
        id: Set(customer_id),
        name: Set("Multi-Entity Test".to_string()),
        email: Set("multi@example.com".to_string()),
        ..Default::default()
    };
    CustomerEntity::insert(customer).exec(&db).await.unwrap();

    let vehicle = vehicle::ActiveModel {
        id: Set(vehicle_id),
        customer_id: Set(customer_id),
        make: Set("Multi".to_string()),
        model: Set("Test".to_string()),
        year: Set(2022),
        vin: Set("MULTI123456789".to_string()),
        ..Default::default()
    };
    VehicleEntity::insert(vehicle).exec(&db).await.unwrap();

    let part = vehicle_part::ActiveModel {
        id: Set(part_id),
        vehicle_id: Set(vehicle_id),
        part_name: Set("Test Part".to_string()),
        part_number: Set("TEST001".to_string()),
        ..Default::default()
    };
    VehiclePartEntity::insert(part).exec(&db).await.unwrap();

    // Test 4: Customer joins should work (currently works)
    println!("🧪 Testing Customer router - should load vehicles");
    let customer_app = Customer::router(&db);
    let customer_server = TestServer::new(customer_app).unwrap();
    let response = customer_server.get("/customers").await;
    response.assert_status_ok();
    let customers: Vec<Customer> = response.json();
    assert_eq!(customers[0].vehicles.len(), 1, "Customer should load vehicles");

    // Test 5: Vehicle joins should work (currently FAILS due to hardcoding)
    println!("🧪 Testing Vehicle router - should load parts and maintenance_records");
    let vehicle_app = Vehicle::router(&db);
    let vehicle_server = TestServer::new(vehicle_app).unwrap();
    let response = vehicle_server.get("/vehicles").await;
    response.assert_status_ok();
    let vehicles: Vec<Vehicle> = response.json();
    
    // These will FAIL because Vehicle entity join loading is not implemented
    assert_eq!(vehicles[0].parts.len(), 1, "Vehicle should load parts via join(all)");
    assert_eq!(vehicles[0].maintenance_records.len(), 0, "Vehicle should load maintenance_records via join(all) (empty but present)");

    // Test 6: Other entity types should also support routers  
    println!("🧪 Testing VehiclePart and MaintenanceRecord routers exist");
    // These will FAIL if routers aren't generated for entities without join fields
    let _part_app = VehiclePart::router(&db); // Should not panic
    let _maintenance_app = MaintenanceRecord::router(&db); // Should not panic
}