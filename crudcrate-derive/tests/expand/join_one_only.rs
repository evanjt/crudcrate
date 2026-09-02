//! `join(one)` and `join(all)` on their own.
//!
//! Every other join fixture declares `join(one, all)`, so the emitters that key
//! off one list being empty (get_all with no all-joins, get_one with no
//! one-joins) never appear in a snapshot.

use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

pub mod part {
    use super::*;

    #[derive(Clone, Debug, DeriveEntityModel, EntityToModels, Serialize, Deserialize, ToSchema)]
    #[sea_orm(table_name = "parts")]
    #[crudcrate(api_struct = "Part")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key)]
        pub id: Uuid,
        pub machine_id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::machine::Entity",
            from = "Column::MachineId",
            to = "super::machine::Column::Id"
        )]
        Machine,
    }

    impl Related<super::machine::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Machine.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod log {
    use super::*;

    #[derive(Clone, Debug, DeriveEntityModel, EntityToModels, Serialize, Deserialize, ToSchema)]
    #[sea_orm(table_name = "logs")]
    #[crudcrate(api_struct = "Log")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key)]
        pub id: Uuid,
        pub machine_id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub message: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::machine::Entity",
            from = "Column::MachineId",
            to = "super::machine::Column::Id"
        )]
        Machine,
    }

    impl Related<super::machine::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Machine.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod machine {
    use super::*;

    #[derive(Clone, Debug, DeriveEntityModel, EntityToModels, Serialize, Deserialize, ToSchema)]
    #[sea_orm(table_name = "machines")]
    #[crudcrate(api_struct = "Machine")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key)]
        pub id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub label: String,

        // Detail view only: absent from the list response.
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr, join(one, depth = 1))]
        pub parts: Vec<super::part::Part>,

        // List view only: absent from the detail response.
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr, join(all, depth = 1))]
        pub logs: Vec<super::log::Log>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::part::Entity")]
        Parts,
        #[sea_orm(has_many = "super::log::Entity")]
        Logs,
    }

    impl Related<super::part::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Parts.def()
        }
    }

    impl Related<super::log::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Logs.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

fn main() {
    let _: fn() -> machine::Machine = || machine::Machine {
        id: Uuid::nil(),
        label: "test".to_string(),
        parts: Vec::new(),
        logs: Vec::new(),
    };
}
