//! Test that deprecated fn_delete syntax produces a helpful error

use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

async fn custom_delete(_db: &DatabaseConnection, _id: Uuid) -> Result<Uuid, crudcrate::ApiError> {
    unimplemented!()
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "todos")]
#[crudcrate(
    api_struct = "Todo",
    fn_delete = custom_delete,  // Deprecated syntax - should error
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
