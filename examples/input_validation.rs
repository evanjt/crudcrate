//! Input Validation Example
//!
//! This example demonstrates comprehensive input validation for CRUD endpoints.
//! Shows production-ready patterns for:
//! - Field-level validation (length, format, ranges)
//! - Custom business logic validation
//! - Proper error responses with user-friendly messages
//! - Integration with create/update operations
//!
//! ## Validation Strategies
//! - Pre-handler validation middleware
//! - Manual validation in handlers
//! - Database constraint validation
//! - Cross-field validation
//!
//! ## Usage
//! ```bash
//! # This is a code example demonstrating validation patterns
//! # See README.md for integration with validator crate or garde
//! ```

use axum::{
    Router,
    response::{Response, IntoResponse},
};
use chrono::{DateTime, Utc};
use crudcrate::{CRUDResource, EntityToModels, MergeIntoActiveModel};
use sea_orm::{DatabaseConnection, entity::prelude::*};
use std::sync::Arc;
use uuid::Uuid;

// ============================================================================
// Validation Error Types
// ============================================================================

#[derive(Debug, serde::Serialize)]
struct ValidationError {
    field: String,
    message: String,
}

#[derive(Debug, serde::Serialize)]
struct ValidationErrorResponse {
    error: String,
    details: Vec<ValidationError>,
}

impl IntoResponse for ValidationErrorResponse {
    fn into_response(self) -> Response {
        (axum::http::StatusCode::BAD_REQUEST, axum::Json(self)).into_response()
    }
}

// ============================================================================
// Entity with Validation Requirements
// ============================================================================

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "products")]
#[crudcrate(
    api_struct = "Product",
    name_singular = "product",
    name_plural = "products",
    description = "Product with validated fields",
    generate_router
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    /// Product name: 3-100 characters, non-empty
    #[crudcrate(sortable, filterable)]
    pub name: String,

    /// Product description: optional, max 1000 characters
    pub description: Option<String>,

    /// Price in cents: must be positive, max 10 million
    #[crudcrate(sortable, filterable)]
    pub price_cents: i32,

    /// Stock quantity: must be >= 0
    #[crudcrate(sortable, filterable)]
    pub stock: i32,

    /// SKU: must be alphanumeric, 6-20 characters, unique
    #[crudcrate(filterable)]
    pub sku: String,

    /// Email for notifications: must be valid email format
    pub notification_email: Option<String>,

    #[crudcrate(exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,

    #[crudcrate(exclude(create, update), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

// ============================================================================
// Validation Functions
// ============================================================================

/// Validates product name
fn validate_name(name: &str) -> Result<(), ValidationError> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        return Err(ValidationError {
            field: "name".to_string(),
            message: "Name cannot be empty".to_string(),
        });
    }

    if trimmed.len() < 3 {
        return Err(ValidationError {
            field: "name".to_string(),
            message: "Name must be at least 3 characters long".to_string(),
        });
    }

    if trimmed.len() > 100 {
        return Err(ValidationError {
            field: "name".to_string(),
            message: "Name cannot exceed 100 characters".to_string(),
        });
    }

    Ok(())
}

/// Validates description length
fn validate_description(description: &Option<String>) -> Result<(), ValidationError> {
    if let Some(desc) = description {
        if desc.len() > 1000 {
            return Err(ValidationError {
                field: "description".to_string(),
                message: "Description cannot exceed 1000 characters".to_string(),
            });
        }
    }
    Ok(())
}

/// Validates price is positive and reasonable
fn validate_price(price_cents: i32) -> Result<(), ValidationError> {
    if price_cents <= 0 {
        return Err(ValidationError {
            field: "price_cents".to_string(),
            message: "Price must be greater than 0".to_string(),
        });
    }

    if price_cents > 10_000_000 {
        return Err(ValidationError {
            field: "price_cents".to_string(),
            message: "Price cannot exceed $100,000".to_string(),
        });
    }

    Ok(())
}

/// Validates stock quantity is non-negative
fn validate_stock(stock: i32) -> Result<(), ValidationError> {
    if stock < 0 {
        return Err(ValidationError {
            field: "stock".to_string(),
            message: "Stock cannot be negative".to_string(),
        });
    }

    Ok(())
}

