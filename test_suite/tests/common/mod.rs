use axum::Router;
use sea_orm::{Database, DatabaseConnection, DbErr};
use sea_orm_migration::prelude::*;

// Import local test models
pub mod models;

// Re-export local test models for easy access
pub use self::models::{category, customer, maintenance_record, vehicle, vehicle_part};

// Helper function to get database URL from environment or default to SQLite
fn get_test_database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string())
}

pub async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let database_url = get_test_database_url();

    // Connect and run migrations
    let db = Database::connect(&database_url).await?;
    CustomerVehicleMigrator::up(&db, None).await?;

    // For persistent databases, clear data between tests
    // SQLite in-memory: No cleanup needed, database is fresh
    // PostgreSQL/MySQL: DELETE to clear data (fast, keeps schema)
    if !database_url.starts_with("sqlite::memory:") {
        // Clear in reverse dependency order
        let _ = db
            .execute_unprepared("DELETE FROM maintenance_records")
            .await;
        let _ = db.execute_unprepared("DELETE FROM vehicle_parts").await;
        let _ = db.execute_unprepared("DELETE FROM vehicles").await;
        let _ = db.execute_unprepared("DELETE FROM customers").await;
        let _ = db.execute_unprepared("DELETE FROM categories").await;
    }

    Ok(db)
}

#[allow(dead_code)] // Used in tests
pub fn setup_test_app(db: &DatabaseConnection) -> Router {
    // Create a simple router that uses the generated CRUD endpoints from local models
    Router::new()
        .nest("/categories", category::Category::router(db).into())
        .nest("/customers", customer::Customer::router(db).into())
        .nest("/vehicles", vehicle::Vehicle::router(db).into())
        .nest(
            "/vehicle_parts",
            vehicle_part::VehiclePart::router(db).into(),
        )
        .nest(
            "/maintenance_records",
            maintenance_record::MaintenanceRecord::router(db).into(),
        )
}

// Customer-Vehicle-Parts Migrator for testing
pub struct CustomerVehicleMigrator;

#[async_trait::async_trait]
impl MigratorTrait for CustomerVehicleMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(CreateCategoryTable),
            Box::new(CreateCustomerTable),
            Box::new(CreateVehicleTable),
            Box::new(CreateVehiclePartTable),
            Box::new(CreateMaintenanceRecordTable),
        ]
    }
}

pub struct CreateCategoryTable;

