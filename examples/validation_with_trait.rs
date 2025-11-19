//! Validation with Validatable Trait Example
//!
//! This example demonstrates how to use crudcrate's built-in validation module
//! to add type-safe validation to CRUD operations.
//!
//! ## Features Demonstrated
//! - Using the `Validatable` trait
//! - Built-in validator helpers (length, range, email, required)
//! - Integration with custom handlers
//! - Validation error responses
//!
//! ## Comparison with Manual Validation
//! This example uses the validation module for cleaner, more maintainable code
//! compared to manual validation functions.

use axum::{
    Router,
    response::{Response, IntoResponse},
};
use chrono::{DateTime, Utc};
use crudcrate::{
    CRUDResource, EntityToModels, MergeIntoActiveModel,
    validation::{Validatable, ValidationError, validators},
};
use sea_orm::{DatabaseConnection, entity::prelude::*};
use std::sync::Arc;
use uuid::Uuid;

// ============================================================================
// Entity Definition
// ============================================================================

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "products")]
#[crudcrate(
    api_struct = "Product",
    name_singular = "product",
    name_plural = "products",
    description = "Product with validation",
    generate_router
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(sortable, filterable)]
    pub name: String,

    pub description: Option<String>,

    #[crudcrate(sortable, filterable)]
    pub price_cents: i32,

    #[crudcrate(sortable, filterable)]
    pub stock: i32,

    #[crudcrate(filterable)]
    pub sku: String,

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
// Validation Implementation for ProductCreate
// ============================================================================

impl Validatable for ProductCreate {
    fn validate(&self) -> Result<(), ValidationError> {
        // Validate name (required, 3-100 characters)
        validators::validate_required("name", &self.name)?;
        validators::validate_length("name", &self.name, Some(3), Some(100))?;

        // Validate description (max 1000 characters)
        if let Some(ref desc) = self.description {
            validators::validate_length("description", desc, None, Some(1000))?;
        }

        // Validate price (positive, max $100,000)
        validators::validate_range("price_cents", self.price_cents, Some(1), Some(10_000_000))?;

        // Validate stock (non-negative)
        validators::validate_range("stock", self.stock, Some(0), None)?;

        // Validate SKU (6-20 alphanumeric characters)
        validators::validate_length("sku", &self.sku, Some(6), Some(20))?;
        if !self.sku.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(ValidationError::new(
                "sku",
                "SKU must contain only alphanumeric characters, hyphens, or underscores",
            ));
        }

        // Validate email format
        if let Some(ref email) = self.notification_email {
            validators::validate_email("notification_email", email)?;
        }

        Ok(())
    }
}

// ============================================================================
// Validation Implementation for ProductUpdate
// ============================================================================

impl Validatable for ProductUpdate {
    fn validate(&self) -> Result<(), ValidationError> {
        // Validate name if present
        if let Some(name_option) = &self.name {
            if let Some(name_str) = name_option {
                validators::validate_required("name", name_str)?;
                validators::validate_length("name", name_str, Some(3), Some(100))?;
            }
        }

        // Validate description if present
        if let Some(desc_option) = &self.description {
            if let Some(desc_str) = desc_option {
                validators::validate_length("description", desc_str, None, Some(1000))?;
            }
        }

        // Validate price if present
        if let Some(price_option) = &self.price_cents {
            if let Some(price) = price_option {
                validators::validate_range("price_cents", *price, Some(1), Some(10_000_000))?;
            }
        }

        // Validate stock if present
        if let Some(stock_option) = &self.stock {
            if let Some(stock) = stock_option {
                validators::validate_range("stock", *stock, Some(0), None)?;
            }
        }

        // Validate SKU if present
        if let Some(sku_option) = &self.sku {
            if let Some(sku_str) = sku_option {
                validators::validate_length("sku", sku_str, Some(6), Some(20))?;
                if !sku_str.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
                    return Err(ValidationError::new(
                        "sku",
                        "SKU must contain only alphanumeric characters, hyphens, or underscores",
                    ));
                }
            }
        }

        // Validate email if present
        if let Some(email_option) = &self.notification_email {
            if let Some(email_str) = email_option {
                validators::validate_email("notification_email", email_str)?;
            }
        }

        Ok(())
    }
}

// ============================================================================
// Validation Error Response
// ============================================================================

#[derive(Debug, serde::Serialize)]
struct ValidationErrorResponse {
    error: String,
    field: String,
    message: String,
}

impl From<ValidationError> for ValidationErrorResponse {
    fn from(err: ValidationError) -> Self {
        Self {
            error: "Validation failed".to_string(),
            field: err.field,
            message: err.message,
        }
    }
}

