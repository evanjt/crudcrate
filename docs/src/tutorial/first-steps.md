# Your First API

{{#test_link crud}}

Let's build a task manager. We'll start simple and add features as we need them.

## Setup

```bash
cargo new taskmanager
cd taskmanager
```

Add dependencies to `Cargo.toml`:

```toml
[package]
name = "taskmanager"
version = "0.1.0"
edition = "2021"

[dependencies]
crudcrate = "0.1"
sea-orm = { version = "1.0", features = ["runtime-tokio-rustls", "sqlx-sqlite"] }
axum = "0.7"
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
```

## The Simplest Task

Replace `src/main.rs`:

```rust
use axum::Router;
use crudcrate::EntityToModels;
use sea_orm::{entity::prelude::*, Database, DatabaseConnection};

#[derive(Clone, Debug, DeriveEntityModel, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    #[sea_orm(primary_key)]
    #[crudcrate(primary_key)]
    pub id: i32,

    pub title: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[tokio::main]
async fn main() {
    let db: DatabaseConnection = Database::connect("sqlite::memory:")
        .await
        .expect("Database connection failed");

    // Create table
    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        "CREATE TABLE tasks (id INTEGER PRIMARY KEY, title TEXT NOT NULL)".to_owned(),
    ))
    .await
    .expect("Table creation failed");

    let app = Router::new()
        .merge(task_router())
        .layer(axum::Extension(db));

    println!("Task Manager running at http://localhost:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

## Run It

```bash
cargo run
```

## Try It

```bash
# Create a task
curl -X POST http://localhost:3000/tasks \
  -H "Content-Type: application/json" \
  -d '{"id": 1, "title": "Learn CRUDCrate"}'

# List tasks
curl http://localhost:3000/tasks

# Get one task
curl http://localhost:3000/tasks/1

# Update it
curl -X PUT http://localhost:3000/tasks/1 \
  -H "Content-Type: application/json" \
  -d '{"title": "Master CRUDCrate"}'

# Delete it
curl -X DELETE http://localhost:3000/tasks/1
```

**That's it.** You have a working REST API.

---

## What We Got

From those few lines, CRUDCrate generated:

| Endpoint | What it does |
|----------|--------------|
| `GET /tasks` | List all tasks |
| `GET /tasks/:id` | Get one task |
| `POST /tasks` | Create a task |
| `PUT /tasks/:id` | Update a task |
| `DELETE /tasks/:id` | Delete a task |

Plus request/response models, error handling, and JSON serialization.

---

## Try the Minimal Example

Don't want to type all this? Run the included example:

```bash
git clone https://github.com/evanjt/crudcrate
cd crudcrate/crudcrate
cargo run --example minimal
```

Then visit:
- **API**: http://localhost:3000/todo
- **Docs**: http://localhost:3000/docs (interactive OpenAPI)

---

## But Wait...

Did you notice we had to specify `"id": 1` when creating? That's annoying. Users shouldn't have to pick their own IDs.

**Next:** [Let's fix that](./auto-ids.md) - make IDs generate automatically.
