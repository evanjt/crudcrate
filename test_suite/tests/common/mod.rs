use axum::Router;
use sea_orm::{Database, DatabaseConnection, DbErr};
use sea_orm_migration::prelude::*;
use tokio::sync::Mutex;

// Global mutex to serialize database setup for PostgreSQL to avoid race conditions
static POSTGRES_SETUP_MUTEX: Mutex<()> = Mutex::const_new(());

pub mod task_entity;
pub mod todo_entity;
pub mod customer_entity;
pub mod vehicle_entity;
pub mod vehicle_part_entity;
pub mod maintenance_record_entity;

// Helper function to get database URL from environment or default to SQLite
fn get_test_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite::memory:".to_string())
}


// Cleanup function for persistent databases
async fn cleanup_test_tables(db: &DatabaseConnection) {
    let database_url = get_test_database_url();
    
    // Drop tables in reverse dependency order to avoid foreign key issues
    let _ = db.execute_unprepared("DROP TABLE IF EXISTS maintenance_records").await;
    let _ = db.execute_unprepared("DROP TABLE IF EXISTS vehicle_parts").await;
    let _ = db.execute_unprepared("DROP TABLE IF EXISTS vehicles").await;
    let _ = db.execute_unprepared("DROP TABLE IF EXISTS customers").await;
    let _ = db.execute_unprepared("DROP TABLE IF EXISTS todos").await;
    let _ = db.execute_unprepared("DROP TABLE IF EXISTS tasks").await;
    let _ = db.execute_unprepared("DROP TABLE IF EXISTS index_test_posts").await;
    let _ = db.execute_unprepared("DROP TABLE IF EXISTS benchmark_posts").await;
    
    // PostgreSQL-specific cleanup: drop custom enum types that Sea-ORM creates
    if database_url.starts_with("postgres") {
        // Drop custom enum types used by Sea-ORM (these may exist from previous test runs)
        let _ = db.execute_unprepared("DROP TYPE IF EXISTS status CASCADE").await;
        let _ = db.execute_unprepared("DROP TYPE IF EXISTS priority CASCADE").await;
        let _ = db.execute_unprepared("DROP TYPE IF EXISTS task_status CASCADE").await;
        let _ = db.execute_unprepared("DROP TYPE IF EXISTS task_priority CASCADE").await;
        
        // Drop the Sea-ORM migrations table to allow fresh migrations
        let _ = db.execute_unprepared("DROP TABLE IF EXISTS seaql_migrations CASCADE").await;
    }
    
    // MySQL-specific cleanup
    if database_url.starts_with("mysql") {
        // MySQL doesn't have the same enum issues, but ensure clean migrations
        let _ = db.execute_unprepared("DROP TABLE IF EXISTS seaql_migrations").await;
    }
}

#[allow(dead_code)]
pub async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let database_url = get_test_database_url();
    
    // Serialize PostgreSQL setup to avoid race conditions with custom types
    if database_url.starts_with("postgres") {
        // We need to serialize the entire PostgreSQL setup process to prevent
        // race conditions when creating custom types during migrations
        let _lock = POSTGRES_SETUP_MUTEX.lock().await;
        let db = Database::connect(&database_url).await?;
        cleanup_test_tables(&db).await;
        Migrator::up(&db, None).await?;
        Ok(db)
    } else {
        let db = Database::connect(&database_url).await?;
        
        // For persistent databases, clean up any existing tables
        if !database_url.starts_with("sqlite::memory:") {
            cleanup_test_tables(&db).await;
        }

        // Run migrations
        Migrator::up(&db, None).await?;
        
        Ok(db)
    }
}

#[allow(dead_code)]
pub async fn setup_test_db_with_tasks() -> Result<DatabaseConnection, DbErr> {
    let database_url = get_test_database_url();
    
    // Serialize PostgreSQL setup to avoid race conditions with custom types
    if database_url.starts_with("postgres") {
        // We need to serialize the entire PostgreSQL setup process to prevent
        // race conditions when creating custom types during migrations
        let _lock = POSTGRES_SETUP_MUTEX.lock().await;
        let db = Database::connect(&database_url).await?;
        cleanup_test_tables(&db).await;
        TaskMigrator::up(&db, None).await?;
        Ok(db)
    } else {
        let db = Database::connect(&database_url).await?;
        
        // For persistent databases, clean up any existing tables
        if !database_url.starts_with("sqlite::memory:") {
            cleanup_test_tables(&db).await;
        }

        // Run migrations
        TaskMigrator::up(&db, None).await?;
        
        Ok(db)
    }
}

#[allow(dead_code)]
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

