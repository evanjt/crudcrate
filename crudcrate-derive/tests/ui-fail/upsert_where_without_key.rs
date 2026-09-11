//! `upsert_where` narrows the key a registration finds a row by. Without a key there is no find
//! to narrow, so a predicate on its own is a declaration that does nothing, and the derive says
//! so rather than accepting it.

use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "uw_episodes")]
#[crudcrate(api_struct = "Episode", upsert_where = Column::ResolvedAt.is_null())]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key)]
    pub id: i32,
    pub slot: String,
    pub resolved_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

fn main() {}
