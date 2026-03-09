use chrono::{DateTime, Utc};
use crudcrate::{EntityToModels, traits::CRUDResource};
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "categories")]
#[crudcrate(api_struct = "Category", generate_router, derive_partial_eq)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,
    #[crudcrate(filterable, sortable)]
    pub name: String,
    #[crudcrate(filterable)]
    pub parent_id: Option<Uuid>,
    #[crudcrate(sortable, exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
    #[crudcrate(sortable, exclude(create, update), on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,

    /// Self-referencing relationship (must use depth=1)
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr = true, exclude(create, update), join(one, depth = 1))]
    pub children: Vec<Category>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    // Self-referencing relationship
    #[sea_orm(has_many = "Entity", from = "Column::Id", to = "Column::ParentId")]
    Children,
}

impl Related<Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Children.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
