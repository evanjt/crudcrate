//! Test that filterable() and sortable() inside join() compile correctly
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// Child entity
pub mod vehicle {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, EntityToModels, Serialize, Deserialize, ToSchema)]
    #[sea_orm(table_name = "vehicles")]
    #[crudcrate(api_struct = "Vehicle", derive_partial_eq, derive_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key)]
        pub id: Uuid,
        pub customer_id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub make: String,
        #[crudcrate(filterable, sortable)]
        pub year: i32,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::customer::Entity",
            from = "Column::CustomerId",
            to = "super::customer::Column::Id"
        )]
        Customer,
    }

    impl Related<super::customer::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Customer.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

// Parent entity with nested filterable/sortable in join()
pub mod customer {
    use super::*;

    #[derive(Clone, Debug, DeriveEntityModel, EntityToModels, Serialize, Deserialize, ToSchema)]
    #[sea_orm(table_name = "customers")]
    #[crudcrate(api_struct = "Customer")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key)]
        pub id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[sea_orm(ignore)]
        #[crudcrate(
            non_db_attr,
            join(one, all, depth = 1, filterable("make", "year"), sortable("year"))
        )]
        pub vehicles: Vec<super::vehicle::Vehicle>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::vehicle::Entity")]
        Vehicles,
    }

    impl Related<super::vehicle::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Vehicles.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

fn main() {
    use crudcrate::CRUDResource;

    // Verify trait methods are generated
    let filterable = customer::Customer::joined_filterable_columns();
    assert_eq!(filterable.len(), 2); // make, year

    let sortable = customer::Customer::joined_sortable_columns();
    assert_eq!(sortable.len(), 1); // year
}
