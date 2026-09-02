//! Cross-model join chain: Customer -> Vehicle -> Part, depth 3 on the root
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

pub mod part {
    use super::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "parts")]
    #[crudcrate(api_struct = "Part")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key)]
        pub id: Uuid,
        pub vehicle_id: Uuid,
        #[crudcrate(filterable)]
        pub name: String,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(belongs_to = "super::vehicle::Entity", from = "Column::VehicleId", to = "super::vehicle::Column::Id")]
        Vehicle,
    }
    impl Related<super::vehicle::Entity> for Entity {
        fn to() -> RelationDef { Relation::Vehicle.def() }
    }
    impl ActiveModelBehavior for ActiveModel {}
}

pub mod vehicle {
    use super::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "vehicles")]
    #[crudcrate(api_struct = "Vehicle")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key)]
        pub id: Uuid,
        pub customer_id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub make: String,
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr, join(one, all, depth = 2))]
        pub parts: Vec<super::part::Part>,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(belongs_to = "super::customer::Entity", from = "Column::CustomerId", to = "super::customer::Column::Id")]
        Customer,
        #[sea_orm(has_many = "super::part::Entity")]
        Parts,
    }
    impl Related<super::customer::Entity> for Entity {
        fn to() -> RelationDef { Relation::Customer.def() }
    }
    impl Related<super::part::Entity> for Entity {
        fn to() -> RelationDef { Relation::Parts.def() }
    }
    impl ActiveModelBehavior for ActiveModel {}
}

pub mod customer {
    use super::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "customers")]
    #[crudcrate(api_struct = "Customer", generate_router)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key)]
        pub id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub name: String,
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr, join(one, all, depth = 3))]
        pub vehicles: Vec<super::vehicle::Vehicle>,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::vehicle::Entity")]
        Vehicles,
    }
    impl Related<super::vehicle::Entity> for Entity {
        fn to() -> RelationDef { Relation::Vehicles.def() }
    }
    impl ActiveModelBehavior for ActiveModel {}
}

fn main() {}
