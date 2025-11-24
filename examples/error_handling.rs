//! # Error Handling Example
//!
//! Demonstrates CrudCrate's ApiError system with:
//! - All ApiError constructor methods (bad_request, forbidden, custom, etc.)
//! - Automatic DbErr → ApiError conversion
//! - Sanitized user-facing messages vs internal logging
//! - Proper HTTP status codes (400, 401, 403, 404, 409, 422, 500, etc.)
//! - Custom status codes with internal/external message separation
//!
//! Run with: `cargo run --example error_handling`

use async_trait::async_trait;
use axum::Router;
use crudcrate::{ApiError, CRUDOperations, CRUDResource, EntityToModels};
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
// Operations demonstrating ALL ApiError patterns
//

pub struct ProductOperations;

#[async_trait]
impl CRUDOperations for ProductOperations {
    type Resource = Product;

    /// Example 1: ApiError::bad_request() - 400 Bad Request
    /// Used for validation errors and malformed input
    async fn before_create(
        &self,
        _db: &DatabaseConnection,
        data: &ProductCreate,
    ) -> Result<(), ApiError> {
        tracing::info!("Validating product creation...");

        // Simple validation - bad request
        if data.price <= 0 {
            return Err(ApiError::bad_request("Price must be greater than 0"));
        }

        if data.name.trim().is_empty() {
            return Err(ApiError::bad_request("Product name cannot be empty"));
        }

        // Example: Multiple validation errors - 422 Unprocessable Entity
        let mut errors = vec![];
        if data.name.len() < 3 {
            errors.push("Product name must be at least 3 characters".to_string());
        }
        if data.price > 1_000_000 {
            errors.push("Price exceeds maximum allowed value".to_string());
        }
        if !errors.is_empty() {
            return Err(ApiError::validation_failed(errors));
        }

        Ok(())
    }

    /// Example 2: ApiError::forbidden() - 403 Forbidden
    /// Used for permission/authorization failures
    async fn before_delete(&self, _db: &DatabaseConnection, id: Uuid) -> Result<(), ApiError> {
        tracing::info!("Checking delete permission for product {}", id);

        // Simulate permission check
        if id.to_string().starts_with('a') {
            return Err(ApiError::forbidden(
                "You don't have permission to delete this product"
            ));
        }

        Ok(())
    }

    /// Example 3: ApiError::unauthorized() - 401 Unauthorized
    /// Used for authentication failures
    async fn before_update(
        &self,
        _db: &DatabaseConnection,
        id: Uuid,
        _data: &ProductUpdate,
    ) -> Result<(), ApiError> {
        // Simulate authentication check
        if id.to_string().starts_with('b') {
            return Err(ApiError::unauthorized("Authentication required to update products"));
        }

        Ok(())
    }

    /// Example 4: ApiError::conflict() - 409 Conflict
    /// Used for duplicate records or conflicting state
    async fn after_create(
        &self,
        _db: &DatabaseConnection,
        entity: &mut Product,
    ) -> Result<(), ApiError> {
        // Simulate duplicate check (normally done in before_create)
        if entity.name == "DuplicateTest" {
            return Err(ApiError::conflict(
                format!("Product with name '{}' already exists", entity.name)
            ));
        }

        Ok(())
    }

    /// Example 5: ApiError::custom() - Any HTTP status code
    /// Used for custom status codes with internal/external message separation
    async fn before_get_one(
        &self,
        _db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<(), ApiError> {
        // Example: Custom 429 Too Many Requests with internal logging
        if id.to_string().starts_with('c') {
            return Err(ApiError::custom(
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                "Rate limit exceeded. Please try again in 60 seconds",  // User sees this
                Some(format!("Product {} hit rate limit at {}", id, chrono::Utc::now()))  // Logged internally
            ));
        }

        // Example: Custom 503 Service Unavailable
        if id.to_string().starts_with('d') {
            return Err(ApiError::custom(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "Service temporarily unavailable",
                Some("Database connection pool exhausted".to_string())
            ));
        }

        Ok(())
    }

    /// Example 6: Automatic DbErr → ApiError conversion
    /// The ? operator automatically converts DbErr to ApiError!
    async fn fetch_one(
        &self,
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Product, ApiError> {
        use sea_orm::EntityTrait;

        // This returns Result<Model, DbErr>
        // The ? operator automatically converts to ApiError:
        // - DbErr::RecordNotFound → ApiError::NotFound (404)
        // - Other DbErr → ApiError::Database (500, sanitized)
        let model = <Product as CRUDResource>::EntityType::find_by_id(id)
            .one(db)
            .await?  // ← Automatic conversion! No manual handling needed
            .ok_or_else(|| ApiError::not_found("Product", Some(id.to_string())))?;

        Ok(Product::from(model))
    }

    /// Example 7: ApiError::internal() - 500 with internal details
    /// Used for unexpected errors you want to log but not expose
    async fn after_get_one(
        &self,
        _db: &DatabaseConnection,
        entity: &mut Product,
    ) -> Result<(), ApiError> {
        // Simulate an unexpected error
        if entity.name == "ErrorTest" {
            return Err(ApiError::internal(
                "An unexpected error occurred",  // User sees this generic message
                Some(format!("Cache service returned invalid data for product {}", entity.id))  // Logged internally
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
    // internal details are NOT logged. This works well when you
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
    println!("  • Design: sanitized public API responses, detailed server-side logs\n");

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