impl IntoResponse for ValidationErrorResponse {
    fn into_response(self) -> Response {
        (axum::http::StatusCode::BAD_REQUEST, axum::Json(self)).into_response()
    }
}

// ============================================================================
// Custom Handlers with Validation
// ============================================================================

/// POST /products - Create with automatic validation
async fn create_product(
    axum::extract::State(db): axum::extract::State<Arc<DatabaseConnection>>,
    axum::Json(payload): axum::Json<ProductCreate>,
) -> Result<axum::Json<Product>, ValidationErrorResponse> {
    // Validate using the Validatable trait
    payload.validate().map_err(ValidationErrorResponse::from)?;

    // If validation passes, create the product
    let mut active_model: ActiveModel = payload.into();
    active_model.id = sea_orm::ActiveValue::Set(Uuid::new_v4());
    active_model.created_at = sea_orm::ActiveValue::Set(Utc::now());
    active_model.updated_at = sea_orm::ActiveValue::Set(Utc::now());

    let product = active_model
        .insert(db.as_ref())
        .await
        .map_err(|e| ValidationError::new("database", format!("Failed to create: {}", e)))
        .map_err(ValidationErrorResponse::from)?;

    Ok(axum::Json(Product::from(product)))
}

/// PUT /products/:id - Update with automatic validation
async fn update_product(
    axum::extract::State(db): axum::extract::State<Arc<DatabaseConnection>>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    axum::Json(payload): axum::Json<ProductUpdate>,
) -> Result<axum::Json<Product>, ValidationErrorResponse> {
    // Validate using the Validatable trait
    payload.validate().map_err(ValidationErrorResponse::from)?;

    // Fetch existing product
    let existing = Entity::find_by_id(id)
        .one(db.as_ref())
        .await
        .map_err(|e| ValidationError::new("database", format!("Database error: {}", e)))
        .map_err(ValidationErrorResponse::from)?
        .ok_or_else(|| ValidationError::new("id", "Product not found"))
        .map_err(ValidationErrorResponse::from)?;

    // Update with validated payload
    let active_model: ActiveModel = existing.into();
    let updated_model = payload
        .merge_into_activemodel(active_model)
        .map_err(|e| ValidationError::new("update", format!("Merge failed: {}", e)))
        .map_err(ValidationErrorResponse::from)?;

    let mut updated_model = updated_model;
    updated_model.updated_at = sea_orm::ActiveValue::Set(Utc::now());

    let updated = updated_model
        .update(db.as_ref())
        .await
        .map_err(|e| ValidationError::new("database", format!("Update failed: {}", e)))
        .map_err(ValidationErrorResponse::from)?;

    Ok(axum::Json(Product::from(updated)))
}

// ============================================================================
// Router Setup
// ============================================================================

fn create_validated_router(db: Arc<DatabaseConnection>) -> Router {
    // Create custom routes with validation
    let custom_routes = Router::new()
        .route("/products", axum::routing::post(create_product))
        .route("/products/:id", axum::routing::put(update_product))
        .with_state(db.clone());

    // Get generated routes (GET, DELETE)
    let generated_router = Product::router(db.as_ref());
    let (generated_router, _openapi) = generated_router.split_for_parts();

    // Merge custom and generated routes
    custom_routes.merge(generated_router)
}

// ============================================================================
// Main (Example Usage)
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Validation with Validatable Trait Example");
    println!("=========================================");
    println!();
    println!("Features demonstrated:");
    println!("  ✓ Validatable trait implementation");
    println!("  ✓ Built-in validator helpers");
    println!("  ✓ Automatic validation in handlers");
    println!("  ✓ Type-safe validation errors");
    println!("  ✓ Clean, maintainable code");
    println!();
    println!("Built-in validators:");
    println!("  - validate_required(field, value)");
    println!("  - validate_length(field, value, min, max)");
    println!("  - validate_range(field, value, min, max)");
    println!("  - validate_email(field, value)");
    println!();
    println!("Validation rules:");
    println!("  - name: required, 3-100 characters");
    println!("  - description: optional, max 1000 characters");
    println!("  - price_cents: 1 to 10,000,000");
    println!("  - stock: >= 0");
    println!("  - sku: 6-20 alphanumeric characters");
    println!("  - notification_email: valid email format");
    println!();
    println!("Example error response:");
    println!(r#"{{
  "error": "Validation failed",
  "field": "name",
  "message": "Name must be at least 3 characters"
}}"#);
    println!();
    println!("Benefits over manual validation:");
    println!("  - Reusable validator functions");
    println!("  - Consistent error format");
    println!("  - Type-safe validation");
    println!("  - Easier to test");
    println!("  - Less boilerplate code");

    Ok(())
}
