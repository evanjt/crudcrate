use async_trait::async_trait;
use chrono::{DateTime, Utc};
use crudcrate::traits::CRUDResource;
use crudcrate::{ToCreateModel, ToUpdateModel, crud_handlers};
use sea_orm::{ActiveValue, FromQueryResult, entity::prelude::*};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Deserialize, Serialize)]
#[sea_orm(table_name = "todos")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(column_type = "Text")]
    pub title: String,
    pub completed: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(
    ToSchema,
    Serialize,
    Deserialize,
    FromQueryResult,
    Clone,
    Debug,
    PartialEq,
    ToUpdateModel,
    ToCreateModel,
)]
#[active_model = "ActiveModel"]
pub struct Todo {
    #[crudcrate(update_model = false, create_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,
    pub title: String,
    #[crudcrate(on_create = false)]
    pub completed: bool,
    #[crudcrate(update_model = false, create_model = false, on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
    #[crudcrate(update_model = false, create_model = false, on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,
}

impl From<Model> for Todo {
    fn from(model: Model) -> Self {
        Todo {
            id: model.id,
            title: model.title,
            completed: model.completed,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[async_trait]
impl CRUDResource for Todo {
    type EntityType = Entity;
    type ColumnType = Column;
    type ActiveModelType = ActiveModel;
    type CreateModel = TodoCreate;
    type UpdateModel = TodoUpdate;
    type ListModel = Todo; // No optimization for this legacy test entity

    const ID_COLUMN: Self::ColumnType = Column::Id;
    const RESOURCE_NAME_SINGULAR: &'static str = "todo";
    const RESOURCE_NAME_PLURAL: &'static str = "todos";
    const TABLE_NAME: &'static str = "todos";
    const RESOURCE_DESCRIPTION: &'static str = "Todo items for testing";

    fn sortable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        vec![
            ("title", Column::Title),
            ("completed", Column::Completed),
            ("created_at", Column::CreatedAt),
            ("updated_at", Column::UpdatedAt),
        ]
    }

    fn filterable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        vec![("title", Column::Title), ("completed", Column::Completed)]
    }

    fn like_filterable_columns() -> Vec<&'static str> {
        vec!["title"] // Only title should use LIKE queries, completed (bool) uses exact
    }
}

crud_handlers!(Todo, TodoUpdate, TodoCreate);

pub mod prelude {}
