# Quick Start

Build a complete REST API in under 5 minutes.

## Prerequisites

- Rust 1.70+ installed
- A database (PostgreSQL, MySQL, or SQLite)
- Basic familiarity with Sea-ORM

## Step 1: Create a New Project

```bash
cargo new my-api
cd my-api
```

## Step 2: Add Dependencies

```toml
# Cargo.toml
[dependencies]
crudcrate = "0.1"
sea-orm = { version = "1.0", features = ["runtime-tokio-rustls", "sqlx-sqlite"] }
axum = "0.7"
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
```

## Step 3: Define Your Entity

Create `src/entities/todo.rs`:

```rust
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "todos")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable, fulltext)]
    pub title: String,

    pub description: Option<String>,

    #[crudcrate(filterable)]
    pub completed: bool,

    #[crudcrate(sortable, exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

## Step 4: Create Your Server

```rust
// src/main.rs
mod entities;

use axum::{Extension, Router};
use sea_orm::{Database, DatabaseConnection};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    // Connect to database
    let db: DatabaseConnection = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to database");

    // Create table (in production, use migrations)
    // sea_orm::Schema::create_table_from_entity(entities::todo::Entity);

    // Build router with generated CRUD endpoints
    let app = Router::new()
        .merge(entities::todo::todo_router())
        .layer(Extension(db));

    // Start server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Server running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

## Step 5: Run It

```bash
cargo run
```

## Step 6: Test Your API

```bash
# Create a todo
curl -X POST http://localhost:3000/todos \
  -H "Content-Type: application/json" \
  -d '{"title": "Learn CRUDCrate", "description": "Read the docs", "completed": false}'

# List all todos
curl http://localhost:3000/todos

# Get a specific todo
curl http://localhost:3000/todos/{id}

# Update a todo
curl -X PUT http://localhost:3000/todos/{id} \
  -H "Content-Type: application/json" \
  -d '{"completed": true}'

# Filter todos
curl "http://localhost:3000/todos?filter={\"completed\":false}"

# Search todos
curl "http://localhost:3000/todos?q=learn"

# Sort todos
curl "http://localhost:3000/todos?sort=[\"created_at\",\"DESC\"]"

# Delete a todo
curl -X DELETE http://localhost:3000/todos/{id}
```

## What Just Happened?

From a single `#[derive(EntityToModels)]`, CRUDCrate generated:

1. **Four model structs:**
   - `Todo` - Full response model
   - `TodoCreate` - Create request (excludes `id`, `created_at`)
   - `TodoUpdate` - Update request (all fields optional)
   - `TodoList` - List response model

2. **Six HTTP endpoints:**
   - `GET /todos` - List with filtering, sorting, pagination
   - `GET /todos/:id` - Get single item
   - `POST /todos` - Create
   - `PUT /todos/:id` - Update
   - `DELETE /todos/:id` - Delete one
   - `DELETE /todos` - Bulk delete

3. **Query capabilities:**
   - Filtering on `title`, `completed` fields
   - Sorting on `title`, `created_at` fields
   - Fulltext search on `title`
   - Automatic pagination

## Next Steps

- Learn about [Field Attributes](../concepts/attributes.md) to customize your models
- Add [Relationships](../features/relationships.md) for nested data
- Implement [Custom Operations](../advanced/custom-operations.md) for business logic
- Configure [Validation](../advanced/validation.md) for input checking

<div class="info">

**Tip:** Run `cargo doc --open` to see the generated documentation for your models and traits.

</div>
