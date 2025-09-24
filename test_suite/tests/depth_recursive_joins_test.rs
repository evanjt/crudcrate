/// Multi-level recursive joins test with depth parameter
/// 
/// Tests the core functionality:
/// 1. Customer (depth=3) should load Vehicle → Parts → SubComponents 
/// 2. Validates that depth parameter triggers recursive loading
/// 3. Ensures join(all) fields are loaded when triggered by parent's depth

use crudcrate::{traits::CRUDResource, EntityToModels};
use sea_orm::{entity::prelude::*, Database, DatabaseConnection, Schema, Set};
use uuid::Uuid;

mod test_customer {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "depth_test_customers")]
    #[crudcrate(api_struct = "DepthTestCustomer", generate_router)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, create_model = false, update_model = false, on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        // This field has depth=3, should trigger 3 levels of loading
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, join(one, all, depth = 3))]
        pub vehicles: Vec<super::test_vehicle::DepthTestVehicle>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::test_vehicle::Entity")]
        Vehicles,
    }

    impl Related<super::test_vehicle::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Vehicles.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

mod test_vehicle {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "depth_test_vehicles")]
    #[crudcrate(api_struct = "DepthTestVehicle", generate_router)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, create_model = false, update_model = false, on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable)]
        pub customer_id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub make: String,

        // These fields only have join(all), normally only loaded on direct queries
        // But should be loaded when triggered by Customer's depth=3
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, join(all))]
        pub parts: Vec<super::test_part::DepthTestPart>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::test_customer::Entity",
            from = "Column::CustomerId",
            to = "super::test_customer::Column::Id"
        )]
        Customer,

        #[sea_orm(has_many = "super::test_part::Entity")]
        Parts,
    }

    impl Related<super::test_customer::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Customer.def()
        }
    }

    impl Related<super::test_part::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Parts.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

mod test_part {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "depth_test_parts")]
    #[crudcrate(api_struct = "DepthTestPart", generate_router)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, create_model = false, update_model = false, on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable)]
        pub vehicle_id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        // This field should be loaded when triggered by Customer's depth=3 (3rd level)
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, join(all))]
        pub sub_components: Vec<super::test_sub_component::DepthTestSubComponent>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::test_vehicle::Entity",
            from = "Column::VehicleId",
            to = "super::test_vehicle::Column::Id"
        )]
        Vehicle,

        #[sea_orm(has_many = "super::test_sub_component::Entity")]
        SubComponents,
    }

    impl Related<super::test_vehicle::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Vehicle.def()
        }
    }

    impl Related<super::test_sub_component::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::SubComponents.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

mod test_sub_component {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "depth_test_sub_components")]
    #[crudcrate(api_struct = "DepthTestSubComponent", generate_router)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, create_model = false, update_model = false, on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable)]
        pub part_id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[crudcrate(filterable)]
        pub material: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::test_part::Entity",
            from = "Column::PartId",
            to = "super::test_part::Column::Id"
        )]
        Part,
    }

    impl Related<super::test_part::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Part.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

// Re-export for easier access in tests
pub use test_customer::{DepthTestCustomer, Entity as CustomerEntity, ActiveModel as CustomerActiveModel};
pub use test_vehicle::{DepthTestVehicle, Entity as VehicleEntity, ActiveModel as VehicleActiveModel};
pub use test_part::{DepthTestPart, Entity as PartEntity, ActiveModel as PartActiveModel};
pub use test_sub_component::{DepthTestSubComponent, Entity as SubComponentEntity, ActiveModel as SubComponentActiveModel};

async fn setup_test_database() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to test database");

    let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);

    // Create tables in dependency order
    let statements = [
        schema.create_table_from_entity(CustomerEntity),
        schema.create_table_from_entity(VehicleEntity), 
        schema.create_table_from_entity(PartEntity),
        schema.create_table_from_entity(SubComponentEntity),
    ];

    for statement in statements {
        db.execute_unprepared(&statement.to_string(sea_query::SqliteQueryBuilder))
            .await
            .expect("Failed to create table");
    }

    db
}

