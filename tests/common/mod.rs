use axum::Router;
use sea_orm::{Database, DatabaseConnection, DbErr};
use sea_orm_migration::prelude::*;

pub mod task_entity;
pub mod todo_entity;

pub async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let db = Database::connect("sqlite::memory:").await?;

    // Run migrations
    Migrator::up(&db, None).await?;

    Ok(db)
}

pub async fn setup_test_db_with_tasks() -> Result<DatabaseConnection, DbErr> {
    let db = Database::connect("sqlite::memory:").await?;

    // Run migrations
    TaskMigrator::up(&db, None).await?;

    Ok(db)
}

pub fn setup_test_app(db: DatabaseConnection) -> Router {
    use todo_entity::{
        create_one_handler, delete_one_handler, get_all_handler, get_one_handler,
        update_one_handler,
    };

    let api = Router::new()
        .route(
            "/todos",
            axum::routing::get(get_all_handler).post(create_one_handler),
        )
        .route(
            "/todos/{id}",
            axum::routing::get(get_one_handler)
                .put(update_one_handler)
                .delete(delete_one_handler),
        )
        .with_state(db);

    Router::new().nest("/api/v1", api)
}

pub fn setup_task_app(db: DatabaseConnection) -> Router {
    use task_entity::{
        create_one_handler, delete_one_handler, get_all_handler, get_one_handler,
        update_one_handler,
    };

    let api = Router::new()
        .route(
            "/tasks",
            axum::routing::get(get_all_handler).post(create_one_handler),
        )
        .route(
            "/tasks/{id}",
            axum::routing::get(get_one_handler)
                .put(update_one_handler)
                .delete(delete_one_handler),
        )
        .with_state(db);

    Router::new().nest("/api/v1", api)
}

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(CreateTodoTable)]
    }
}

pub struct CreateTodoTable;

#[async_trait::async_trait]
impl MigrationName for CreateTodoTable {
    fn name(&self) -> &'static str {
        "m20240101_000001_create_todo_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for CreateTodoTable {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let table = Table::create()
            .table(TodoEntity)
            .if_not_exists()
            .col(
                ColumnDef::new(TodoColumn::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            )
            .col(ColumnDef::new(TodoColumn::Title).string().not_null())
            .col(
                ColumnDef::new(TodoColumn::Completed)
                    .boolean()
                    .not_null()
                    .default(false),
            )
            .col(
                ColumnDef::new(TodoColumn::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .col(
                ColumnDef::new(TodoColumn::UpdatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .to_owned();

        manager.create_table(table).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TodoEntity).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum TodoColumn {
    Id,
    Title,
    Completed,
    CreatedAt,
    UpdatedAt,
}

impl Iden for TodoColumn {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(
            s,
            "{}",
            match self {
                Self::Id => "id",
                Self::Title => "title",
                Self::Completed => "completed",
                Self::CreatedAt => "created_at",
                Self::UpdatedAt => "updated_at",
            }
        )
        .unwrap();
    }
}

#[derive(Debug)]
pub struct TodoEntity;

impl Iden for TodoEntity {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(s, "todos").unwrap();
    }
}

// Task migrations
pub struct TaskMigrator;

#[async_trait::async_trait]
impl MigratorTrait for TaskMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(CreateTaskTable)]
    }
}

pub struct CreateTaskTable;

#[async_trait::async_trait]
impl MigrationName for CreateTaskTable {
    fn name(&self) -> &'static str {
        "m20240101_000002_create_task_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for CreateTaskTable {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let table = Table::create()
            .table(TaskEntity)
            .if_not_exists()
            .col(
                ColumnDef::new(TaskColumn::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            )
            .col(ColumnDef::new(TaskColumn::Title).string().not_null())
            .col(ColumnDef::new(TaskColumn::Description).text().null())
            .col(
                ColumnDef::new(TaskColumn::Completed)
                    .boolean()
                    .not_null()
                    .default(false),
            )
            .col(
                ColumnDef::new(TaskColumn::Priority)
                    .string()
                    .not_null()
                    .default("medium"),
            )
            .col(
                ColumnDef::new(TaskColumn::Status)
                    .string()
                    .not_null()
                    .default("todo"),
            )
            .col(
                ColumnDef::new(TaskColumn::Score)
                    .double()
                    .not_null()
                    .default(0.0),
            )
            .col(
                ColumnDef::new(TaskColumn::Points)
                    .integer()
                    .not_null()
                    .default(0),
            )
            .col(ColumnDef::new(TaskColumn::EstimatedHours).float().null())
            .col(
                ColumnDef::new(TaskColumn::AssigneeCount)
                    .small_integer()
                    .not_null()
                    .default(1),
            )
            .col(
                ColumnDef::new(TaskColumn::IsPublic)
                    .boolean()
                    .not_null()
                    .default(true),
            )
            .col(
                ColumnDef::new(TaskColumn::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .col(
                ColumnDef::new(TaskColumn::UpdatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .to_owned();

        manager.create_table(table).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TaskEntity).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum TaskColumn {
    Id,
    Title,
    Description,
    Completed,
    Priority,
    Status,
    Score,
    Points,
    EstimatedHours,
    AssigneeCount,
    IsPublic,
    CreatedAt,
    UpdatedAt,
}

impl Iden for TaskColumn {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(
            s,
            "{}",
            match self {
                Self::Id => "id",
                Self::Title => "title",
                Self::Description => "description",
                Self::Completed => "completed",
                Self::Priority => "priority",
                Self::Status => "status",
                Self::Score => "score",
                Self::Points => "points",
                Self::EstimatedHours => "estimated_hours",
                Self::AssigneeCount => "assignee_count",
                Self::IsPublic => "is_public",
                Self::CreatedAt => "created_at",
                Self::UpdatedAt => "updated_at",
            }
        )
        .unwrap();
    }
}

#[derive(Debug)]
pub struct TaskEntity;

impl Iden for TaskEntity {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(s, "tasks").unwrap();
    }
}
