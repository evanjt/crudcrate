//! A join field whose target has a mutual `Related<Self>` impl (a bidirectional
//! relation) MUST specify an explicit depth. Without one the loader would default
//! to depth=5 and recurse into the back-reference, so the derive emits a
//! compile-time error pointing the user at `depth = 1`. This pins that invariant
//! (relation_validator.rs) which previously had no failing-compile test.
//!
//! Mirrors the working ui-pass/join_filter_sort.rs setup exactly; the only
//! difference is the omitted `depth` on the `children` join.

use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub mod parent {
    use super::*;

    #[derive(Clone, Debug, DeriveEntityModel, EntityToModels, Serialize, Deserialize, ToSchema)]
    #[sea_orm(table_name = "bd_parents")]
    #[crudcrate(api_struct = "Parent")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key)]
        pub id: i32,

        // Bidirectional (parent <-> child both impl Related) AND no explicit depth.
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr, join(one, all))]
        pub children: Vec<super::child::Child>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::child::Entity")]
        Children,
    }

    impl Related<super::child::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Children.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod child {
    use super::*;

    #[derive(
        Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels, Serialize, Deserialize, ToSchema,
    )]
    #[sea_orm(table_name = "bd_children")]
    #[crudcrate(api_struct = "Child", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key)]
        pub id: i32,

        pub parent_id: i32,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::parent::Entity",
            from = "Column::ParentId",
            to = "super::parent::Column::Id"
        )]
        Parent,
    }

    impl Related<super::parent::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Parent.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

fn main() {}
