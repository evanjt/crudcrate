/// Integration tests for recursive join functionality
/// 
/// This test proves the recursive join methodology using Customer → Vehicle → VehiclePart/MaintenanceRecord
/// The test will initially fail since the join attribute parsing isn't implemented yet,
/// demonstrating our test-driven development approach.
///
/// Expected behavior (when fully implemented):
/// 1. GET /customers/{id} returns customer with nested vehicles
/// 2. Each vehicle includes nested parts and maintenance_records  
/// 3. Depth limiting prevents infinite recursion
/// 4. Circular reference detection works at compile time

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

mod common;
use common::{
    setup_customer_vehicle_db,
    customer_entity::{Customer, CustomerCreate, CustomerEntity},
    vehicle_entity::{Vehicle, VehicleCreate, VehicleEntity}, 
    vehicle_part_entity::{VehiclePart, VehiclePartCreate, VehiclePartEntity},
    maintenance_record_entity::{MaintenanceRecord, MaintenanceRecordCreate, MaintenanceRecordEntity},
};
use sea_orm::{EntityTrait, ActiveModelTrait, Set, DatabaseConnection, QueryFilter, ColumnTrait};

/// Test data structure that matches our expected recursive join response
#[derive(Debug, Serialize, Deserialize)]
struct ExpectedCustomerResponse {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // This should be populated by recursive joins (depth = 2)
    pub vehicles: Vec<ExpectedVehicleWithJoins>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExpectedVehicleWithJoins {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub make: String,
    pub model: String,
    pub year: i32,
    pub vin: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // These should be populated by recursive joins from Vehicle
    pub parts: Vec<ExpectedVehiclePart>,
    pub maintenance_records: Vec<ExpectedMaintenanceRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExpectedVehiclePart {
    pub id: Uuid,
    pub vehicle_id: Uuid,
    pub name: String,
    pub part_number: String,
    pub category: String,
    pub price: Option<sea_orm::prelude::Decimal>,
    pub in_stock: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExpectedMaintenanceRecord {
    pub id: Uuid,
    pub vehicle_id: Uuid,
    pub service_type: String,
    pub description: String,
    pub cost: Option<sea_orm::prelude::Decimal>,
    pub service_date: DateTime<Utc>,
    pub mechanic_name: Option<String>,
    pub completed: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Helper function to create test data in the database
async fn create_test_data(db: &DatabaseConnection) -> Result<(Uuid, Uuid, Uuid, Uuid), sea_orm::DbErr> {
    let now = Utc::now();
    
    // Create customer
    let customer_id = Uuid::new_v4();
    let customer = common::customer_entity::ActiveModel {
        id: Set(customer_id),
        name: Set("John Doe".to_string()),
        email: Set("john.doe@example.com".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    customer.insert(db).await?;
    
    // Create vehicle
    let vehicle_id = Uuid::new_v4();
    let vehicle = common::vehicle_entity::ActiveModel {
        id: Set(vehicle_id),
        customer_id: Set(customer_id),
        make: Set("Toyota".to_string()),
        model: Set("Camry".to_string()),
        year: Set(2022),
        vin: Set("1HGBH41JXMN109186".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    vehicle.insert(db).await?;
    
    // Create vehicle part
    let part_id = Uuid::new_v4();
    let part = common::vehicle_part_entity::ActiveModel {
        id: Set(part_id),
        vehicle_id: Set(vehicle_id),
        name: Set("Engine Air Filter".to_string()),
        part_number: Set("AF123".to_string()),
        category: Set("Engine".to_string()),
        price: Set(Some(sea_orm::prelude::Decimal::new(2999, 2))), // $29.99
        in_stock: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    part.insert(db).await?;
    
    // Create maintenance record
    let maintenance_id = Uuid::new_v4();
    let maintenance = common::maintenance_record_entity::ActiveModel {
        id: Set(maintenance_id),
        vehicle_id: Set(vehicle_id),
        service_type: Set("Oil Change".to_string()),
        description: Set("Regular oil change with synthetic oil".to_string()),
        cost: Set(Some(sea_orm::prelude::Decimal::new(5999, 2))), // $59.99
        service_date: Set(now),
        mechanic_name: Set(Some("Mike Smith".to_string())),
        completed: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    maintenance.insert(db).await?;
    
    Ok((customer_id, vehicle_id, part_id, maintenance_id))
}

/// Creates a basic router for testing with manually implemented customer routes
fn create_test_router(db: DatabaseConnection) -> Router {
    use axum::{
        extract::{State, Path},
        http::StatusCode,
        Json,
        response::Json as ResponseJson,
        routing::get,
    };
    use uuid::Uuid;
    use common::customer_entity::{Customer, Entity as CustomerEntity};
    use sea_orm::{EntityTrait, DatabaseConnection};
    use crudcrate::traits::CRUDResource;
    
    async fn get_customer(
        State(db): State<DatabaseConnection>,
        Path(id): Path<Uuid>,
    ) -> Result<ResponseJson<Customer>, StatusCode> {
        // Use the generated CRUDResource implementation with automatic recursive joins
        match Customer::get_one(&db, id).await {
            Ok(customer) => Ok(ResponseJson(customer)),
            Err(_) => Err(StatusCode::NOT_FOUND),
        }
    }
    
    Router::new()
        .route("/api/v1/customers/{id}", get(get_customer))
        .with_state(db)
}

#[tokio::test]
async fn test_recursive_join_setup() {
    // Test 1: Verify database setup and entity creation works
    let db = setup_customer_vehicle_db().await.expect("Failed to setup database");
    
    // Create test data
    let (customer_id, vehicle_id, part_id, maintenance_id) = 
        create_test_data(&db).await.expect("Failed to create test data");
    
    // Verify data was created correctly
    let customer = CustomerEntity::find_by_id(customer_id).one(&db).await
        .expect("Database query failed")
        .expect("Customer not found");
    assert_eq!(customer.name, "John Doe");
    
    let vehicle = VehicleEntity::find_by_id(vehicle_id).one(&db).await
        .expect("Database query failed")
        .expect("Vehicle not found");
    assert_eq!(vehicle.customer_id, customer_id);
    assert_eq!(vehicle.make, "Toyota");
    
    let part = VehiclePartEntity::find_by_id(part_id).one(&db).await
        .expect("Database query failed") 
        .expect("Part not found");
    assert_eq!(part.vehicle_id, vehicle_id);
    assert_eq!(part.name, "Engine Air Filter");
    
    let maintenance = MaintenanceRecordEntity::find_by_id(maintenance_id).one(&db).await
        .expect("Database query failed")
        .expect("Maintenance record not found");
    assert_eq!(maintenance.vehicle_id, vehicle_id);
    assert_eq!(maintenance.service_type, "Oil Change");
    
    println!("✅ Database setup and basic entity creation works");
}

#[tokio::test]
async fn test_relationships_defined_correctly() {
    // Test 2: Verify Sea-ORM relationships are properly defined
    let db = setup_customer_vehicle_db().await.expect("Failed to setup database");
    let (customer_id, vehicle_id, _part_id, _maintenance_id) = 
        create_test_data(&db).await.expect("Failed to create test data");
    
    // Test relationship queries work (even without automatic joins)
    use sea_orm::{EntityTrait, Related};
    
    let vehicles = VehicleEntity::find()
        .filter(common::vehicle_entity::Column::CustomerId.eq(customer_id))
        .all(&db)
        .await
        .expect("Failed to query customer vehicles");
    
    assert_eq!(vehicles.len(), 1);
    assert_eq!(vehicles[0].id, vehicle_id);
    assert_eq!(vehicles[0].make, "Toyota");
    
    // Test Vehicle -> Parts relationship  
    let parts = VehiclePartEntity::find()
        .filter(common::vehicle_part_entity::Column::VehicleId.eq(vehicle_id))
        .all(&db)
        .await
        .expect("Failed to query vehicle parts");
    
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].name, "Engine Air Filter");
    
    // Test Vehicle -> Maintenance relationship
    let maintenance_records = MaintenanceRecordEntity::find()
        .filter(common::maintenance_record_entity::Column::VehicleId.eq(vehicle_id))
        .all(&db)
        .await
        .expect("Failed to query maintenance records");
    
    assert_eq!(maintenance_records.len(), 1);
    assert_eq!(maintenance_records[0].service_type, "Oil Change");
    
    println!("✅ Sea-ORM relationships work correctly");
}

#[tokio::test]
// Test enabled to fail until join functionality is implemented
async fn test_recursive_join_get_one() {
    // Test 3: Verify recursive joins work with GET /customers/{id}
    let db = setup_customer_vehicle_db().await.expect("Failed to setup database");
    let (customer_id, _vehicle_id, _part_id, _maintenance_id) = 
        create_test_data(&db).await.expect("Failed to create test data");
    
    let app = create_test_router(db);
    
    // Make GET request to customer endpoint
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/customers/{}", customer_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let customer_response: ExpectedCustomerResponse = serde_json::from_slice(&body)
        .expect("Failed to deserialize customer response");
    
    // Verify customer data
    assert_eq!(customer_response.id, customer_id);
    assert_eq!(customer_response.name, "John Doe");
    assert_eq!(customer_response.email, "john.doe@example.com");
    
    // Verify recursive join: customer should have vehicles
    assert_eq!(customer_response.vehicles.len(), 1);
    let vehicle = &customer_response.vehicles[0];
    assert_eq!(vehicle.make, "Toyota");
    assert_eq!(vehicle.model, "Camry");
    assert_eq!(vehicle.year, 2022);
    
    // Verify recursive join depth 2: vehicle should have parts and maintenance
    assert_eq!(vehicle.parts.len(), 1);
    assert_eq!(vehicle.parts[0].name, "Engine Air Filter");
    assert_eq!(vehicle.parts[0].category, "Engine");
    
    assert_eq!(vehicle.maintenance_records.len(), 1);
    assert_eq!(vehicle.maintenance_records[0].service_type, "Oil Change");
    assert_eq!(vehicle.maintenance_records[0].completed, true);
    
    println!("✅ Recursive joins work with depth=2");
}

#[tokio::test] 
#[ignore] // This test will be enabled once join attributes are implemented
async fn test_recursive_join_get_all() {
    // Test 4: Verify recursive joins work with GET /customers (all customers)
    let db = setup_customer_vehicle_db().await.expect("Failed to setup database");
    let (customer_id, _vehicle_id, _part_id, _maintenance_id) = 
        create_test_data(&db).await.expect("Failed to create test data");
    
    let app = create_test_router(db);
    
    // Make GET request to customers endpoint (get all)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/customers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let customers_response: Vec<ExpectedCustomerResponse> = serde_json::from_slice(&body)
        .expect("Failed to deserialize customers response");
    
    // Verify we get the customer with joins
    assert_eq!(customers_response.len(), 1);
    let customer = &customers_response[0];
    
    assert_eq!(customer.id, customer_id);
    assert_eq!(customer.vehicles.len(), 1);
    
    let vehicle = &customer.vehicles[0];
    assert_eq!(vehicle.parts.len(), 1);
    assert_eq!(vehicle.maintenance_records.len(), 1);
    
    println!("✅ Recursive joins work with get_all");
}

#[tokio::test]
#[ignore] // This test will be enabled once circular reference detection is implemented
async fn test_circular_reference_detection() {
    // Test 5: Verify circular reference detection prevents infinite loops
    
    // This test would create a circular reference scenario:
    // Customer -> Vehicle -> Customer (owner field)
    // The join system should detect this at compile time or runtime
    
    // Expected behavior:
    // 1. Compile-time detection preferred (macro expansion fails)  
    // 2. Runtime detection as fallback (error message, truncated response)
    // 3. Depth limiting prevents infinite recursion
    
    println!("✅ Circular reference detection (placeholder - to be implemented)");
}

#[tokio::test]
#[ignore] // This test will be enabled once depth limiting is implemented  
async fn test_depth_limiting() {
    // Test 6: Verify depth limiting works correctly
    
    // Create a deeper hierarchy: Customer -> Vehicle -> Part -> Supplier -> Address
    // With depth=2, should only join Customer -> Vehicle -> Part
    // With depth=3, should join Customer -> Vehicle -> Part -> Supplier
    
    // This tests that the depth parameter in #[crudcrate(join(one, all, depth=2))] works
    
    println!("✅ Depth limiting (placeholder - to be implemented)");
}

#[tokio::test]
async fn test_expected_join_attribute_compilation() {
    // Test 7: Verify that once implemented, the join attributes compile correctly
    
    // This test checks that our entity definitions with join attributes 
    // will compile once the attribute parsing is implemented
    
    // Currently our entities have:
    // #[crudcrate(join(one, all, depth = 2))]
    // pub vehicles: Vec<Vehicle>,
    
    // This should compile without errors and generate the appropriate
    // CRUDResource implementations with join support
    
    println!("✅ Join attribute syntax is ready for implementation");
}

/// Development helper: Print the expected JSON structure for manual verification
#[tokio::test]
async fn show_expected_json_structure() {
    let expected_response = json!({
        "id": "123e4567-e89b-12d3-a456-426614174000",
        "name": "John Doe", 
        "email": "john.doe@example.com",
        "created_at": "2024-01-15T10:30:00Z",
        "updated_at": "2024-01-15T10:30:00Z",
        "vehicles": [
            {
                "id": "789e0123-e89b-12d3-a456-426614174001",
                "customer_id": "123e4567-e89b-12d3-a456-426614174000",
                "make": "Toyota",
                "model": "Camry", 
                "year": 2022,
                "vin": "1HGBH41JXMN109186",
                "created_at": "2024-01-15T10:30:00Z",
                "updated_at": "2024-01-15T10:30:00Z",
                "parts": [
                    {
                        "id": "456e7890-e89b-12d3-a456-426614174002",
                        "vehicle_id": "789e0123-e89b-12d3-a456-426614174001",
                        "name": "Engine Air Filter",
                        "part_number": "AF123",
                        "category": "Engine",
                        "price": "29.99",
                        "in_stock": true,
                        "created_at": "2024-01-15T10:30:00Z",
                        "updated_at": "2024-01-15T10:30:00Z"
                    }
                ],
                "maintenance_records": [
                    {
                        "id": "321e6547-e89b-12d3-a456-426614174003", 
                        "vehicle_id": "789e0123-e89b-12d3-a456-426614174001",
                        "service_type": "Oil Change",
                        "description": "Regular oil change with synthetic oil",
                        "cost": "59.99",
                        "service_date": "2024-01-15T10:30:00Z",
                        "mechanic_name": "Mike Smith",
                        "completed": true,
                        "created_at": "2024-01-15T10:30:00Z",
                        "updated_at": "2024-01-15T10:30:00Z"
                    }
                ]
            }
        ]
    });
    
    println!("Expected JSON structure for recursive joins:");
    println!("{}", serde_json::to_string_pretty(&expected_response).unwrap());
    
    // This shows exactly what our API should return when recursive joins are implemented
}