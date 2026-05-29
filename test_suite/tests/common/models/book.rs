use chrono::{DateTime, Utc};
use crudcrate::{traits::CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "books")]
#[crudcrate(api_struct = "Book", generate_router, derive_partial_eq)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,
    #[crudcrate(filterable, sortable)]
    pub title: String,
    #[crudcrate(filterable)]
    pub author_ref: Uuid,
    #[crudcrate(sortable, on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::author::Entity",
        from = "Column::AuthorRef",
        to = "super::author::Column::Id"
    )]
    Author,
    #[sea_orm(
        belongs_to = "super::managed_author::Entity",
        from = "Column::AuthorRef",
        to = "super::managed_author::Column::Id"
    )]
    ManagedAuthor,
}

impl Related<super::author::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Author.def()
    }
}

impl Related<super::managed_author::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ManagedAuthor.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
