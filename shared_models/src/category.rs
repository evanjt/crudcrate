use chrono::{DateTime, Utc};
use crudcrate::{EntityToModels, traits::CRUDResource};
use sea_orm::entity::prelude::*;
use uuid::Uuid;

/// Category model demonstrating potential cyclic dependency
/// This should trigger a compiler warning since it references itself without explicit depth
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "categories")]
#[crudcrate(api_struct = "Category", generate_router)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, create_model = false, update_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub name: String,

    #[crudcrate(filterable)]
    pub description: Option<String>,

    #[sea_orm(column_type = "Uuid", nullable)]
    #[crudcrate(filterable)]
    pub parent_id: Option<Uuid>,

    #[crudcrate(sortable, create_model = false, update_model = false, on_create = Utc::now())]
    pub created_at: DateTime<Utc>,

    #[crudcrate(sortable, create_model = false, update_model = false, on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,

    // This should trigger a cyclic dependency warning because Category references itself
    // without explicit depth - should default to depth=3 with a compiler warning
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all))]
    pub subcategories: Vec<Box<Model>>,

    // Parent category - another self-reference
    #[sea_orm(ignore)]  
    #[crudcrate(non_db_attr, join(one))]
    pub parent_category: Option<Box<Model>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        has_many = "Entity",
        from = "Column::Id",
        to = "Column::ParentId"
    )]
    Subcategories,

    #[sea_orm(
        belongs_to = "Entity", 
        from = "Column::ParentId",
        to = "Column::Id"
    )]
    ParentCategory,
}

impl Related<Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Subcategories.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// Type alias for easier importing  
pub type CategoryEntity = Entity;