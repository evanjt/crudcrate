//! # CRUDOperations & Lifecycle Hooks
//!
//! Demonstrates the CRUDOperations trait - customize CRUD behavior by implementing hooks
//! and overriding operations. All customization logic lives in one place.
//!
//! ## Available Customization Levels:
//!
//! **Level 1: Lifecycle Hooks** (before_*/after_*)
//! - Validation, logging, side effects
//! - Original operation logic stays intact
//!
//! **Level 2: Core Method Overrides** (fetch_one, fetch_all, etc.)
//! - Custom queries, filtering, authorization
//! - Full control over data retrieval
//!
//! **Level 3: Full Operation Overrides** (create, update, delete)
//! - Complex multi-step operations
//! - External service integration
//!
//! Run with: `cargo run --example crud_operations`

use async_trait::async_trait;
use axum::Router;
use sea_orm::{Condition, Database, DatabaseConnection, Order, entity::prelude::*};
use uuid::Uuid;
use crudcrate::{ApiError, CRUDOperations, CRUDResource, EntityToModels};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "products")]
#[crudcrate(
    api_struct = "Product",
    generate_router,
    operations = ProductOperations
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub name: String,

    pub price: i32,
    pub published: bool,
    pub image_s3_key: Option<String>,

    #[crudcrate(exclude(create, update))]
    pub view_count: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub struct ProductOperations;

#[async_trait]
impl CRUDOperations for ProductOperations {
    type Resource = Product;

    // LEVEL 1: Lifecycle Hooks

    /// Validation before creation
    async fn before_create(
        &self,
        _db: &DatabaseConnection,
        data: &ProductCreate,
    ) -> Result<(), ApiError> {
        if data.price <= 0 {
            return Err(ApiError::bad_request("Price must be positive"));
        }
        if data.name.trim().is_empty() {
            return Err(ApiError::bad_request("Name cannot be empty"));
        }
        Ok(())
    }

    /// Actions after creation (logging, notifications, etc.)
    async fn after_create(
        &self,
        _db: &DatabaseConnection,
        entity: &mut Product,
    ) -> Result<(), ApiError> {
        // Send notification, trigger webhook, log event, etc.
        println!("✓ Created product: {}", entity.name);
        Ok(())
    }

    /// Enrich data after fetching
    async fn after_get_one(
        &self,
        _db: &DatabaseConnection,
        entity: &mut Product,
    ) -> Result<(), ApiError> {
        // Populate computed fields, fetch related data, etc.
        entity.view_count = 42; // In real code: fetch from analytics
        Ok(())
    }

    /// Permission checks before deletion
    async fn before_delete(&self, _db: &DatabaseConnection, id: Uuid) -> Result<(), ApiError> {
        // Check user permissions, validate business rules, etc.
        // if !current_user.can_delete() { return Err(ApiError::forbidden(...)) }
        println!("Deleting product {id}");
        Ok(())
    }

    // LEVEL 2: Core Method Overrides

    /// Custom fetch_all - add default filters
    async fn fetch_all(
        &self,
        db: &DatabaseConnection,
        condition: &Condition,
        order_column: <Self::Resource as CRUDResource>::ColumnType,
        order_direction: Order,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<<Self::Resource as CRUDResource>::ListModel>, ApiError> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

        // Add published=true filter by default
        let mut custom_condition = condition.clone();
        custom_condition = custom_condition.add(Column::Published.eq(true));

        let models = <Self::Resource as CRUDResource>::EntityType::find()
            .filter(custom_condition)
            .order_by(order_column, order_direction)
            .offset(offset)
            .limit(limit)
            .all(db)
            .await?;

        Ok(models
            .into_iter()
            .map(|model| <Self::Resource as CRUDResource>::ListModel::from(Self::Resource::from(model)))
            .collect())
    }

    // LEVEL 3: Full Operation Overrides

    /// Complete delete override with external cleanup
    async fn delete(&self, db: &DatabaseConnection, id: Uuid) -> Result<Uuid, ApiError> {
        // Multi-step operation: fetch, cleanup external resources, delete
        let product = self.fetch_one(db, id).await?;

        if let Some(s3_key) = &product.image_s3_key {
            delete_from_s3(s3_key).await
                .map_err(|e| ApiError::internal(format!("S3 cleanup failed: {e}"), None))?;
        }

        self.perform_delete(db, id).await
    }
}

// Simulated external service
async fn delete_from_s3(s3_key: &str) -> Result<(), String> {
    println!("  Deleting S3: {s3_key}");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::connect("sqlite::memory:").await?;
    create_schema(&db).await?;

    let _app = Router::new()
        .nest("/products", Product::router(&db).into())
        .route("/", axum::routing::get(|| async { "CRUD Operations Example" }));

    // Demo the hooks
    demo(&db).await?;

    println!("\n✅ All operations complete!");
    println!("\n📚 What you can customize:");
    println!("  • Hooks: before_create, after_create, before_delete, after_get_one, etc.");
    println!("  • Methods: fetch_all, fetch_one (custom queries)");
    println!("  • Full ops: create, update, delete (multi-step operations)");

    Ok(())
}

async fn demo(db: &DatabaseConnection) -> Result<(), ApiError> {
    let product = Product::create(db, ProductCreate {
        name: "Laptop".to_string(),
        price: 1000,
        published: true,
        image_s3_key: Some("images/laptop.jpg".to_string()),
    }).await?;

    let _fetched = Product::get_one(db, product.id).await?; // Triggers after_get_one
    Product::delete(db, product.id).await?; // Triggers S3 cleanup

    Ok(())
}

async fn create_schema(db: &DatabaseConnection) -> Result<(), ApiError> {
    use sea_orm::{ConnectionTrait, Statement};
    db.execute(Statement::from_string(
        sea_orm::DatabaseBackend::Sqlite,
        "CREATE TABLE IF NOT EXISTS products (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            price INTEGER NOT NULL,
            published BOOLEAN NOT NULL DEFAULT 0,
            image_s3_key TEXT,
            view_count INTEGER NOT NULL DEFAULT 0
        )"
    )).await?;
    Ok(())
}
