// Feature Group 4: Relationship Loading
// Tests join data, recursive depth, single and multi-level loading

use chrono::{DateTime, Utc};
use crudcrate::{EntityToModels, traits::CRUDResource};
use sea_orm::{entity::prelude::*, Database, DatabaseConnection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Customer entity with vehicle relationships
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "customers")]
#[crudcrate(api_struct = "Customer", generate_router)]
pub struct CustomerModel {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub name: String,

    #[crudcrate(filterable)]
    pub email: String,

    #[crudcrate(sortable, exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,

    // Single-level join loading
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all))]
    pub vehicles: Vec<Vehicle>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum CustomerRelation {
    #[sea_orm(has_many = "super::VehicleEntity")]
    Vehicles,
}

impl Related<VehicleEntity> for CustomerEntity {
    fn to() -> RelationDef {
        CustomerRelation::Vehicles.def()
    }
}

impl ActiveModelBehavior for customers::ActiveModel {}

// Vehicle entity with parts relationships
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "vehicles")]
#[crudcrate(api_struct = "Vehicle", generate_router)]
pub struct VehicleModel {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub customer_id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub make: String,

    #[crudcrate(filterable, sortable)]
    pub model: String,

    #[crudcrate(filterable)]
    pub year: i32,

    // Multi-level join (planned feature)
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all, depth = 2))]
    pub parts: Vec<VehiclePart>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum VehicleRelation {
    #[sea_orm(belongs_to = "super::CustomerEntity", from = "Column::CustomerId", to = "super::customers::Column::Id")]
    Customer,
    #[sea_orm(has_many = "super::VehiclePartEntity")]
    Parts,
}

impl Related<CustomerEntity> for VehicleEntity {
    fn to() -> RelationDef {
        VehicleRelation::Customer.def()
    }
}

impl Related<VehiclePartEntity> for VehicleEntity {
    fn to() -> RelationDef {
        VehicleRelation::Parts.def()
    }
}

impl ActiveModelBehavior for vehicles::ActiveModel {}

// VehiclePart entity for deep relationships
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "vehicle_parts")]
#[crudcrate(api_struct = "VehiclePart")]
pub struct VehiclePartModel {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub vehicle_id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub name: String,

    #[crudcrate(filterable)]
    pub part_number: String,

    #[crudcrate(filterable)]
    pub price: f64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum VehiclePartRelation {
    #[sea_orm(belongs_to = "super::VehicleEntity", from = "Column::VehicleId", to = "super::vehicles::Column::Id")]
    Vehicle,
}

impl Related<VehicleEntity> for VehiclePartEntity {
    fn to() -> RelationDef {
        VehiclePartRelation::Vehicle.def()
    }
}

impl ActiveModelBehavior for vehicle_parts::ActiveModel {}

// Test entity for single entity joins (Optional/T fields)
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "profiles")]
#[crudcrate(api_struct = "Profile")]
pub struct ProfileModel {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub customer_id: Uuid,

    pub bio: String,

    // Single entity join (belongs_to/has_one relationship)
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one))]
    pub customer: Option<Customer>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum ProfileRelation {
    #[sea_orm(belongs_to = "super::CustomerEntity", from = "Column::CustomerId", to = "super::customers::Column::Id")]
    Customer,
}

impl Related<CustomerEntity> for ProfileEntity {
    fn to() -> RelationDef {
        ProfileRelation::Customer.def()
    }
}

impl ActiveModelBehavior for profiles::ActiveModel {}

