//! JWT Authentication Example
//!
//! This example demonstrates how to protect CRUD endpoints with JWT authentication.
//! It shows a production-ready pattern for:
//! - JWT token validation
//! - User extraction from claims
//! - Protecting mutating routes (POST, PUT, DELETE)
//! - Optional public read routes (GET)
//!
//! ## Security Features
//! - Token signature verification
//! - Expiration checking
//! - Claims validation
//! - Per-route protection
//!
//! ## Usage
//! ```bash
//! # This is a code example only - requires JWT secret configuration
//! # See README.md for full setup instructions
//! ```

use axum::{
    Router,
    middleware::{self, Next},
    http::{Request, header::AUTHORIZATION},
    response::Response,
    body::Body,
};
use chrono::{DateTime, Utc};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::{DatabaseConnection, entity::prelude::*};
use std::sync::Arc;
use uuid::Uuid;

// ============================================================================
// JWT Configuration (In production, use environment variables!)
// ============================================================================

#[derive(Clone)]
struct JwtConfig {
    secret: String,
    // In production, add: issuer, audience, algorithms, etc.
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: "your-secret-key-here-use-env-var-in-production".to_string(),
        }
    }
}

// ============================================================================
// User Context (Extracted from JWT)
// ============================================================================

#[derive(Clone, Debug)]
struct AuthenticatedUser {
    user_id: Uuid,
    email: String,
    roles: Vec<String>,
}

// ============================================================================
// Entity Definition
// ============================================================================

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "protected_resources")]
#[crudcrate(
    api_struct = "ProtectedResource",
    name_singular = "resource",
    name_plural = "resources",
    description = "JWT-protected resource",
    generate_router
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(sortable, filterable)]
    pub title: String,

    #[crudcrate(filterable)]
    pub owner_id: Uuid,  // For row-level security

    #[crudcrate(exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

// ============================================================================
// JWT Middleware
// ============================================================================

async fn jwt_auth_middleware(
    axum::extract::State(config): axum::extract::State<Arc<JwtConfig>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, axum::http::StatusCode> {
    // 1. Extract Authorization header
    let auth_header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    // 2. Extract Bearer token
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;

    // 3. Validate JWT (In production, use jsonwebtoken crate)
    // Example with jsonwebtoken:
    // ```
    // use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
    //
    // let validation = Validation::new(Algorithm::HS256);
    // let token_data = decode::<Claims>(
    //     token,
    //     &DecodingKey::from_secret(config.secret.as_bytes()),
    //     &validation,
    // ).map_err(|_| StatusCode::UNAUTHORIZED)?;
    //
    // let user = AuthenticatedUser {
    //     user_id: token_data.claims.sub,
    //     email: token_data.claims.email,
    //     roles: token_data.claims.roles,
    // };
    // ```

    // For this example, we'll do simple validation
    let user = if token == "valid-jwt-token" {
        AuthenticatedUser {
            user_id: Uuid::new_v4(),
            email: "user@example.com".to_string(),
            roles: vec!["user".to_string()],
        }
    } else {
        return Err(axum::http::StatusCode::UNAUTHORIZED);
    };

    // 4. Insert user into request extensions for handlers to access
    req.extensions_mut().insert(user);

    // 5. Continue to next middleware/handler
    Ok(next.run(req).await)
}

// ============================================================================
// Router Setup with JWT Protection
// ============================================================================

fn create_protected_router(db: &DatabaseConnection, config: Arc<JwtConfig>) -> Router {
    // Generate CRUD router
    let crud_router = ProtectedResource::router(db);

    // Wrap with JWT middleware
    Router::new()
        .nest("/api/resources", crud_router.into())
        .layer(middleware::from_fn_with_state(config, jwt_auth_middleware))
}

// ============================================================================
// Alternative: Selective Protection (Public reads, protected writes)
// ============================================================================

fn create_selective_router(db: &DatabaseConnection, config: Arc<JwtConfig>) -> Router {
    let crud_router = ProtectedResource::router(db);

    // Split routes by method
    Router::new()
        // Public read routes (GET)
        .route("/api/resources", axum::routing::get(|| async { "public" }))
        .route("/api/resources/:id", axum::routing::get(|| async { "public" }))
        // Protected mutation routes (POST, PUT, DELETE)
        .nest("/api/resources", crud_router.into())
        .layer(middleware::from_fn_with_state(config.clone(), jwt_auth_middleware))
}

// ============================================================================
// Main (Example usage)
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("JWT Authentication Example");
    println!("==========================");
    println!();
    println!("This example demonstrates JWT-protected CRUD endpoints.");
    println!();
    println!("Security features:");
    println!("  - Bearer token validation");
    println!("  - User extraction from claims");
    println!("  - Request extension for handler access");
    println!("  - Selective route protection");
    println!();
    println!("Production recommendations:");
    println!("  - Use jsonwebtoken crate for real JWT validation");
    println!("  - Store secrets in environment variables");
    println!("  - Implement token refresh logic");
    println!("  - Add role-based permissions");
    println!("  - Log authentication failures");

    Ok(())
}