#[async_trait::async_trait]
impl MigrationName for CreateCategoryTable {
    fn name(&self) -> &'static str {
        "m20240101_000002_create_category_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for CreateCategoryTable {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let table = Table::create()
            .table(category::Entity)
            .if_not_exists()
            .col(
                ColumnDef::new(category::Column::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            )
            .col(ColumnDef::new(category::Column::Name).text().not_null())
            .col(ColumnDef::new(category::Column::ParentId).uuid().null())
            .col(
                ColumnDef::new(category::Column::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .col(
                ColumnDef::new(category::Column::UpdatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .foreign_key(
                ForeignKey::create()
                    .name("fk_category_parent")
                    .from(category::Entity, category::Column::ParentId)
                    .to(category::Entity, category::Column::Id)
                    .on_delete(ForeignKeyAction::Cascade),
            )
            .to_owned();

        manager.create_table(table).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(category::Entity).to_owned())
            .await?;
        Ok(())
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
            .table(customer::Entity)
            .if_not_exists()
            .col(
                ColumnDef::new(customer::Column::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            )
            .col(ColumnDef::new(customer::Column::Name).text().not_null())
            .col(ColumnDef::new(customer::Column::Email).text().not_null())
            .col(
                ColumnDef::new(customer::Column::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .col(
                ColumnDef::new(customer::Column::UpdatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .to_owned();

        manager.create_table(table).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(customer::Entity).to_owned())
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
            .table(vehicle::Entity)
            .if_not_exists()
            .col(
                ColumnDef::new(vehicle::Column::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            )
            .col(
                ColumnDef::new(vehicle::Column::CustomerId)
                    .uuid()
                    .not_null(),
            )
            .col(ColumnDef::new(vehicle::Column::Make).text().not_null())
            .col(ColumnDef::new(vehicle::Column::Model).text().not_null())
            .col(ColumnDef::new(vehicle::Column::Year).integer().not_null())
            .col(ColumnDef::new(vehicle::Column::Vin).text().not_null())
            .col(
                ColumnDef::new(vehicle::Column::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .col(
                ColumnDef::new(vehicle::Column::UpdatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .foreign_key(
                ForeignKey::create()
                    .name("fk_vehicle_customer")
                    .from(vehicle::Entity, vehicle::Column::CustomerId)
                    .to(customer::Entity, customer::Column::Id)
                    .on_delete(ForeignKeyAction::Cascade),
            )
            .to_owned();

        manager.create_table(table).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(vehicle::Entity).to_owned())
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
            .table(vehicle_part::Entity)
            .if_not_exists()
            .col(
                ColumnDef::new(vehicle_part::Column::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            )
            .col(
                ColumnDef::new(vehicle_part::Column::VehicleId)
                    .uuid()
                    .not_null(),
            )
            .col(ColumnDef::new(vehicle_part::Column::Name).text().not_null())
            .col(
                ColumnDef::new(vehicle_part::Column::PartNumber)
                    .text()
                    .not_null(),
            )
            .col(
                ColumnDef::new(vehicle_part::Column::Category)
                    .text()
                    .not_null(),
            )
            .col(ColumnDef::new(vehicle_part::Column::Price).decimal().null())
            .col(
                ColumnDef::new(vehicle_part::Column::InStock)
                    .boolean()
                    .not_null()
                    .default(true),
            )
            .col(
                ColumnDef::new(vehicle_part::Column::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .col(
                ColumnDef::new(vehicle_part::Column::UpdatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .foreign_key(
                ForeignKey::create()
                    .name("fk_vehicle_part_vehicle")
                    .from(vehicle_part::Entity, vehicle_part::Column::VehicleId)
                    .to(vehicle::Entity, vehicle::Column::Id)
                    .on_delete(ForeignKeyAction::Cascade),
            )
            .to_owned();

        manager.create_table(table).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(vehicle_part::Entity).to_owned())
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
            .table(maintenance_record::Entity)
            .if_not_exists()
            .col(
                ColumnDef::new(maintenance_record::Column::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            )
            .col(
                ColumnDef::new(maintenance_record::Column::VehicleId)
                    .uuid()
                    .not_null(),
            )
            .col(
                ColumnDef::new(maintenance_record::Column::ServiceType)
                    .text()
                    .not_null(),
            )
            .col(
                ColumnDef::new(maintenance_record::Column::Description)
                    .text()
                    .not_null(),
            )
            .col(
                ColumnDef::new(maintenance_record::Column::Cost)
                    .decimal()
                    .null(),
            ) // Temporarily disabled
            .col(
                ColumnDef::new(maintenance_record::Column::ServiceDate)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .col(
                ColumnDef::new(maintenance_record::Column::MechanicName)
                    .text()
                    .null(),
            )
            .col(
                ColumnDef::new(maintenance_record::Column::Completed)
                    .boolean()
                    .not_null()
                    .default(false),
            )
            .col(
                ColumnDef::new(maintenance_record::Column::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .col(
                ColumnDef::new(maintenance_record::Column::UpdatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .foreign_key(
                ForeignKey::create()
                    .name("fk_maintenance_record_vehicle")
                    .from(
                        maintenance_record::Entity,
                        maintenance_record::Column::VehicleId,
                    )
                    .to(vehicle::Entity, vehicle::Column::Id)
                    .on_delete(ForeignKeyAction::Cascade),
            )
            .to_owned();

        manager.create_table(table).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(maintenance_record::Entity).to_owned())
            .await?;
        Ok(())
    }
}

// Helper function to create a test customer and return the customer ID
// This is needed for vehicle tests because vehicles have a foreign key to customers
#[allow(dead_code)]
pub async fn create_test_customer(app: &Router) -> String {
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::json;
    use tower::ServiceExt;

    let customer_data = json!({
        "name": "Test Customer for Vehicles",
        "email": "vehicle-test@example.com"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/customers")
                .header("content-type", "application/json")
                .body(Body::from(customer_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_customer: serde_json::Value = serde_json::from_slice(&body).unwrap();
    created_customer["id"].as_str().unwrap().to_string()
}