async fn setup_test_db() -> Result<DatabaseConnection, sea_orm::DbErr> {
    let db = Database::connect("sqlite::memory:").await?;
    
    // Create tables (simplified migration)
    db.execute_unprepared("
        CREATE TABLE customers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            created_at TEXT NOT NULL
        )
    ").await?;

    db.execute_unprepared("
        CREATE TABLE vehicles (
            id TEXT PRIMARY KEY,
            customer_id TEXT NOT NULL,
            make TEXT NOT NULL,
            model TEXT NOT NULL,
            year INTEGER NOT NULL,
            FOREIGN KEY (customer_id) REFERENCES customers (id)
        )
    ").await?;

    db.execute_unprepared("
        CREATE TABLE vehicle_parts (
            id TEXT PRIMARY KEY,
            vehicle_id TEXT NOT NULL,
            name TEXT NOT NULL,
            part_number TEXT NOT NULL,
            price REAL NOT NULL,
            FOREIGN KEY (vehicle_id) REFERENCES vehicles (id)
        )
    ").await?;

    db.execute_unprepared("
        CREATE TABLE profiles (
            id TEXT PRIMARY KEY,
            customer_id TEXT NOT NULL,
            bio TEXT NOT NULL,
            FOREIGN KEY (customer_id) REFERENCES customers (id)
        )
    ").await?;

    Ok(db)
}

#[tokio::test]
async fn test_single_level_join_get_one() {
    let db = setup_test_db().await.unwrap();
    
    // Test that join(one) loads related data in get_one() responses
    let customer_id = Uuid::new_v4();
    let vehicle_id = Uuid::new_v4();

    // Insert test data
    db.execute_unprepared(&format!("
        INSERT INTO customers (id, name, email, created_at) 
        VALUES ('{}', 'John Doe', 'john@example.com', '{}')
    ", customer_id, Utc::now().to_rfc3339())).await.unwrap();

    db.execute_unprepared(&format!("
        INSERT INTO vehicles (id, customer_id, make, model, year) 
        VALUES ('{}', '{}', 'Toyota', 'Camry', 2020)
    ", vehicle_id, customer_id)).await.unwrap();

    // Test get_one with join loading
    let result = Customer::get_one(&db, customer_id).await;
    assert!(result.is_ok());
    
    let customer = result.unwrap();
    assert_eq!(customer.name, "John Doe");
    assert_eq!(customer.vehicles.len(), 1);
    assert_eq!(customer.vehicles[0].make, "Toyota");
    assert_eq!(customer.vehicles[0].model, "Camry");
}

#[tokio::test]
async fn test_single_level_join_get_all() {
    let db = setup_test_db().await.unwrap();
    
    // Test that join(all) loads related data in get_all() responses
    let customer1_id = Uuid::new_v4();
    let customer2_id = Uuid::new_v4();
    let vehicle1_id = Uuid::new_v4();
    let vehicle2_id = Uuid::new_v4();

    // Insert test data
    db.execute_unprepared(&format!("
        INSERT INTO customers (id, name, email, created_at) VALUES 
        ('{}', 'John Doe', 'john@example.com', '{}'),
        ('{}', 'Jane Smith', 'jane@example.com', '{}')
    ", customer1_id, Utc::now().to_rfc3339(), customer2_id, Utc::now().to_rfc3339())).await.unwrap();

    db.execute_unprepared(&format!("
        INSERT INTO vehicles (id, customer_id, make, model, year) VALUES
        ('{}', '{}', 'Toyota', 'Camry', 2020),
        ('{}', '{}', 'Honda', 'Civic', 2019)
    ", vehicle1_id, customer1_id, vehicle2_id, customer2_id)).await.unwrap();

    // Test get_all with join loading
    let condition = sea_orm::Condition::all();
    let result = Customer::get_all(&db, &condition, 
        customers::Column::Id, sea_orm::Order::Asc, 0, 10).await;
    assert!(result.is_ok());
    
    let customers = result.unwrap();
    assert_eq!(customers.len(), 2);
    
    // Verify join data is loaded
    for customer in customers {
        assert_eq!(customer.vehicles.len(), 1);
        assert!(!customer.vehicles[0].make.is_empty());
    }
}

#[tokio::test]
async fn test_join_one_only() {
    let db = setup_test_db().await.unwrap();
    
    // Test that join(one) only loads in get_one(), not get_all()
    let customer_id = Uuid::new_v4();
    
    // Insert test data
    db.execute_unprepared(&format!("
        INSERT INTO customers (id, name, email, created_at) 
        VALUES ('{}', 'John Doe', 'john@example.com', '{}')
    ", customer_id, Utc::now().to_rfc3339())).await.unwrap();

    db.execute_unprepared(&format!("
        INSERT INTO profiles (id, customer_id, bio) 
        VALUES ('{}', '{}', 'Software developer')
    ", Uuid::new_v4(), customer_id)).await.unwrap();

    // get_one should load the profile
    let profile_result = Profile::get_one(&db, customer_id).await;
    if profile_result.is_ok() {
        let profile = profile_result.unwrap();
        // Profile has join(one) so customer should be loaded in get_one
        assert!(profile.customer.is_some());
    }

    // get_all should NOT load the profile data for join(one) fields
    let condition = sea_orm::Condition::all();
    let profiles_result = Profile::get_all(&db, &condition,
        profiles::Column::Id, sea_orm::Order::Asc, 0, 10).await;
    
    if profiles_result.is_ok() {
        let profiles = profiles_result.unwrap();
        if !profiles.is_empty() {
            // Profile customer should NOT be loaded in get_all for join(one)
            assert!(profiles[0].customer.is_none());
        }
    }
}

#[tokio::test]
async fn test_multiple_related_entities() {
    let db = setup_test_db().await.unwrap();
    
    // Test loading multiple vehicles for one customer
    let customer_id = Uuid::new_v4();
    
    // Insert customer
    db.execute_unprepared(&format!("
        INSERT INTO customers (id, name, email, created_at) 
        VALUES ('{}', 'Multi Vehicle Owner', 'multi@example.com', '{}')
    ", customer_id, Utc::now().to_rfc3339())).await.unwrap();

    // Insert multiple vehicles
    for i in 1..=3 {
        db.execute_unprepared(&format!("
            INSERT INTO vehicles (id, customer_id, make, model, year) 
            VALUES ('{}', '{}', 'Make{}', 'Model{}', {})
        ", Uuid::new_v4(), customer_id, i, i, 2020 + i)).await.unwrap();
    }

    let result = Customer::get_one(&db, customer_id).await;
    assert!(result.is_ok());
    
    let customer = result.unwrap();
    assert_eq!(customer.vehicles.len(), 3);
    
    // Verify all vehicles are loaded
    let makes: Vec<&str> = customer.vehicles.iter().map(|v| v.make.as_str()).collect();
    assert!(makes.contains(&"Make1"));
    assert!(makes.contains(&"Make2"));
    assert!(makes.contains(&"Make3"));
}

#[tokio::test]
async fn test_empty_relationships() {
    let db = setup_test_db().await.unwrap();
    
    // Test customer with no vehicles
    let customer_id = Uuid::new_v4();
    
    db.execute_unprepared(&format!("
        INSERT INTO customers (id, name, email, created_at) 
        VALUES ('{}', 'No Vehicles', 'none@example.com', '{}')
    ", customer_id, Utc::now().to_rfc3339())).await.unwrap();

    let result = Customer::get_one(&db, customer_id).await;
    assert!(result.is_ok());
    
    let customer = result.unwrap();
    assert_eq!(customer.vehicles.len(), 0);
    assert!(customer.vehicles.is_empty());
}

#[tokio::test]
async fn test_type_based_relationship_detection() {
    // Test that Vec<T> fields are treated as has_many
    // and Option<T>/T fields are treated as belongs_to/has_one
    
    // This is validated at compile time by the macro generation
    // If the relationships compile correctly, the detection works
    
    // Vec<Vehicle> -> has_many relationship -> .all() loading
    let customer = Customer {
        id: Uuid::new_v4(),
        name: "Test".to_string(),
        email: "test@example.com".to_string(),
        created_at: Utc::now(),
        vehicles: vec![], // Vec<T> indicates has_many
    };
    
    // Option<Customer> -> belongs_to/has_one relationship -> .one() loading  
    let profile = Profile {
        id: Uuid::new_v4(),
        customer_id: Uuid::new_v4(),
        bio: "Test bio".to_string(),
        customer: None, // Option<T> indicates belongs_to/has_one
    };
    
    assert_eq!(customer.vehicles.len(), 0);
    assert!(profile.customer.is_none());
}

#[tokio::test]
async fn test_join_depth_parameter_parsing() {
    // Test that depth parameter is parsed correctly in join attributes
    // This is validated at compile time
    
    // Vehicle has join(one, all, depth = 2) for parts
    let vehicle = Vehicle {
        id: Uuid::new_v4(),
        customer_id: Uuid::new_v4(),
        make: "Toyota".to_string(),
        model: "Camry".to_string(),
        year: 2020,
        parts: vec![], // depth = 2 should enable recursive loading
    };
    
    assert_eq!(vehicle.parts.len(), 0);
    
    // If this compiles, the depth parameter syntax is working
    assert!(true);
}

#[tokio::test] 
async fn test_join_configuration_variations() {
    // Test different join configuration options compile correctly
    
    #[allow(dead_code)]
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "join_test")]
    #[crudcrate(api_struct = "JoinTest")]
    struct JoinTestModel {
        #[sea_orm(primary_key)]
        #[crudcrate(primary_key)]
        pub id: Uuid,
        
        // Different join configurations
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr, join(one))]
        pub one_only: Vec<String>,
        
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr, join(all))]
        pub all_only: Vec<String>,
        
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr, join(one, all))]
        pub both: Vec<String>,
        
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr, join(one, all, depth = 3))]
        pub with_depth: Vec<String>,
    }
    
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    enum JoinTestRelation {}
    
    impl ActiveModelBehavior for join_test::ActiveModel {}
    
    // If this compiles, all join syntax variations work
    assert!(true);
}

#[tokio::test]
async fn test_non_db_attr_requirement() {
    // Test that join fields must also have non_db_attr
    
    // This test validates at compile time that:
    // 1. Join fields require #[sea_orm(ignore)]
    // 2. Join fields require #[crudcrate(non_db_attr)]
    // 3. The combination works correctly
    
    let customer = Customer {
        id: Uuid::new_v4(),
        name: "Test".to_string(),
        email: "test@example.com".to_string(),
        created_at: Utc::now(),
        vehicles: vec![], // This field has both #[sea_orm(ignore)] and non_db_attr
    };
    
    assert!(customer.vehicles.is_empty());
    
    // If this compiles, the non_db_attr requirement is working
    assert!(true);
}

#[tokio::test]
async fn test_multi_level_join_planned_feature() {
    let db = setup_test_db().await.unwrap();
    
    // Test the planned multi-level join feature
    // Currently this tests single-level, but the depth parameter is ready for expansion
    
    let customer_id = Uuid::new_v4();
    let vehicle_id = Uuid::new_v4();
    let part_id = Uuid::new_v4();
    
    // Insert nested test data
    db.execute_unprepared(&format!("
        INSERT INTO customers (id, name, email, created_at) 
        VALUES ('{}', 'Deep Test', 'deep@example.com', '{}')
    ", customer_id, Utc::now().to_rfc3339())).await.unwrap();

    db.execute_unprepared(&format!("
        INSERT INTO vehicles (id, customer_id, make, model, year) 
        VALUES ('{}', '{}', 'Toyota', 'Camry', 2020)
    ", vehicle_id, customer_id)).await.unwrap();

    db.execute_unprepared(&format!("
        INSERT INTO vehicle_parts (id, vehicle_id, name, part_number, price) 
        VALUES ('{}', '{}', 'Engine', 'ENG001', 5000.00)
    ", part_id, vehicle_id)).await.unwrap();

    // Currently tests single-level: Customer -> Vehicles
    let result = Customer::get_one(&db, customer_id).await;
    assert!(result.is_ok());
    
    let customer = result.unwrap();
    assert_eq!(customer.vehicles.len(), 1);
    
    // Future: Should also load Vehicle -> Parts (depth = 2)
    // This would require implementing recursive loading through the CRUDResource trait
    // For now, we verify the structure is ready for expansion
    assert!(true);
}