# First API in 5 Minutes

Let's build a task management API from scratch. By the end, you'll have a fully functional REST API with filtering, sorting, and pagination.

## What We're Building

A task management API with:
- Tasks that belong to projects
- Filtering by status and priority
- Fulltext search on task names
- Sorting by due date
- Pagination for large result sets

## Step 1: Project Setup

```bash
cargo new taskapi
cd taskapi
```

Update `Cargo.toml`:

```toml
[package]
name = "taskapi"
version = "0.1.0"
edition = "2021"

[dependencies]
crudcrate = "0.1"
sea-orm = { version = "1.0", features = ["runtime-tokio-rustls", "sqlx-sqlite"] }
axum = "0.7"
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.0", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tower-http = { version = "0.5", features = ["cors"] }
```

## Step 2: Define the Task Entity

Create `src/task.rs`:

```rust
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Task priority levels
#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(10))")]
pub enum Priority {
    #[sea_orm(string_value = "low")]
    Low,
    #[sea_orm(string_value = "medium")]
    Medium,
    #[sea_orm(string_value = "high")]
    High,
    #[sea_orm(string_value = "urgent")]
    Urgent,
}

/// Task status
#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(15))")]
pub enum Status {
    #[sea_orm(string_value = "todo")]
    Todo,
    #[sea_orm(string_value = "in_progress")]
    InProgress,
    #[sea_orm(string_value = "done")]
    Done,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    /// Unique task identifier (auto-generated)
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    /// Task title - searchable and sortable
    #[crudcrate(filterable, sortable, fulltext)]
    pub title: String,

    /// Detailed description (optional)
    pub description: Option<String>,

    /// Task priority - filterable for priority views
    #[crudcrate(filterable, sortable)]
    pub priority: Priority,

    /// Current status - filterable for kanban boards
    #[crudcrate(filterable)]
    pub status: Status,

    /// Due date - sortable for deadline views
    #[crudcrate(sortable, filterable)]
    pub due_date: Option<DateTimeUtc>,

    /// Project assignment (optional)
    #[crudcrate(filterable)]
    pub project_id: Option<Uuid>,

    /// Creation timestamp (auto-set)
    #[crudcrate(sortable, exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTimeUtc,

    /// Last update timestamp (auto-updated)
    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

## Step 3: Create the Server

Update `src/main.rs`:

```rust
mod task;

use axum::{Extension, Router};
use sea_orm::{Database, DatabaseConnection, DbBackend, Schema, ConnectionTrait};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Connect to SQLite (in-memory for demo)
    let db: DatabaseConnection = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to database");

    // Create table
    create_tables(&db).await;

    // Build application with CORS enabled
    let app = Router::new()
        .merge(task::task_router())
        .layer(CorsLayer::permissive())
        .layer(Extension(db));

    // Start server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Task API running at http://{}", addr);
    println!();
    println!("Try these commands:");
    println!("  Create task:  curl -X POST http://localhost:3000/tasks -H 'Content-Type: application/json' -d '{{\"title\":\"My Task\",\"priority\":\"high\",\"status\":\"todo\"}}'");
    println!("  List tasks:   curl http://localhost:3000/tasks");
    println!("  Filter:       curl 'http://localhost:3000/tasks?filter={{\"status\":\"todo\"}}'");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn create_tables(db: &DatabaseConnection) {
    let schema = Schema::new(DbBackend::Sqlite);
    let stmt = schema.create_table_from_entity(task::Entity);

    db.execute(db.get_database_backend().build(&stmt))
        .await
        .expect("Failed to create table");
}
```

## Step 4: Run and Test

```bash
cargo run
```

Now test your API:

```bash
# Create some tasks
curl -X POST http://localhost:3000/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Write documentation",
    "description": "Create user guide for CRUDCrate",
    "priority": "high",
    "status": "in_progress"
  }'

curl -X POST http://localhost:3000/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Fix login bug",
    "priority": "urgent",
    "status": "todo"
  }'

curl -X POST http://localhost:3000/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Review pull request",
    "priority": "medium",
    "status": "todo"
  }'
```

## Step 5: Explore the Generated API

### List All Tasks

```bash
curl http://localhost:3000/tasks | jq
```

### Filter by Status

```bash
# Get all todo tasks
curl 'http://localhost:3000/tasks?filter={"status":"todo"}' | jq
```

### Filter by Priority

```bash
# Get urgent and high priority tasks
curl 'http://localhost:3000/tasks?priority_in=urgent,high' | jq
```

### Search Tasks

```bash
# Fulltext search
curl 'http://localhost:3000/tasks?q=documentation' | jq
```

### Sort Results

```bash
# Sort by priority descending
curl 'http://localhost:3000/tasks?sort=["priority","DESC"]' | jq

# Sort by creation date
curl 'http://localhost:3000/tasks?sort=["created_at","ASC"]' | jq
```

### Pagination

```bash
# React Admin format (first 10)
curl 'http://localhost:3000/tasks?range=[0,9]' | jq

# Standard format
curl 'http://localhost:3000/tasks?page=1&per_page=10' | jq
```

### Update a Task

```bash
# Get task ID from list response, then:
curl -X PUT http://localhost:3000/tasks/{task-id} \
  -H "Content-Type: application/json" \
  -d '{"status": "done"}'
```

### Delete a Task

```bash
curl -X DELETE http://localhost:3000/tasks/{task-id}
```

## What CRUDCrate Generated

From your 50-line entity definition, CRUDCrate generated:

### Models

```rust
// Create request (no id, created_at, updated_at)
pub struct TaskCreate {
    pub title: String,
    pub description: Option<String>,
    pub priority: Priority,
    pub status: Status,
    pub due_date: Option<DateTimeUtc>,
    pub project_id: Option<Uuid>,
}

// Update request (all optional)
pub struct TaskUpdate {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub priority: Option<Priority>,
    pub status: Option<Status>,
    pub due_date: Option<Option<DateTimeUtc>>,
    pub project_id: Option<Option<Uuid>>,
}

// Response model
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub priority: Priority,
    pub status: Status,
    pub due_date: Option<DateTimeUtc>,
    pub project_id: Option<Uuid>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}
```

### Router

```rust
pub fn task_router() -> Router {
    Router::new()
        .route("/tasks", get(list_tasks).post(create_task).delete(bulk_delete_tasks))
        .route("/tasks/:id", get(get_task).put(update_task).delete(delete_task))
}
```

### Handlers

Each handler includes:
- Request parsing and validation
- Database operations
- Error handling with proper HTTP status codes
- Response serialization

## Next Steps

- Add [Relationships](../features/relationships.md) to link tasks to projects
- Implement [Custom Operations](../advanced/custom-operations.md) for business logic
- Add [Validation](../advanced/validation.md) for input checking
- Configure [Security](../advanced/security.md) for production

<div class="success">

**Congratulations!** You've built a production-grade REST API in 5 minutes. The same API built manually would require 500+ lines of boilerplate code.

</div>
