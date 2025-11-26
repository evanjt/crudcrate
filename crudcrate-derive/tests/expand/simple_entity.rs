//! Simple entity expansion test
//! Verifies that EntityToModels generates correct Create/Update/List models

use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "items")]
#[crudcrate(api_struct = "Item")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key)]
    pub id: Uuid,

    #[crudcrate(sortable, filterable)]
    pub name: String,

    pub active: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

fn main() {}
