//! `join(relation = "...")` resolves the foreign key from the named variant of
//! the child's `Relation` enum instead of `Related<Parent>::to()`.

use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

pub mod vehicle_part {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, EntityToModels, Serialize, Deserialize, ToSchema)]
    #[sea_orm(table_name = "vehicle_parts")]
    #[crudcrate(api_struct = "VehiclePart", derive_partial_eq, derive_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key)]
        pub id: Uuid,
        pub vehicle_id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::vehicle::Entity",
            from = "Column::VehicleId",
            to = "super::vehicle::Column::Id"
        )]
        Vehicle,
    }

    impl Related<super::vehicle::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Vehicle.def()
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
        #[crudcrate(filterable, sortable)]
        pub make: String,

        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr, join(one, all, depth = 1, relation = "Vehicle"))]
        pub parts: Vec<super::vehicle_part::VehiclePart>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::vehicle_part::Entity")]
        Parts,
    }

    impl Related<super::vehicle_part::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Parts.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

fn main() {
    let _: fn() -> vehicle::Vehicle = || vehicle::Vehicle {
        id: Uuid::nil(),
        make: "test".to_string(),
        parts: Vec::new(),
    };
}
