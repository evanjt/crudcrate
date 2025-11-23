//! Test that deprecated fn_create syntax produces a helpful error

use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

async fn custom_create(_db: &DatabaseConnection, _data: TodoCreate) -> Result<Todo, crudcrate::ApiError> {
    unimplemented!()
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "todos")]
#[crudcrate(
    api_struct = "Todo",
    fn_create = custom_create,  // Deprecated syntax - should error
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key)]
    pub id: Uuid,
    pub title: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

fn main() {}