/// Validates SKU format (alphanumeric, 6-20 chars)
fn validate_sku(sku: &str) -> Result<(), ValidationError> {
    if sku.len() < 6 || sku.len() > 20 {
        return Err(ValidationError {
            field: "sku".to_string(),
            message: "SKU must be 6-20 characters long".to_string(),
        });
    }

    if !sku.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(ValidationError {
            field: "sku".to_string(),
            message: "SKU must contain only alphanumeric characters, hyphens, or underscores".to_string(),
        });
    }

    Ok(())
}

/// Validates email format (basic check)
fn validate_email(email: &Option<String>) -> Result<(), ValidationError> {
    if let Some(email_str) = email {
        if !email_str.contains('@') || !email_str.contains('.') {
            return Err(ValidationError {
                field: "notification_email".to_string(),
                message: "Invalid email format".to_string(),
            });
        }

        if email_str.len() > 255 {
            return Err(ValidationError {
                field: "notification_email".to_string(),
                message: "Email cannot exceed 255 characters".to_string(),
            });
        }
    }
    Ok(())
}

/// Validates entire ProductCreate payload
fn validate_product_create(payload: &ProductCreate) -> Result<(), ValidationErrorResponse> {
    let mut errors = Vec::new();

    if let Err(e) = validate_name(&payload.name) {
        errors.push(e);
    }

    if let Err(e) = validate_description(&payload.description) {
        errors.push(e);
    }

    if let Err(e) = validate_price(payload.price_cents) {
        errors.push(e);
    }

    if let Err(e) = validate_stock(payload.stock) {
        errors.push(e);
    }

    if let Err(e) = validate_sku(&payload.sku) {
        errors.push(e);
    }

    if let Err(e) = validate_email(&payload.notification_email) {
        errors.push(e);
    }

    if !errors.is_empty() {
        return Err(ValidationErrorResponse {
            error: "Validation failed".to_string(),
            details: errors,
        });
    }

    Ok(())
}

/// Validates ProductUpdate payload (all fields optional)
fn validate_product_update(payload: &ProductUpdate) -> Result<(), ValidationErrorResponse> {
    let mut errors = Vec::new();

    // Validate name if present
    if let Some(name_option) = &payload.name {
        if let Some(name_str) = name_option {
            if let Err(e) = validate_name(name_str) {
                errors.push(e);
            }
        }
    }

    // Validate description if present
    if let Some(desc_option) = &payload.description {
        if let Err(e) = validate_description(desc_option) {
            errors.push(e);
        }
    }

    // Validate price if present
    if let Some(price_option) = &payload.price_cents {
        if let Some(price) = price_option {
            if let Err(e) = validate_price(*price) {
                errors.push(e);
            }
        }
    }

    // Validate stock if present
    if let Some(stock_option) = &payload.stock {
        if let Some(stock) = stock_option {
            if let Err(e) = validate_stock(*stock) {
                errors.push(e);
            }
        }
    }

    // Validate SKU if present
    if let Some(sku_option) = &payload.sku {
        if let Some(sku_str) = sku_option {
            if let Err(e) = validate_sku(sku_str) {
                errors.push(e);
            }
        }
    }

    // Validate email if present
    if let Some(email_option) = &payload.notification_email {
        if let Err(e) = validate_email(email_option) {
            errors.push(e);
        }
    }

    if !errors.is_empty() {
        return Err(ValidationErrorResponse {
            error: "Validation failed".to_string(),
            details: errors,
        });
    }

    Ok(())
}

// ============================================================================
// Custom CRUD Handlers with Validation
// ============================================================================