#[allow(dead_code)]
pub async fn setup_customer_vehicle_db() -> Result<DatabaseConnection, DbErr> {
    let database_url = get_test_database_url();
    
    // Serialize PostgreSQL setup to avoid race conditions with custom types
    if database_url.starts_with("postgres") {
        let _lock = POSTGRES_SETUP_MUTEX.lock().await;
        let db = Database::connect(&database_url).await?;
        cleanup_test_tables(&db).await;
        CustomerVehicleMigrator::up(&db, None).await?;
        Ok(db)
    } else {
        let db = Database::connect(&database_url).await?;
        
        // For persistent databases, clean up any existing tables
        if !database_url.starts_with("sqlite::memory:") {
            cleanup_test_tables(&db).await;
        }

        // Run migrations
        CustomerVehicleMigrator::up(&db, None).await?;
        
        Ok(db)
    }
}

#[allow(dead_code)]
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

#[allow(dead_code)]
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(CreateTodoTable)]
    }
}

// Customer-Vehicle-Parts Migrator for testing recursive joins
#[allow(dead_code)]
pub struct CustomerVehicleMigrator;

#[async_trait::async_trait]
impl MigratorTrait for CustomerVehicleMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(CreateCustomerTable),
            Box::new(CreateVehicleTable),
            Box::new(CreateVehiclePartTable),
            Box::new(CreateMaintenanceRecordTable),
        ]
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
            .col(ColumnDef::new(TodoColumn::Title).text().not_null())
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
#[allow(dead_code)]
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
            .col(ColumnDef::new(TaskColumn::Title).text().not_null())
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

// Customer-Vehicle-Parts migrations and schema definitions

pub struct CreateCustomerTable;

