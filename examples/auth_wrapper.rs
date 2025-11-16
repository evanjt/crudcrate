//! Authentication Wrapper Pattern
//!
//! This example demonstrates how to wrap generated CRUD routers with authentication
//! middleware, based on the production pattern used in spice-api.
//!
//! ## Pattern
//!
//! Generated routers can be wrapped with Keycloak, JWT, or any other auth middleware
//! while keeping mutating routes (POST, PUT, DELETE) protected and read-only routes
//! (GET) optionally public.

use axum::{
    Router,
    middleware::{self, Next},
    http::{Request, StatusCode},
    response::Response,
};
use chrono::{DateTime, Utc};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::{Database, DatabaseConnection, entity::prelude::*};
use uuid::Uuid;

// ============================================================================
// Entity Definition
// ============================================================================

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "samples")]
#[crudcrate(
    api_struct = "Sample",
    name_singular = "sample",
    name_plural = "samples",
    description = "Protected sample resource",
    generate_router
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(sortable, filterable)]
    pub name: String,

    #[crudcrate(exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

// ============================================================================
// Simple Authentication Middleware (Example - Use Keycloak/JWT in production!)
// ============================================================================

#[derive(Clone)]
struct AuthToken(Option<String>);

async fn auth_middleware<B>(
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    // In production, extract from Authorization header and validate JWT
    let token = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .map(String::from);

    if token.as_deref() == Some("Bearer valid-token") {
        req.extensions_mut().insert(AuthToken(token));
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

// ============================================================================
// Router Wrapper Function (Production Pattern)
// ============================================================================

pub fn protected_sample_router(db: &DatabaseConnection) -> Router {
    // Get the generated CRUD router
    let crud_router = Sample::router(db);

    // Wrap with authentication middleware
    // In production, use KeycloakAuthLayer or similar
    crud_router.layer(middleware::from_fn(auth_middleware))
}

// ============================================================================
// Example Usage
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔐 Auth Wrapper Pattern Example\n");

    let db = Database::connect("sqlite::memory:").await?;

    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        r#"CREATE TABLE samples (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL
        )"#.to_owned(),
    )).await?;

    // Create app with protected routes
    let app = Router::new()
        .nest("/api/samples", protected_sample_router(&db));

    println!("✅ Router created with authentication wrapper");
    println!("\n💡 Production Pattern:");
    println!("   • Use Keycloak/JWT middleware instead of simple auth");
    println!("   • Separate read-only and mutating routes if needed");
    println!("   • Add role-based access control (RBAC)");
    println!("\n📝 Example from spice-api:");
    println!("   let mut mutating_router = Sample::router(&db);");
    println!("   mutating_router = mutating_router.layer(");
    println!("       KeycloakAuthLayer::<Role>::builder()");
    println!("           .required_roles(vec![Role::Administrator])");
    println!("           .build()");
    println!("   );");

    Ok(())
}
