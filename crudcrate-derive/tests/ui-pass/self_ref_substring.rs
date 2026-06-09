//! Test that a join target whose type name contains the parent struct name as
//! a substring is not treated as self-referencing.
//!
//! `Vehicle` has a `Vec<VehiclePart>` join field. "Vehicle" is a substring of
//! "VehiclePart", but the two are distinct types. Self-referencing detection
//! must use an exact inner-type match, not a substring test, otherwise this
//! field would be wrongly forced down the self-referencing code path.

use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// Child entity whose api struct name ("VehiclePart") contains the parent
// api struct name ("Vehicle") as a substring.
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

        // Vec<VehiclePart> is not self-referencing despite the substring overlap.
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr, join(one, all, depth = 1))]
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
    // The cross-model join builds a Vehicle that carries the related collection.
    let _: fn() -> vehicle::Vehicle = || vehicle::Vehicle {
        id: Uuid::nil(),
        make: "test".to_string(),
        parts: Vec::new(),
    };
}
