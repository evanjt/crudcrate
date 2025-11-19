//! # Error Handling Example
//!
//! Demonstrates how CrudCrate's ApiError system provides:
//! - Sanitized user-facing error messages
//! - Proper HTTP status codes
//! - Internal error logging (via tracing)
//! - Prevention of sensitive data leakage
//!
//! Run with: `cargo run --example error_handling`

use async_trait::async_trait;
use axum::Router;
use crudcrate::{CRUDOperations, CRUDResource, EntityToModels};
use sea_orm::{Database, DatabaseConnection, entity::prelude::*};
use uuid::Uuid;

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

    #[crudcrate(filterable)]
    pub price: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

//
// Operations with error handling demonstrations
//

pub struct ProductOperations;

#[async_trait]
impl CRUDOperations for ProductOperations {
    type Resource = Product;

    /// Demonstrate validation error (400 Bad Request)
    async fn before_create(
        &self,
        _db: &DatabaseConnection,
        data: &ProductCreate,
    ) -> Result<(), DbErr> {
        // Validation that returns user-friendly error
        if data.price <= 0 {
            return Err(DbErr::Custom(
                "Invalid price: must be greater than 0".to_string(),
            ));
        }

        if data.name.trim().is_empty() {
            return Err(DbErr::Custom("Invalid name: cannot be empty".to_string()));
        }

        Ok(())
    }

    /// Demonstrate permission error (403 Forbidden)
    async fn before_delete(&self, _db: &DatabaseConnection, id: Uuid) -> Result<(), DbErr> {
        // Simulate permission check
        // In real code, you'd check user permissions here
        tracing::info!("Checking delete permission for product {}", id);

        // For demo: prevent deletion (you could return ApiError::forbidden instead)
        if id.to_string().starts_with('a') {
            return Err(DbErr::Custom(
                "Forbidden: You don't have permission to delete this product".to_string(),
            ));
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ============================================================================
    // STEP 1: Enable tracing (OPTIONAL - only if you want error logging)
    // ============================================================================
    //
    // If you DON'T set up tracing, errors are still handled properly but
    // internal details are NOT logged. This is perfect for production if you
    // use external logging/monitoring.

    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .compact()
        .init();

    tracing::info!("🚀 Error Handling Example - Starting server...");

    // Setup database
    let db = Database::connect("sqlite::memory:").await?;
    create_schema(&db).await?;

    // Build router
    let app = Router::new()
        .nest("/products", Product::router(&db).into())
        .route(
            "/",
            axum::routing::get(|| async { "Error Handling Example - see /products" }),
        );

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🌐 Server running on http://localhost:3000");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("📚 Error Handling Demonstrations:\n");

    println!("🔴 1. Validation Error (400 Bad Request)");
    println!("   Try creating a product with invalid price:");
    println!("   curl -X POST http://localhost:3000/products \\");
    println!("     -H 'Content-Type: application/json' \\");
    println!("     -d '{{\"name\": \"Test\", \"price\": -10}}' | jq .");
    println!("   ❌ User sees: \"Invalid price: must be greater than 0\"");
    println!("   📋 Console logs: [TRACE] level details\n");

    println!("🔴 2. Not Found Error (404)");
    println!("   Try getting a non-existent product:");
    println!("   curl -s http://localhost:3000/products/00000000-0000-0000-0000-000000000000 | jq .");
    println!("   ❌ User sees: \"Product not found\"");
    println!("   📋 Console logs: Debug-level error\n");

    println!("🔴 3. Database Error (500 Internal Server Error)");
    println!("   Database errors are NEVER exposed to users!");
    println!("   ❌ User sees: \"A database error occurred\"");
    println!("   📋 Console logs: Full DbErr details (ONLY visible server-side)\n");

    println!("🔴 4. Permission Error (403 Forbidden)");
    println!("   Try deleting a product starting with 'a':");
    println!("   First create one:");
    println!("   ID=$(curl -s -X POST http://localhost:3000/products \\");
    println!("     -H 'Content-Type: application/json' \\");
    println!("     -d '{{\"name\": \"Test\", \"price\": 100}}' | jq -r .id)");
    println!("   curl -X DELETE http://localhost:3000/products/$ID");
    println!("   Note: IDs starting with 'a' are blocked for demo\n");

    println!("✅ 5. Successful Operations");
    println!("   Create a valid product:");
    println!("   curl -X POST http://localhost:3000/products \\");
    println!("     -H 'Content-Type: application/json' \\");
    println!("     -d '{{\"name\": \"Laptop\", \"price\": 1000}}' | jq .");
    println!("   ✅ Returns: Product with ID\n");

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    println!("💡 Key Points:\n");
    println!("  • User-facing errors are clean and sanitized");
    println!("  • HTTP status codes are appropriate (400, 403, 404, 500)");
    println!("  • Database errors NEVER leak to users");
    println!("  • Internal errors are logged via tracing (if enabled)");
    println!("  • You can disable logging by not calling tracing_subscriber::init()");
    println!("  • Perfect for production: safe public API, detailed server logs\n");

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn create_schema(db: &DatabaseConnection) -> Result<(), DbErr> {
    use sea_orm::{ConnectionTrait, Statement};

    db.execute(Statement::from_string(
        sea_orm::DatabaseBackend::Sqlite,
        r#"
        CREATE TABLE IF NOT EXISTS products (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            price INTEGER NOT NULL
        )
        "#,
    ))
    .await?;

    Ok(())
}
