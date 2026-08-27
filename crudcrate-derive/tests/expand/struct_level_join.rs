//! Struct-level join: the field exists only on the generated API struct
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

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
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(belongs_to = "super::customer::Entity", from = "Column::CustomerId", to = "super::customer::Column::Id")]
        Customer,
    }
    impl Related<super::customer::Entity> for Entity {
        fn to() -> RelationDef { Relation::Customer.def() }
    }
    impl ActiveModelBehavior for ActiveModel {}
}

pub mod customer {
    use super::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "customers")]
    #[crudcrate(
        api_struct = "Customer",
        join(name = "vehicles", result = "Vec<super::vehicle::Vehicle>", one, all, depth = 1, filterable("make"), sortable("make"))
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key)]
        pub id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub name: String,
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
