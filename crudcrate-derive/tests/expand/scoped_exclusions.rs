//! exclude(scoped) with a router, exercising ScopedList and ScopedResponse
use chrono::{DateTime, Utc};
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "customers")]
#[crudcrate(api_struct = "Customer", generate_router, derive_partial_eq)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,
    #[crudcrate(filterable, sortable, like_filterable)]
    pub name: String,
    #[crudcrate(filterable)]
    pub email: String,
    #[crudcrate(sortable, exclude(create, update, one), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
    #[crudcrate(sortable, exclude(create, update, list), on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,
    #[crudcrate(filterable, exclude(scoped, create), on_create = false)]
    pub is_private: bool,
    #[crudcrate(exclude(scoped))]
    pub internal_note: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

fn main() {}
