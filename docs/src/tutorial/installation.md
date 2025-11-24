# Installation

CRUDCrate integrates with the Rust ecosystem. Here's how to set it up for your project.

## Requirements

- **Rust**: 1.70 or later (for stable proc-macro features)
- **Sea-ORM**: 1.0+ (the ORM layer)
- **Axum**: 0.7+ (the web framework)
- **Database**: PostgreSQL, MySQL, or SQLite

## Basic Installation

Add CRUDCrate and its peer dependencies to your `Cargo.toml`:

```toml
[dependencies]
crudcrate = "0.1"

# Sea-ORM - choose your database backend
sea-orm = { version = "1.0", features = [
    "runtime-tokio-rustls",
    "sqlx-postgres",  # or sqlx-mysql, sqlx-sqlite
] }

# Axum web framework
axum = "0.7"

# Async runtime
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Common types
uuid = { version = "1.0", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
```

## Database-Specific Setup

### PostgreSQL (Recommended)

```toml
[dependencies]
sea-orm = { version = "1.0", features = [
    "runtime-tokio-rustls",
    "sqlx-postgres",
] }
```

PostgreSQL gets the best optimizations:
- GIN indexes for fulltext search
- `tsvector` query optimization
- Array operations for batch queries

### MySQL

```toml
[dependencies]
sea-orm = { version = "1.0", features = [
    "runtime-tokio-rustls",
    "sqlx-mysql",
] }
```

MySQL features:
- FULLTEXT index support
- `MATCH AGAINST` query optimization
- Optimized `LIKE` queries

### SQLite

```toml
[dependencies]
sea-orm = { version = "1.0", features = [
    "runtime-tokio-rustls",
    "sqlx-sqlite",
] }
```

SQLite is great for:
- Development and testing
- Embedded applications
- Single-file databases

## Optional Features

### Validation Support

```toml
[dependencies]
crudcrate = { version = "0.1", features = ["validation"] }
validator = "0.18"
```

### Tracing/Logging

```toml
[dependencies]
tracing = "0.1"
tracing-subscriber = "0.3"
```

CRUDCrate uses `tracing` for structured logging when available.

## Verify Installation

Create a simple test to verify everything works:

```rust
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "test")]
pub struct Model {
    #[sea_orm(primary_key)]
    #[crudcrate(primary_key)]
    pub id: i32,
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

fn main() {
    // If this compiles, CRUDCrate is working!
    println!("CRUDCrate installed successfully!");
}
```

Run:

```bash
cargo build
```

If compilation succeeds, you're ready to go!

## IDE Setup

### VS Code

Install the `rust-analyzer` extension for:
- Macro expansion preview
- Type hints for generated models
- Jump to definition for generated code

### IntelliJ/CLion

The Rust plugin provides similar features. Enable "Expand macros" in settings.

## Troubleshooting

### "cannot find derive macro `EntityToModels`"

Make sure you've imported it:

```rust
use crudcrate::EntityToModels;
```

### "unresolved import `sea_orm::entity::prelude`"

Check your Sea-ORM features include the database backend:

```toml
sea-orm = { version = "1.0", features = ["runtime-tokio-rustls", "sqlx-postgres"] }
```

### Slow compilation

Proc macros can slow down incremental builds. Tips:
- Use `cargo build --release` for production
- Consider splitting entities into separate crates for large projects
- Use `cargo check` during development

## Next Steps

- Follow the [First API](./first-api.md) tutorial
- Understand the [Project Structure](./project-structure.md)
- Learn about [The Entity Model](../concepts/entity-model.md)
