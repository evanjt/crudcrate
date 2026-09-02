//! A join field typed `Option<Child>` rather than `Vec<Child>`.
//!
//! The belongs_to direction takes a different loader in every join emitter: it
//! resolves one parent row through the child's own foreign key instead of
//! grouping many children under a parent id. No other fixture is shaped this
//! way, so this is the only snapshot of that path.

use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

pub mod manufacturer {
    use super::*;

    #[derive(Clone, Debug, DeriveEntityModel, EntityToModels, Serialize, Deserialize, ToSchema)]
    #[sea_orm(table_name = "manufacturers")]
    #[crudcrate(api_struct = "Manufacturer")]
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
        fn to() -> RelationDef {
            Relation::Vehicles.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod vehicle {
    use super::*;

    #[derive(Clone, Debug, DeriveEntityModel, EntityToModels, Serialize, Deserialize, ToSchema)]
    #[sea_orm(table_name = "vehicles")]
    #[crudcrate(api_struct = "Vehicle")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key)]
        pub id: Uuid,
        pub manufacturer_id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub make: String,

        // Single related parent, not a collection.
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr, join(one, all, depth = 1))]
        pub manufacturer: Option<super::manufacturer::Manufacturer>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::manufacturer::Entity",
            from = "Column::ManufacturerId",
            to = "super::manufacturer::Column::Id"
        )]
        Manufacturer,
    }

    impl Related<super::manufacturer::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Manufacturer.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

fn main() {
    let _: fn() -> vehicle::Vehicle = || vehicle::Vehicle {
        id: Uuid::nil(),
        manufacturer_id: Uuid::nil(),
        make: "test".to_string(),
        manufacturer: None,
    };
}
