//! Test that an integer (`i32`) primary key compiles successfully.
//!
//! CRUDResource no longer requires `PrimaryKey::ValueType: From<Uuid> + Into<Uuid>`,
//! so non-UUID primary keys (here, an auto-increment `i32`) are accepted and the
//! generated CRUD methods are keyed by the entity's own PK value type.

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

    #[crudcrate(filterable)]
    pub color: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

fn main() {
    // The Create model excludes the auto-generated integer id.
    let _: fn() -> TagCreate = || TagCreate {
        name: "rust".to_string(),
        color: Some("#DEA584".to_string()),
    };
}