#[async_trait::async_trait]
impl MigrationName for CreateCustomerTable {
    fn name(&self) -> &'static str {
        "m20240101_000003_create_customer_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for CreateCustomerTable {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let table = Table::create()
            .table(CustomerEntity)
            .if_not_exists()
            .col(
                ColumnDef::new(CustomerColumn::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            )
            .col(ColumnDef::new(CustomerColumn::Name).text().not_null())
            .col(ColumnDef::new(CustomerColumn::Email).text().not_null())
            .col(
                ColumnDef::new(CustomerColumn::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .col(
                ColumnDef::new(CustomerColumn::UpdatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .to_owned();

        manager.create_table(table).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CustomerEntity).to_owned())
            .await?;
        Ok(())
    }
}

pub struct CreateVehicleTable;

#[async_trait::async_trait]
impl MigrationName for CreateVehicleTable {
    fn name(&self) -> &'static str {
        "m20240101_000004_create_vehicle_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for CreateVehicleTable {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let table = Table::create()
            .table(VehicleEntity)
            .if_not_exists()
            .col(
                ColumnDef::new(VehicleColumn::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            )
            .col(ColumnDef::new(VehicleColumn::CustomerId).uuid().not_null())
            .col(ColumnDef::new(VehicleColumn::Make).text().not_null())
            .col(ColumnDef::new(VehicleColumn::Model).text().not_null())
            .col(ColumnDef::new(VehicleColumn::Year).integer().not_null())
            .col(ColumnDef::new(VehicleColumn::Vin).text().not_null())
            .col(
                ColumnDef::new(VehicleColumn::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .col(
                ColumnDef::new(VehicleColumn::UpdatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .foreign_key(
                ForeignKey::create()
                    .name("fk_vehicle_customer")
                    .from(VehicleEntity, VehicleColumn::CustomerId)
                    .to(CustomerEntity, CustomerColumn::Id)
                    .on_delete(ForeignKeyAction::Cascade),
            )
            .to_owned();

        manager.create_table(table).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(VehicleEntity).to_owned())
            .await?;
        Ok(())
    }
}

pub struct CreateVehiclePartTable;

#[async_trait::async_trait]
impl MigrationName for CreateVehiclePartTable {
    fn name(&self) -> &'static str {
        "m20240101_000005_create_vehicle_part_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for CreateVehiclePartTable {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let table = Table::create()
            .table(VehiclePartEntity)
            .if_not_exists()
            .col(
                ColumnDef::new(VehiclePartColumn::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            )
            .col(ColumnDef::new(VehiclePartColumn::VehicleId).uuid().not_null())
            .col(ColumnDef::new(VehiclePartColumn::Name).text().not_null())
            .col(ColumnDef::new(VehiclePartColumn::PartNumber).text().not_null())
            .col(ColumnDef::new(VehiclePartColumn::Category).text().not_null())
            .col(ColumnDef::new(VehiclePartColumn::Price).decimal().null())
            .col(
                ColumnDef::new(VehiclePartColumn::InStock)
                    .boolean()
                    .not_null()
                    .default(true),
            )
            .col(
                ColumnDef::new(VehiclePartColumn::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .col(
                ColumnDef::new(VehiclePartColumn::UpdatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .foreign_key(
                ForeignKey::create()
                    .name("fk_vehicle_part_vehicle")
                    .from(VehiclePartEntity, VehiclePartColumn::VehicleId)
                    .to(VehicleEntity, VehicleColumn::Id)
                    .on_delete(ForeignKeyAction::Cascade),
            )
            .to_owned();

        manager.create_table(table).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(VehiclePartEntity).to_owned())
            .await?;
        Ok(())
    }
}

pub struct CreateMaintenanceRecordTable;

#[async_trait::async_trait]
impl MigrationName for CreateMaintenanceRecordTable {
    fn name(&self) -> &'static str {
        "m20240101_000006_create_maintenance_record_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for CreateMaintenanceRecordTable {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let table = Table::create()
            .table(MaintenanceRecordEntity)
            .if_not_exists()
            .col(
                ColumnDef::new(MaintenanceRecordColumn::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            )
            .col(ColumnDef::new(MaintenanceRecordColumn::VehicleId).uuid().not_null())
            .col(ColumnDef::new(MaintenanceRecordColumn::ServiceType).text().not_null())
            .col(ColumnDef::new(MaintenanceRecordColumn::Description).text().not_null())
            .col(ColumnDef::new(MaintenanceRecordColumn::Cost).decimal().null())
            .col(
                ColumnDef::new(MaintenanceRecordColumn::ServiceDate)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .col(ColumnDef::new(MaintenanceRecordColumn::MechanicName).text().null())
            .col(
                ColumnDef::new(MaintenanceRecordColumn::Completed)
                    .boolean()
                    .not_null()
                    .default(false),
            )
            .col(
                ColumnDef::new(MaintenanceRecordColumn::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .col(
                ColumnDef::new(MaintenanceRecordColumn::UpdatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .foreign_key(
                ForeignKey::create()
                    .name("fk_maintenance_record_vehicle")
                    .from(MaintenanceRecordEntity, MaintenanceRecordColumn::VehicleId)
                    .to(VehicleEntity, VehicleColumn::Id)
                    .on_delete(ForeignKeyAction::Cascade),
            )
            .to_owned();

        manager.create_table(table).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MaintenanceRecordEntity).to_owned())
            .await?;
        Ok(())
    }
}

// Column enums and entity definitions

#[derive(Debug)]
pub enum CustomerColumn {
    Id,
    Name,
    Email,
    CreatedAt,
    UpdatedAt,
}

impl Iden for CustomerColumn {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(
            s,
            "{}",
            match self {
                Self::Id => "id",
                Self::Name => "name",
                Self::Email => "email",
                Self::CreatedAt => "created_at",
                Self::UpdatedAt => "updated_at",
            }
        )
        .unwrap();
    }
}

#[derive(Debug)]
pub struct CustomerEntity;

impl Iden for CustomerEntity {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(s, "customers").unwrap();
    }
}

#[derive(Debug)]
pub enum VehicleColumn {
    Id,
    CustomerId,
    Make,
    Model,
    Year,
    Vin,
    CreatedAt,
    UpdatedAt,
}

impl Iden for VehicleColumn {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(
            s,
            "{}",
            match self {
                Self::Id => "id",
                Self::CustomerId => "customer_id",
                Self::Make => "make",
                Self::Model => "model",
                Self::Year => "year",
                Self::Vin => "vin",
                Self::CreatedAt => "created_at",
                Self::UpdatedAt => "updated_at",
            }
        )
        .unwrap();
    }
}

#[derive(Debug)]
pub struct VehicleEntity;

impl Iden for VehicleEntity {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(s, "vehicles").unwrap();
    }
}

#[derive(Debug)]
pub enum VehiclePartColumn {
    Id,
    VehicleId,
    Name,
    PartNumber,
    Category,
    Price,
    InStock,
    CreatedAt,
    UpdatedAt,
}

impl Iden for VehiclePartColumn {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(
            s,
            "{}",
            match self {
                Self::Id => "id",
                Self::VehicleId => "vehicle_id",
                Self::Name => "name",
                Self::PartNumber => "part_number",
                Self::Category => "category",
                Self::Price => "price",
                Self::InStock => "in_stock",
                Self::CreatedAt => "created_at",
                Self::UpdatedAt => "updated_at",
            }
        )
        .unwrap();
    }
}

#[derive(Debug)]
pub struct VehiclePartEntity;

impl Iden for VehiclePartEntity {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(s, "vehicle_parts").unwrap();
    }
}

#[derive(Debug)]
pub enum MaintenanceRecordColumn {
    Id,
    VehicleId,
    ServiceType,
    Description,
    Cost,
    ServiceDate,
    MechanicName,
    Completed,
    CreatedAt,
    UpdatedAt,
}

impl Iden for MaintenanceRecordColumn {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(
            s,
            "{}",
            match self {
                Self::Id => "id",
                Self::VehicleId => "vehicle_id",
                Self::ServiceType => "service_type",
                Self::Description => "description",
                Self::Cost => "cost",
                Self::ServiceDate => "service_date",
                Self::MechanicName => "mechanic_name",
                Self::Completed => "completed",
                Self::CreatedAt => "created_at",
                Self::UpdatedAt => "updated_at",
            }
        )
        .unwrap();
    }
}

#[derive(Debug)]
pub struct MaintenanceRecordEntity;

impl Iden for MaintenanceRecordEntity {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(s, "maintenance_records").unwrap();
    }
}