async fn seed_test_data(db: &DatabaseConnection) -> (Uuid, Uuid, Uuid, Uuid) {
    let customer_id = Uuid::new_v4();
    let vehicle_id = Uuid::new_v4();
    let part_id = Uuid::new_v4();
    let sub_component_id = Uuid::new_v4();

    // Create customer
    let customer = CustomerActiveModel {
        id: Set(customer_id),
        name: Set("Depth Test Customer".to_string()),
    };
    customer.insert(db).await.expect("Failed to insert customer");

    // Create vehicle
    let vehicle = VehicleActiveModel {
        id: Set(vehicle_id),
        customer_id: Set(customer_id),
        make: Set("Toyota".to_string()),
    };
    vehicle.insert(db).await.expect("Failed to insert vehicle");

    // Create part
    let part = PartActiveModel {
        id: Set(part_id),
        vehicle_id: Set(vehicle_id),
        name: Set("Engine".to_string()),
    };
    part.insert(db).await.expect("Failed to insert part");

    // Create sub-component  
    let sub_component = SubComponentActiveModel {
        id: Set(sub_component_id),
        part_id: Set(part_id),
        name: Set("Piston".to_string()),
        material: Set("Aluminum".to_string()),
    };
    sub_component.insert(db).await.expect("Failed to insert sub-component");

    (customer_id, vehicle_id, part_id, sub_component_id)
}

#[tokio::test]
async fn test_baseline_single_level_joins() {
    let db = setup_test_database().await;
    let (customer_id, vehicle_id, part_id, sub_component_id) = seed_test_data(&db).await;

    // This test should pass - establishes baseline functionality
    let customer = DepthTestCustomer::get_one(&db, customer_id).await
        .expect("Failed to load customer");

    // Level 1: Customer should load vehicles
    assert_eq!(customer.vehicles.len(), 1);
    assert_eq!(customer.vehicles[0].id, vehicle_id);

    // Current behavior: vehicles should NOT have parts loaded (join(all) only, no depth triggering yet)
    assert_eq!(customer.vehicles[0].parts.len(), 0, 
        "Parts should be empty until depth-aware loading is implemented");
}

#[tokio::test] 
async fn test_direct_vehicle_query_loads_parts() {
    let db = setup_test_database().await;
    let (customer_id, vehicle_id, part_id, sub_component_id) = seed_test_data(&db).await;

    // This should work - direct vehicle query should respect join(all) for parts
    let vehicle = DepthTestVehicle::get_one(&db, vehicle_id).await
        .expect("Failed to load vehicle directly");

    // Direct vehicle query should load parts (join(all))
    assert_eq!(vehicle.parts.len(), 1, "Direct vehicle query should load parts via join(all)");
    assert_eq!(vehicle.parts[0].id, part_id);

    // But parts should NOT load sub-components (no depth specified on Vehicle)
    assert_eq!(vehicle.parts[0].sub_components.len(), 0,
        "Sub-components should not load without depth specification");
}

#[tokio::test]
async fn test_depth_3_recursive_loading() {
    let db = setup_test_database().await;
    let (customer_id, vehicle_id, part_id, sub_component_id) = seed_test_data(&db).await;

    // THIS TEST WILL FAIL INITIALLY - this is the target behavior we want to achieve
    let customer = DepthTestCustomer::get_one(&db, customer_id).await
        .expect("Failed to load customer");

    // Level 1: Customer → Vehicle
    assert_eq!(customer.vehicles.len(), 1);
    let vehicle = &customer.vehicles[0];
    assert_eq!(vehicle.id, vehicle_id);

    // Level 2: Vehicle → Parts (should be loaded due to Customer's depth=3)
    assert_eq!(vehicle.parts.len(), 1, 
        "Customer depth=3 should trigger Vehicle parts loading even though Vehicle only has join(all)");
    let part = &vehicle.parts[0];
    assert_eq!(part.id, part_id);

    // Level 3: Parts → SubComponents (should be loaded due to Customer's depth=3)
    assert_eq!(part.sub_components.len(), 1,
        "Customer depth=3 should trigger Part sub_components loading");
    assert_eq!(part.sub_components[0].id, sub_component_id);
    assert_eq!(part.sub_components[0].name, "Piston");
}

#[tokio::test]
async fn test_get_all_with_depth_3() {
    let db = setup_test_database().await;
    let (customer_id, vehicle_id, part_id, sub_component_id) = seed_test_data(&db).await;

    // get_all should also respect depth parameter
    let customers = DepthTestCustomer::get_all(
        &db,
        &sea_orm::Condition::all(),
        test_customer::Column::Name,
        sea_orm::Order::Asc,
        0,
        10,
    ).await.expect("Failed to load customers");

    assert_eq!(customers.len(), 1);
    let customer = &customers[0];

    // Same depth=3 behavior should work in get_all
    assert_eq!(customer.vehicles.len(), 1);
    assert_eq!(customer.vehicles[0].parts.len(), 1,
        "get_all should respect depth=3 for parts");
    assert_eq!(customer.vehicles[0].parts[0].sub_components.len(), 1,
        "get_all should respect depth=3 for sub-components");
}