# Crudcrate Examples

This directory contains examples demonstrating various features of crudcrate.

## Available Examples

### Minimal Example (`minimal/`)

A complete CRUD API implemented in just ~60 lines of code.

```bash
cargo run --example minimal
```

**URLs:**
- API: http://localhost:3000/todo
- Documentation: http://localhost:3000/docs

### CRUD Operations (`crud_operations.rs`)

Demonstrates the `CRUDOperations` trait for advanced customization:
- Lifecycle hooks (`before_create`, `after_get_one`, etc.)
- Core method overrides (`fetch_all`, `fetch_one`)
- Full operation overrides with external service integration

```bash
cargo run --example crud_operations
```

### Error Handling (`error_handling.rs`)

Comprehensive error handling patterns with `ApiError`.

```bash
cargo run --example error_handling
```

### Recursive Joins (`recursive_join/`)

Demonstrates recursive entity relationships with depth control.

```bash
cargo run --example recursive_join
```

## Hook System

Crudcrate provides two ways to customize CRUD behavior:

### 1. Attribute-Based Hooks (Simple)

Use the hook syntax directly in your entity definition:

```rust
#[derive(EntityToModels)]
#[crudcrate(
    api_struct = "Asset",
    generate_router,
    // Pre-hooks run before the operation
    create::one::pre = validate_asset,
    // Body hooks replace the default implementation
    delete::one::body = delete_with_s3_cleanup,
    // Post-hooks run after the operation
    create::one::post = notify_created,
)]
pub struct Model { /* ... */ }

// Hook function signatures:
async fn validate_asset(db: &DatabaseConnection, data: &AssetCreate) -> Result<(), ApiError>;
async fn delete_with_s3_cleanup(db: &DatabaseConnection, id: Uuid) -> Result<Uuid, ApiError>;
async fn notify_created(db: &DatabaseConnection, entity: &Asset) -> Result<(), ApiError>;
```

**Hook Syntax:** `{operation}::{cardinality}::{phase}`
- **Operations:** `create`, `read`, `update`, `delete`
- **Cardinality:** `one` (single item), `many` (batch)
- **Phases:** `pre`, `body`, `post`

### 2. CRUDOperations Trait (Advanced)

For complex customization, implement the `CRUDOperations` trait:

```rust
#[derive(EntityToModels)]
#[crudcrate(generate_router, operations = MyOperations)]
pub struct Model { /* ... */ }

pub struct MyOperations;

#[async_trait]
impl CRUDOperations for MyOperations {
    type Resource = MyEntity;

    async fn before_create(&self, db: &DatabaseConnection, data: &CreateModel) -> Result<(), ApiError> {
        // Validation logic
        Ok(())
    }

    async fn delete(&self, db: &DatabaseConnection, id: Uuid) -> Result<Uuid, ApiError> {
        // Full operation override with custom logic
        self.perform_delete(db, id).await
    }
}
```

See `crud_operations.rs` for a complete example.

## Testing APIs

```bash
# Create an item
curl -X POST http://localhost:3000/todo \
  -H 'Content-Type: application/json' \
  -d '{"title": "Learn Rust", "completed": false}'

# List items
curl http://localhost:3000/todo

# Get OpenAPI documentation
open http://localhost:3000/docs
```
