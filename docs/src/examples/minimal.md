# Minimal API Example

The simplest possible CRUDCrate API.

## Full Code

```rust
// main.rs
use axum::{Extension, Router};
use crudcrate::EntityToModels;
use sea_orm::{entity::prelude::*, Database, DatabaseConnection};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

// Define the entity
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub name: String,

    pub description: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[tokio::main]
async fn main() {
    let db: DatabaseConnection = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect");

    // Create table (use migrations in production)
    let schema = sea_orm::Schema::new(sea_orm::DatabaseBackend::Sqlite);
    db.execute(db.get_database_backend().build(&schema.create_table_from_entity(Entity)))
        .await
        .expect("Failed to create table");

    let app = Router::new()
        .merge(item_router())
        .layer(Extension(db));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

## Cargo.toml

```toml
[package]
name = "minimal-api"
version = "0.1.0"
edition = "2021"

[dependencies]
crudcrate = "0.1"
sea-orm = { version = "1.0", features = ["runtime-tokio-rustls", "sqlx-sqlite"] }
axum = "0.7"
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
```

## Run

```bash
cargo run
```

## Test

```bash
# Create
curl -X POST http://localhost:3000/items \
  -H "Content-Type: application/json" \
  -d '{"name": "Test Item"}'

# List
curl http://localhost:3000/items

# Get
curl http://localhost:3000/items/{id}

# Update
curl -X PUT http://localhost:3000/items/{id} \
  -H "Content-Type: application/json" \
  -d '{"name": "Updated"}'

# Delete
curl -X DELETE http://localhost:3000/items/{id}
```

## Lines of Code

- Entity definition: ~20 lines
- Server setup: ~15 lines
- **Total: ~35 lines**

Without CRUDCrate, this would require ~500+ lines for handlers, models, filtering, pagination, etc.
