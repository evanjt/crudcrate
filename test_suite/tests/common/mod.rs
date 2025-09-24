use axum::Router;
use sea_orm::{Database, DatabaseConnection, DbErr};
use sea_orm_migration::prelude::*;
use tokio::sync::Mutex;

// Re-export shared models for easy access
pub use shared_models::{
    Customer, CustomerEntity,
    Vehicle, VehicleEntity, 
    VehiclePart, VehiclePartEntity,
    MaintenanceRecord, MaintenanceRecordEntity,
};

// Global mutex to serialize database setup for PostgreSQL to avoid race conditions
static POSTGRES_SETUP_MUTEX: Mutex<()> = Mutex::const_new(());

// Helper function to get database URL from environment or default to SQLite
fn get_test_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite::memory:".to_string())
}

// Cleanup function for persistent databases
async fn cleanup_test_tables(db: &DatabaseConnection) {
    // Drop tables in reverse dependency order to avoid foreign key issues
    let _ = db.execute_unprepared("DROP TABLE IF EXISTS maintenance_records").await;
    let _ = db.execute_unprepared("DROP TABLE IF EXISTS vehicle_parts").await;
    let _ = db.execute_unprepared("DROP TABLE IF EXISTS vehicles").await;
    let _ = db.execute_unprepared("DROP TABLE IF EXISTS customers").await;
}

#[allow(dead_code)]
pub async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
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
pub fn setup_test_app(db: DatabaseConnection) -> Router {
    // Create a simple router that uses the generated CRUD endpoints from shared_models  
    Router::new()
        .nest("/customers", Customer::router(&db).into())
        .nest("/vehicles", Vehicle::router(&db).into())
}

// Customer-Vehicle-Parts Migrator for testing
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