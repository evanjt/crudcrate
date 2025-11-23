//! Test that entity with new hook syntax compiles successfully

use crudcrate::{EntityToModels, ApiError};
use sea_orm::entity::prelude::*;
use uuid::Uuid;

// Hook functions
async fn validate_create(_db: &DatabaseConnection, _data: &AssetCreate) -> Result<(), ApiError> {
    Ok(())
}

async fn after_create(_db: &DatabaseConnection, _entity: &Asset) -> Result<(), ApiError> {
    Ok(())
}

async fn custom_delete(_db: &DatabaseConnection, id: Uuid) -> Result<Uuid, ApiError> {
    Ok(id)
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "assets")]
#[crudcrate(
    api_struct = "Asset",
    // New hook syntax
    create::one::pre = validate_create,
    create::one::post = after_create,
    delete::one::body = custom_delete,
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key)]
    pub id: Uuid,

    #[crudcrate(sortable, filterable)]
    pub filename: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

fn main() {}
