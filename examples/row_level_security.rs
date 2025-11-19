//! Row-Level Security (RLS) Example
//!
//! This example demonstrates how to implement row-level security to ensure users
//! can only access their own data. This is critical for multi-tenant applications.
//!
//! ## Security Patterns
//! - Automatic filtering by user ownership
//! - Preventing cross-user data access
//! - Secure create/update operations
//! - Admin bypass capabilities
//!
//! ## Implementation
//! Uses Axum middleware to inject user context and custom CRUD handlers
//! to enforce ownership checks.

use axum::{
    Router,
    middleware::{self, Next},
    http::Request,
    response::Response,
    body::Body,
};
use chrono::{DateTime, Utc};
use crudcrate::{CRUDResource, EntityToModels, MergeIntoActiveModel};
use sea_orm::{
    DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait,
    entity::prelude::*,
};
use std::sync::Arc;
use uuid::Uuid;

// ============================================================================
// User Context (from authentication)
// ============================================================================

#[derive(Clone, Debug)]
struct CurrentUser {
    id: Uuid,
    email: String,
    is_admin: bool,
}

// ============================================================================
// Multi-tenant Entity (with owner_id)
// ============================================================================

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "user_documents")]
#[crudcrate(
    api_struct = "Document",
    name_singular = "document",
    name_plural = "documents",
    description = "User-owned documents with RLS",
    generate_router
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(sortable, filterable)]
    pub title: String,

    pub content: String,

    /// Owner of this document - CRITICAL for RLS
    #[crudcrate(filterable, exclude(create, update))]
    pub owner_id: Uuid,

    #[crudcrate(exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

// ============================================================================
// Row-Level Security Middleware
// ============================================================================

/// Injects current user into request extensions
async fn inject_user_middleware(
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, axum::http::StatusCode> {
    // In production, extract from JWT or session
    let user = CurrentUser {
        id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
        email: "user@example.com".to_string(),
        is_admin: false,
    };

    req.extensions_mut().insert(user);
    Ok(next.run(req).await)
}

// ============================================================================
// Custom RLS-Aware CRUD Handlers
// ============================================================================

/// GET /documents - Only returns current user's documents
async fn get_user_documents(
    axum::extract::State(db): axum::extract::State<Arc<DatabaseConnection>>,
    axum::Extension(user): axum::Extension<CurrentUser>,
) -> Result<axum::Json<Vec<Document>>, axum::http::StatusCode> {
    let documents = if user.is_admin {
        // Admins see everything
        Entity::find()
            .all(db.as_ref())
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        // Users only see their own documents
        Entity::find()
            .filter(Column::OwnerId.eq(user.id))
            .all(db.as_ref())
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
    };

    let api_documents: Vec<Document> = documents
        .into_iter()
        .map(Document::from)
        .collect();

    Ok(axum::Json(api_documents))
}

/// GET /documents/:id - Only if user owns it
async fn get_user_document(
    axum::extract::State(db): axum::extract::State<Arc<DatabaseConnection>>,
    axum::Extension(user): axum::Extension<CurrentUser>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<axum::Json<Document>, axum::http::StatusCode> {
    let document = Entity::find_by_id(id)
        .one(db.as_ref())
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(axum::http::StatusCode::NOT_FOUND)?;

    // RLS check: Verify ownership (or admin)
    if !user.is_admin && document.owner_id != user.id {
        return Err(axum::http::StatusCode::NOT_FOUND); // Don't leak existence
    }

    Ok(axum::Json(Document::from(document)))
}

/// POST /documents - Automatically set owner_id
async fn create_user_document(
    axum::extract::State(db): axum::extract::State<Arc<DatabaseConnection>>,
    axum::Extension(user): axum::Extension<CurrentUser>,
    axum::Json(payload): axum::Json<DocumentCreate>,
) -> Result<axum::Json<Document>, axum::http::StatusCode> {
    // Critical: Force owner_id to current user (prevent impersonation)
    let mut active_model: ActiveModel = payload.into();
    active_model.id = sea_orm::ActiveValue::Set(Uuid::new_v4());
    active_model.owner_id = sea_orm::ActiveValue::Set(user.id);
    active_model.created_at = sea_orm::ActiveValue::Set(Utc::now());

    let document = active_model
        .insert(db.as_ref())
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(axum::Json(Document::from(document)))
}

/// PUT /documents/:id - Only if user owns it
async fn update_user_document(
    axum::extract::State(db): axum::extract::State<Arc<DatabaseConnection>>,
    axum::Extension(user): axum::Extension<CurrentUser>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    axum::Json(payload): axum::Json<DocumentUpdate>,
) -> Result<axum::Json<Document>, axum::http::StatusCode> {
    // 1. Fetch existing document
    let existing = Entity::find_by_id(id)
        .one(db.as_ref())
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(axum::http::StatusCode::NOT_FOUND)?;

    // 2. RLS check: Verify ownership
    if !user.is_admin && existing.owner_id != user.id {
        return Err(axum::http::StatusCode::NOT_FOUND);
    }

    // 3. Update (owner_id cannot be changed)
    let mut active_model: ActiveModel = existing.into();
    let updated_model = payload.merge_into_activemodel(active_model)
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

    let updated = updated_model
        .update(db.as_ref())
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(axum::Json(Document::from(updated)))
}

/// DELETE /documents/:id - Only if user owns it
async fn delete_user_document(
    axum::extract::State(db): axum::extract::State<Arc<DatabaseConnection>>,
    axum::Extension(user): axum::Extension<CurrentUser>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<axum::http::StatusCode, axum::http::StatusCode> {
    // 1. Fetch to check ownership
    let document = Entity::find_by_id(id)
        .one(db.as_ref())
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(axum::http::StatusCode::NOT_FOUND)?;

    // 2. RLS check
    if !user.is_admin && document.owner_id != user.id {
        return Err(axum::http::StatusCode::NOT_FOUND);
    }

    // 3. Delete
    Entity::delete_by_id(id)
        .exec(db.as_ref())
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ============================================================================
// Router with RLS Enforcement
// ============================================================================

fn create_rls_router(db: Arc<DatabaseConnection>) -> Router {
    Router::new()
        .route("/documents", axum::routing::get(get_user_documents))
        .route("/documents", axum::routing::post(create_user_document))
        .route("/documents/:id", axum::routing::get(get_user_document))
        .route("/documents/:id", axum::routing::put(update_user_document))
        .route("/documents/:id", axum::routing::delete(delete_user_document))
        .layer(middleware::from_fn(inject_user_middleware))
        .with_state(db)
}

// ============================================================================
// Main (Example usage)
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Row-Level Security Example");
    println!("==========================");
    println!();
    println!("Security features demonstrated:");
    println!("  ✓ Automatic owner_id injection on create");
    println!("  ✓ Filtering by ownership on list");
    println!("  ✓ Ownership verification on get/update/delete");
    println!("  ✓ Admin bypass capability");
    println!("  ✓ No data leakage (404 instead of 403)");
    println!();
    println!("Key principles:");
    println!("  1. Never trust client-provided owner_id");
    println!("  2. Always filter queries by current user");
    println!("  3. Verify ownership before mutations");
    println!("  4. Return 404 for forbidden resources (don't leak existence)");
    println!("  5. Use database constraints as second layer of defense");

    Ok(())
}
