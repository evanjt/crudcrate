# Minimal Example

The simplest CRUDCrate API - under 60 lines of code.

## Run It Now

```bash
git clone https://github.com/evanjt/crudcrate
cd crudcrate/crudcrate
cargo run --example minimal
```

Then visit:
- **API**: http://localhost:3000/todo
- **Docs**: http://localhost:3000/docs (interactive OpenAPI)

---

## The Code

```rust
use axum::Router;
use crudcrate::EntityToModels;
use sea_orm::{entity::prelude::*, Database, DatabaseConnection};
use uuid::Uuid;

#[derive(Clone, Debug, DeriveEntityModel, EntityToModels)]
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

    setup_database(&db).await;

    let app = Router::new()
        .merge(item_router())
        .layer(axum::Extension(db));

    println!("Running at http://localhost:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn setup_database(db: &DatabaseConnection) {
    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        "CREATE TABLE items (id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT)"
            .to_owned(),
    ))
    .await
    .expect("Failed to create table");
}
```

## Dependencies

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
  -d '{"name": "Test", "description": "A test item"}'

# List all
curl http://localhost:3000/items

# Filter
curl 'http://localhost:3000/items?filter={"name":"Test"}'

# Sort
curl 'http://localhost:3000/items?sort=["name","DESC"]'

# Get one (use ID from create response)
curl http://localhost:3000/items/{id}

# Update
curl -X PUT http://localhost:3000/items/{id} \
  -H "Content-Type: application/json" \
  -d '{"name": "Updated"}'

# Delete
curl -X DELETE http://localhost:3000/items/{id}
```

## What You Get

From ~35 lines:
- 6 REST endpoints
- UUID generation
- Filtering on `name`
- Sorting on `name`
- Pagination with Content-Range headers
- JSON serialization
- Error handling

Without CRUDCrate, this would require 500+ lines of handlers, models, and parsing logic.

---

**Next:** See the [Todo App](./todo-app.md) for a more complete example with timestamps and status tracking.
