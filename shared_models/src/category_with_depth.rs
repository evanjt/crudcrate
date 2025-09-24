use chrono::{DateTime, Utc};
use crudcrate::{EntityToModels, traits::CRUDResource};
use sea_orm::entity::prelude::*;
use uuid::Uuid;

/// Category model with explicit depth - this should compile without warnings
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "categories_with_depth")]
#[crudcrate(api_struct = "CategoryWithDepth", generate_router)]
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

    // This should NOT trigger a warning because depth is explicitly set
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr = true, join(one, all, depth = 2))]
    pub subcategories: Vec<CategoryWithDepth>,

    // Parent category with explicit depth
    #[sea_orm(ignore)]  
    #[crudcrate(non_db_attr = true, join(one, depth = 1))]
    pub parent_category: Option<CategoryWithDepth>,
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
pub type CategoryWithDepthEntity = Entity;