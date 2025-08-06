use async_trait::async_trait;
use chrono::{DateTime, Utc};
use crudcrate::traits::CRUDResource;
use crudcrate::{ToCreateModel, ToUpdateModel, crud_handlers};
use sea_orm::{ActiveValue, DeriveActiveEnum, FromQueryResult, entity::prelude::*};
use sea_orm_migration::sea_query::StringLen;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// Define enums for testing
#[derive(
    Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(50))")]
pub enum Priority {
    #[sea_orm(string_value = "Low")]
    Low,
    #[sea_orm(string_value = "Medium")]
    Medium,
    #[sea_orm(string_value = "High")]
    High,
    #[sea_orm(string_value = "Urgent")]
    Urgent,
}

#[derive(
    Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize, ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(50))")]
pub enum Status {
    #[sea_orm(string_value = "Todo")]
    Todo,
    #[sea_orm(string_value = "InProgress")]
    InProgress,
    #[sea_orm(string_value = "Done")]
    Done,
    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Deserialize, Serialize)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(column_type = "Text")]
    pub title: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,
    pub completed: bool,
    pub priority: Priority,
    pub status: Status,
    pub score: f64,
    pub points: i32,
    pub estimated_hours: Option<f32>,
    pub assignee_count: i16,
    pub is_public: bool,
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
pub struct Task {
    #[crudcrate(update_model = false, create_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    #[crudcrate(on_create = false)]
    pub completed: bool,
    #[crudcrate(on_create = Priority::Medium)]
    pub priority: Priority,
    #[crudcrate(on_create = Status::Todo)]
    pub status: Status,
    #[crudcrate(on_create = 0.0)]
    pub score: f64,
    #[crudcrate(on_create = 0)]
    pub points: i32,
    pub estimated_hours: Option<f32>,
    #[crudcrate(on_create = 1i16)]
    pub assignee_count: i16,
    #[crudcrate(on_create = true)]
    pub is_public: bool,
    #[crudcrate(update_model = false, create_model = false, on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
    #[crudcrate(update_model = false, create_model = false, on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,
}

impl From<Model> for Task {
    fn from(model: Model) -> Self {
        Task {
            id: model.id,
            title: model.title,
            description: model.description,
            completed: model.completed,
            priority: model.priority,
            status: model.status,
            score: model.score,
            points: model.points,
            estimated_hours: model.estimated_hours,
            assignee_count: model.assignee_count,
            is_public: model.is_public,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[async_trait]
impl CRUDResource for Task {
    type EntityType = Entity;
    type ColumnType = Column;
    type ActiveModelType = ActiveModel;
    type CreateModel = TaskCreate;
    type UpdateModel = TaskUpdate;
    type ListModel = Task; // No optimization for this legacy test entity

    const ID_COLUMN: Self::ColumnType = Column::Id;
    const RESOURCE_NAME_SINGULAR: &'static str = "task";
    const RESOURCE_NAME_PLURAL: &'static str = "tasks";
    const RESOURCE_DESCRIPTION: &'static str = "Task items for comprehensive type testing";

    fn sortable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        vec![
            ("title", Column::Title),
            ("completed", Column::Completed),
            ("priority", Column::Priority),
            ("status", Column::Status),
            ("score", Column::Score),
            ("points", Column::Points),
            ("estimated_hours", Column::EstimatedHours),
            ("assignee_count", Column::AssigneeCount),
            ("is_public", Column::IsPublic),
            ("created_at", Column::CreatedAt),
            ("updated_at", Column::UpdatedAt),
        ]
    }

    fn filterable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        vec![
            ("title", Column::Title),
            ("description", Column::Description),
            ("completed", Column::Completed),
            ("priority", Column::Priority),
            ("status", Column::Status),
            ("score", Column::Score),
            ("points", Column::Points),
            ("estimated_hours", Column::EstimatedHours),
            ("assignee_count", Column::AssigneeCount),
            ("is_public", Column::IsPublic),
        ]
    }

    fn like_filterable_columns() -> Vec<&'static str> {
        vec!["title", "description"] // Text fields use LIKE, enums/numbers use exact
    }
}

crud_handlers!(Task, TaskUpdate, TaskCreate);

pub mod prelude {}
