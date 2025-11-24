# Project Structure

How to organize a CRUDCrate project as it grows.

## Minimal Structure

For small projects with a few entities:

```
my-api/
├── Cargo.toml
├── src/
│   ├── main.rs           # Server setup and routing
│   ├── user.rs           # User entity + CRUDCrate derives
│   ├── post.rs           # Post entity + CRUDCrate derives
│   └── comment.rs        # Comment entity + CRUDCrate derives
```

**main.rs:**

```rust
mod user;
mod post;
mod comment;

use axum::Router;

#[tokio::main]
async fn main() {
    let db = /* database connection */;

    let app = Router::new()
        .merge(user::user_router())
        .merge(post::post_router())
        .merge(comment::comment_router())
        .layer(Extension(db));

    // ...
}
```

## Medium Projects

For projects with 5-15 entities, group by domain:

```
my-api/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── entities/
│   │   ├── mod.rs
│   │   ├── user.rs
│   │   ├── post.rs
│   │   └── comment.rs
│   ├── routes/
│   │   ├── mod.rs
│   │   └── api.rs        # Combines all entity routers
│   └── config.rs         # Database and app configuration
```

**entities/mod.rs:**

```rust
pub mod user;
pub mod post;
pub mod comment;

pub use user::*;
pub use post::*;
pub use comment::*;
```

**routes/api.rs:**

```rust
use axum::Router;
use crate::entities;

pub fn api_router() -> Router {
    Router::new()
        .nest("/api/v1", v1_routes())
}

fn v1_routes() -> Router {
    Router::new()
        .merge(entities::user::user_router())
        .merge(entities::post::post_router())
        .merge(entities::comment::comment_router())
}
```

## Large Projects

For enterprise applications with many entities:

```
my-api/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── lib.rs
│   │
│   ├── domain/                    # Domain modules
│   │   ├── mod.rs
│   │   │
│   │   ├── users/                 # User domain
│   │   │   ├── mod.rs
│   │   │   ├── entity.rs          # Sea-ORM entity
│   │   │   ├── operations.rs      # Custom CRUDOperations
│   │   │   ├── validation.rs      # Domain validation
│   │   │   └── service.rs         # Business logic
│   │   │
│   │   ├── posts/                 # Post domain
│   │   │   ├── mod.rs
│   │   │   ├── entity.rs
│   │   │   └── operations.rs
│   │   │
│   │   └── billing/               # Billing domain
│   │       ├── mod.rs
│   │       ├── invoice.rs
│   │       ├── payment.rs
│   │       └── subscription.rs
│   │
│   ├── infrastructure/
│   │   ├── mod.rs
│   │   ├── database.rs            # Database setup
│   │   ├── middleware.rs          # Auth, logging, etc.
│   │   └── config.rs              # Environment config
│   │
│   └── api/
│       ├── mod.rs
│       ├── router.rs              # Main router assembly
│       ├── v1/                    # API v1
│       │   ├── mod.rs
│       │   └── routes.rs
│       └── v2/                    # API v2 (when needed)
│           ├── mod.rs
│           └── routes.rs
```

### Domain Module Pattern

**domain/users/entity.rs:**

```rust
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, DeriveEntityModel, EntityToModels)]
#[crudcrate(
    generate_router,
    operations = super::operations::UserOperations
)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    #[crudcrate(primary_key, exclude(create, update))]
    pub id: i32,

    #[crudcrate(filterable, sortable)]
    pub email: String,

    #[crudcrate(exclude(one, list))]
    pub password_hash: String,
}

// ... Relation, ActiveModelBehavior
```

**domain/users/operations.rs:**

```rust
use crate::domain::users::entity;
use crudcrate::{CRUDOperations, ApiError};
use sea_orm::DatabaseConnection;

pub struct UserOperations;

#[async_trait::async_trait]
impl CRUDOperations for UserOperations {
    type Resource = entity::Model;

    async fn before_create(
        &self,
        db: &DatabaseConnection,
        data: &mut entity::ModelCreate,
    ) -> Result<(), ApiError> {
        // Hash password, validate email uniqueness, etc.
        Ok(())
    }
}
```

**domain/users/mod.rs:**

```rust
pub mod entity;
pub mod operations;
pub mod validation;
pub mod service;

pub use entity::*;
```

## Workspace Structure

For very large projects, consider a Cargo workspace:

```
my-platform/
├── Cargo.toml                    # Workspace root
│
├── crates/
│   ├── api/                      # Main API binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs
│   │
│   ├── entities/                 # Shared entities library
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── user.rs
│   │       └── post.rs
│   │
│   ├── domain/                   # Business logic library
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   │
│   └── migrations/               # Database migrations
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs
│
└── tests/                        # Integration tests
    └── api_tests.rs
```

**Root Cargo.toml:**

```toml
[workspace]
members = ["crates/*"]

[workspace.dependencies]
crudcrate = "0.1"
sea-orm = { version = "1.0", features = ["runtime-tokio-rustls", "sqlx-postgres"] }
axum = "0.7"
tokio = { version = "1", features = ["full"] }
```

**crates/entities/Cargo.toml:**

```toml
[package]
name = "entities"
version = "0.1.0"
edition = "2021"

[dependencies]
crudcrate.workspace = true
sea-orm.workspace = true
```

## Best Practices

### Entity Organization

1. **One entity per file** - Keeps files focused and manageable
2. **Group by domain** - Not by technical layer
3. **Co-locate related code** - Operations, validation with entities

### Router Organization

1. **Version your API** - Use `/api/v1/`, `/api/v2/` prefixes
2. **Merge routers in one place** - Makes it easy to see all routes
3. **Add middleware at the router level** - Auth, CORS, logging

```rust
pub fn api_router() -> Router {
    Router::new()
        .nest("/api/v1", v1_routes())
        .layer(AuthLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}
```

### Configuration

Use environment variables with a config module:

```rust
// src/config.rs
use std::env;

pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub environment: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL")
                .expect("DATABASE_URL must be set"),
            port: env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .expect("PORT must be a number"),
            environment: env::var("RUST_ENV")
                .unwrap_or_else(|_| "development".to_string()),
        }
    }
}
```

### Testing Structure

```
my-api/
├── src/
│   └── ...
├── tests/
│   ├── common/
│   │   └── mod.rs           # Test utilities, fixtures
│   ├── user_tests.rs        # User entity tests
│   ├── post_tests.rs        # Post entity tests
│   └── integration.rs       # Full API integration tests
```

## Migration Path

### Starting Small → Growing Large

1. **Start with minimal structure** - Don't over-engineer early
2. **Extract modules when pain appears** - Files > 500 lines, shared code
3. **Add domain folders when themes emerge** - Related entities cluster
4. **Consider workspace when compilation slows** - Parallel builds help

<div class="info">

**Tip:** Use `cargo modules structure` (from cargo-modules crate) to visualize your project structure as it grows.

</div>
