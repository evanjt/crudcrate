//! Test that a basic entity compiles successfully

use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "todos")]
#[crudcrate(api_struct = "Todo")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(sortable, filterable)]
    pub title: String,

    pub completed: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

fn main() {
    // Verify types are generated - id is auto-generated so not in Create model
    let _: fn() -> TodoCreate = || TodoCreate {
        title: "test".to_string(),
        completed: false,
    };
}