/// POST /products - Create with validation
async fn create_product(
    axum::extract::State(db): axum::extract::State<Arc<DatabaseConnection>>,
    axum::Json(payload): axum::Json<ProductCreate>,
) -> Result<axum::Json<Product>, ValidationErrorResponse> {
    // Validate input
    validate_product_create(&payload)?;

    // Create product (in production, handle DB errors properly)
    let mut active_model: ActiveModel = payload.into();
    active_model.id = sea_orm::ActiveValue::Set(Uuid::new_v4());
    active_model.created_at = sea_orm::ActiveValue::Set(Utc::now());
    active_model.updated_at = sea_orm::ActiveValue::Set(Utc::now());

    let product = active_model
        .insert(db.as_ref())
        .await
        .map_err(|e| ValidationErrorResponse {
            error: "Database error".to_string(),
            details: vec![ValidationError {
                field: "database".to_string(),
                message: format!("Failed to create product: {}", e),
            }],
        })?;

    Ok(axum::Json(Product::from(product)))
}

/// PUT /products/:id - Update with validation
async fn update_product(
    axum::extract::State(db): axum::extract::State<Arc<DatabaseConnection>>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    axum::Json(payload): axum::Json<ProductUpdate>,
) -> Result<axum::Json<Product>, ValidationErrorResponse> {
    // Validate input
    validate_product_update(&payload)?;

    // Fetch existing product
    let existing = Entity::find_by_id(id)
        .one(db.as_ref())
        .await
        .map_err(|e| ValidationErrorResponse {
            error: "Database error".to_string(),
            details: vec![ValidationError {
                field: "database".to_string(),
                message: format!("Failed to fetch product: {}", e),
            }],
        })?
        .ok_or_else(|| ValidationErrorResponse {
            error: "Not found".to_string(),
            details: vec![ValidationError {
                field: "id".to_string(),
                message: "Product not found".to_string(),
            }],
        })?;

    // Update product
    let active_model: ActiveModel = existing.into();
    let updated_model = payload
        .merge_into_activemodel(active_model)
        .map_err(|e| ValidationErrorResponse {
            error: "Merge error".to_string(),
            details: vec![ValidationError {
                field: "payload".to_string(),
                message: format!("Failed to merge update: {}", e),
            }],
        })?;

    // Set updated_at timestamp
    let mut updated_model = updated_model;
    updated_model.updated_at = sea_orm::ActiveValue::Set(Utc::now());

    let updated = updated_model
        .update(db.as_ref())
        .await
        .map_err(|e| ValidationErrorResponse {
            error: "Database error".to_string(),
            details: vec![ValidationError {
                field: "database".to_string(),
                message: format!("Failed to update product: {}", e),
            }],
        })?;

    Ok(axum::Json(Product::from(updated)))
}

// ============================================================================
// Router with Validated Endpoints
// ============================================================================

fn create_validated_router(db: Arc<DatabaseConnection>) -> Router {
    // Create custom validated routes
    let custom_routes = Router::new()
        .route("/products", axum::routing::post(create_product))
        .route("/products/:id", axum::routing::put(update_product))
        .with_state(db.clone());

    // Get the generated CRUD router (includes GET, DELETE, and list routes)
    let generated_router = Product::router(db.as_ref());
    let (generated_router, _openapi) = generated_router.split_for_parts();

    // Merge the custom routes with generated routes
    custom_routes.merge(generated_router)
}

// ============================================================================
// Main (Example usage)
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Input Validation Example");
    println!("========================");
    println!();
    println!("Validation patterns demonstrated:");
    println!("  ✓ Field-level validation (length, format, range)");
    println!("  ✓ Custom business logic validation");
    println!("  ✓ User-friendly error messages");
    println!("  ✓ Multiple validation errors in single response");
    println!("  ✓ Different rules for create vs update");
    println!();
    println!("Validation rules:");
    println!("  - name: 3-100 characters, non-empty");
    println!("  - description: max 1000 characters (optional)");
    println!("  - price_cents: 1 to 10,000,000");
    println!("  - stock: >= 0");
    println!("  - sku: 6-20 alphanumeric characters");
    println!("  - notification_email: valid email format (optional)");
    println!();
    println!("Production recommendations:");
    println!("  - Use 'validator' or 'garde' crate for declarative validation");
    println!("  - Validate at multiple layers (input, business logic, database)");
    println!("  - Return specific field-level errors for better UX");
    println!("  - Log validation failures for security monitoring");
    println!("  - Sanitize input to prevent injection attacks");
    println!("  - Consider using JSON Schema for API contract validation");

    Ok(())
}
