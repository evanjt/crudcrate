//! Test that integer primary keys fail with a clear error.
//!
//! CRUDResource currently requires PrimaryKey::ValueType: From<Uuid> + Into<Uuid>,
//! so non-UUID PKs (i32, i64, String) cannot be used. This test documents the
//! limitation and ensures the error message is visible.

use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "tags")]
#[crudcrate(api_struct = "Tag")]
pub struct Model {
    #[sea_orm(primary_key)]
    #[crudcrate(primary_key, exclude(create, update))]
    pub id: i32,

    #[crudcrate(filterable, sortable)]
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

fn main() {}
