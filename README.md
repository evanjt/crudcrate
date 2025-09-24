# crudcrate

[![Tests](https://github.com/evanjt/crudcrate/actions/workflows/test.yml/badge.svg)](https://github.com/evanjt/crudcrate/actions/workflows/test.yml)
[![codecov](https://codecov.io/gh/evanjt/crudcrate/branch/main/graph/badge.svg)](https://codecov.io/gh/evanjt/crudcrate)
[![Crates.io](https://img.shields.io/crates/v/crudcrate.svg)](https://crates.io/crates/crudcrate)
[![Documentation](https://docs.rs/crudcrate/badge.svg)](https://docs.rs/crudcrate)

**Zero-boilerplate CRUD APIs for Sea-ORM and Axum.**

Transform Sea-ORM entities into complete REST APIs with a single derive macro. Generate CRUD endpoints, type-safe models, and OpenAPI documentation automatically while working seamlessly alongside custom queries and handlers.

```rust
use crudcrate::EntityToModels;

#[derive(EntityToModels)]
#[crudcrate(generate_router)]
pub struct Model {
    #[crudcrate(primary_key, exclude(create, update))]
    pub id: Uuid,
    #[crudcrate(filterable, sortable, fulltext)]
    pub title: String,
    #[crudcrate(filterable)]
    pub completed: bool,
}

// Generates: Todo, TodoCreate, TodoUpdate + router() function with all CRUD endpoints
```

## Quick Start

```bash
cargo add crudcrate
```

### Database Support

Choose your database drivers to optimize binary size:

```toml
# Default: SQLite only
crudcrate = "0.4.1"

# Single database (smallest binary)
crudcrate = { version = "0.4.1", features = ["mysql"], default-features = false }

# Multiple databases (runtime flexibility)
crudcrate = { version = "0.4.1", features = ["mysql", "postgresql"] }
```

### Complete Working Example

```rust
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "todos")]
#[crudcrate(generate_router)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update))]
    pub id: Uuid,

    #[crudcrate(filterable, sortable, fulltext)]
    pub title: String,

    #[crudcrate(filterable)]
    pub completed: bool,
}

// Use the generated router
let app = Router::new()
    .nest("/api/todos", router(&db))  // Generated router function
    .with_state(db);
```

**Generated API endpoints:**
- `GET /api/todos` - List with filtering, sorting, pagination
- `GET /api/todos/{id}` - Get single item
- `POST /api/todos` - Create new item  
- `PUT /api/todos/{id}` - Update item
- `DELETE /api/todos/{id}` - Delete item
- `DELETE /api/todos/batch` - Batch delete

## 1. Auto-Generated CRUD Operations

Transform Sea-ORM entities into complete REST APIs with zero boilerplate.

### Router Generation

```rust
#[crudcrate(generate_router)]
pub struct Model { /* ... */ }

// Creates a router() function with all CRUD endpoints
let app = Router::new()
    .nest("/api/todos", router(&db))
    .with_state(db);
```

### Custom Function Injection

Override default CRUD operations with custom logic:

```rust
#[crudcrate(fn_get_one = custom_get_one)]
pub struct Model { /* ... */ }

async fn custom_get_one(db: &DatabaseConnection, id: Uuid) -> Result<Todo, DbErr> {
    // Add permissions, caching, audit trails, etc.
    Entity::find_by_id(id)
        .filter(Column::UserId.eq(current_user_id()))
        .one(db)
        .await?
        .ok_or(DbErr::RecordNotFound("Todo not found"))
}
```

Available function overrides: `fn_get_one`, `fn_get_all`, `fn_create`, `fn_update`, `fn_delete`, `fn_delete_many`

### OpenAPI Documentation

Automatic API documentation generation via utoipa integration:

```rust
#[crudcrate(description = "Task management system")]
pub struct Model { /* ... */ }
```

## 2. Smart Model Generation

Automatically creates Create/Update/List structs from your database model.

### Generated Models

```rust
// Your entity becomes 4 specialized models:
#[derive(EntityToModels)]
pub struct Model {
    pub id: Uuid,
    pub title: String,
    pub completed: bool,
}

// Generated:
pub struct Todo {           // API response
    pub id: Uuid,
    pub title: String,
    pub completed: bool,
}

pub struct TodoCreate {     // POST request body
    pub title: String,
    pub completed: bool,
    // id excluded automatically
}

pub struct TodoUpdate {     // PUT request body  
    pub title: Option<String>,
    pub completed: Option<Option<bool>>,
    // Option<Option<T>> distinguishes "don't update" vs "set to null"
}
```

### Model Control

```rust
// Function-style syntax
#[crudcrate(exclude(create, update))]    // Exclude from Create and Update
pub id: Uuid,

// Boolean syntax (equivalent)
#[crudcrate(create_model = false, update_model = false)]
pub id: Uuid,

// Auto-generation
#[crudcrate(on_create = Uuid::new_v4(), on_update = Utc::now())]
pub updated_at: DateTime<Utc>,
```

### Timestamp Management

```rust
// Auto-managed timestamps
#[crudcrate(sortable, exclude(create, update), on_create = Utc::now())]
pub created_at: DateTime<Utc>,

#[crudcrate(sortable, exclude(create, update), on_create = Utc::now(), on_update = Utc::now())]
pub updated_at: DateTime<Utc>,
```

## 3. Advanced Filtering & Search

Rich query parameter handling with database-optimized fulltext search.

### Basic Filtering

```bash
# Simple equality filters
GET /api/todos?filter={"completed":false,"priority":"high"}

# Numeric comparisons
GET /api/todos?filter={"priority_gte":3,"due_date_lt":"2024-01-01"}

# List operations (IN queries)
GET /api/todos?filter={"id":["uuid1","uuid2","uuid3"]}
```

### Fulltext Search

Multi-field search with automatic database optimizations:

```rust
#[crudcrate(filterable, fulltext)]
pub title: String,

#[crudcrate(filterable, fulltext)]
pub content: String,
```

```bash
# Search across all fulltext fields
GET /api/todos?filter={"q":"important meeting"}
```

### Database-Specific Optimizations

**PostgreSQL**: Native tsvector with GIN indexes
```sql
-- Auto-generated query
WHERE to_tsvector('english', title || ' ' || content) @@ plainto_tsquery('english', 'search terms')

-- Recommended index
CREATE INDEX idx_todos_fulltext ON todos USING GIN (to_tsvector('english', title || ' ' || content));
```

**SQLite**: Case-insensitive LIKE queries
```sql
-- Auto-generated fallback
WHERE (UPPER(title) LIKE UPPER('%search%') OR UPPER(content) LIKE UPPER('%terms%'))
```

**MySQL**: MATCH AGAINST for fulltext indexes

### Sorting & Pagination

```bash
# Sorting
GET /api/todos?sort=created_at&order=DESC

# Pagination (React Admin compatible)
GET /api/todos?page=0&per_page=20
```

### Language Configuration

```rust
#[crudcrate(fulltext_language = "spanish")]    // Spanish text processing
#[crudcrate(fulltext_language = "simple")]     // Language-agnostic
// Default: "english"
```

## 4. Relationship Loading

Populate related data in API responses automatically without N+1 queries.

### Single-Level Joins

```rust
#[derive(EntityToModels)]
pub struct Model {
    pub id: Uuid,
    pub name: String,
    
    // Automatically loads related vehicles in API responses
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all))]
    pub vehicles: Vec<Vehicle>,
}
```

### Join Configuration Options

```rust
#[crudcrate(join(one))]          // Load only in get_one() responses
#[crudcrate(join(all))]          // Load only in get_all() responses  
#[crudcrate(join(one, all))]     // Load in both responses
```

### Multi-Level Joins (Planned)

```rust
// Future: recursive loading with depth control
#[crudcrate(join(one, all, depth = 2))]
pub deep_relationships: Vec<RelatedEntity>,
```

### Type-Based Relationship Detection

- `Vec<T>` fields → `has_many` relationships → `.all()` loading
- `Option<T>` or `T` fields → `belongs_to`/`has_one` relationships → `.one()` loading

## 5. Multi-Database Optimization

Database-specific query optimizations and index recommendations for production performance.

### Automatic Index Analysis

```rust
// Analyze all models at startup
let _ = crudcrate::analyse_all_registered_models(&db, false).await;  // Compact
let _ = crudcrate::analyse_all_registered_models(&db, true).await;   // With SQL
```

**Sample output:**
```
crudcrate Index Analysis
═══════════════════════════

HIGH High Priority:
  todos - Fulltext search on 2 columns without proper index
    CREATE INDEX idx_todos_fulltext ON todos USING GIN (to_tsvector('english', title || ' ' || content));

MEDIUM Medium Priority:  
  todos - Field 'completed' is filterable but not indexed
    CREATE INDEX idx_todos_completed ON todos (completed);
```

### Database-Specific Features

**PostgreSQL**: 
- GIN indexes for fulltext search
- tsvector optimization
- JSON operations
- Best for production

**MySQL**:
- FULLTEXT indexes
- Spatial data support
- Match against queries

**SQLite**:
- LIKE-based fallback
- Ideal for development/testing
- No network overhead

### Performance Characteristics

- **Sub-millisecond responses**: Most operations 200-300µs
- **Efficient connection pooling**: Via Sea-ORM
- **Query optimization**: Proper indexing hints
- **Zero-copy deserialization**: Where possible

### Multi-Database Testing

```bash
# PostgreSQL testing
DATABASE_URL=postgres://postgres:pass@localhost/test_db cargo test

# MySQL testing  
DATABASE_URL=mysql://root:pass@127.0.0.1:3306/test_db cargo test -- --test-threads=1

# SQLite (default)
cargo test
```

## 6. Development Experience

Rich tooling, debug output, and IDE support for fast development cycles.

### Debug Generated Code

See exactly what code the macros generate:

```bash
cargo run --example minimal --features=debug
```

```rust
#[crudcrate(debug_output)]  // Add to any struct
pub struct Model { /* ... */ }
```

### IDE Support

Complete autocomplete and type definitions for all 30+ attributes:

```rust
#[crudcrate(
    // Struct-level
    generate_router,
    api_struct = "Customer", 
    description = "Customer records",
    
    // Field-level  
    primary_key,
    filterable,
    sortable,
    fulltext,
    exclude(create, update),
    join(one, all),
    on_create = Uuid::new_v4()
)]
```

### Error Handling & Validation

- **SQL injection prevention**: All input parameterized via Sea-ORM
- **Input validation**: Field names and values validated
- **Query sanitization**: Search terms escaped automatically
- **Type safety**: Compile-time validation of configurations

### Testing Infrastructure

Comprehensive test suite with multi-database support:

```bash
# Run all tests
cargo test --workspace

# Specific categories
cargo test --test crud_operations_test
cargo test --test filtering_search_test
cargo test --test relationship_loading_test
```

### Performance Benchmarking

```bash
# SQLite benchmarks
cargo bench --bench crud_benchmarks

# PostgreSQL comparison
docker run --name benchmark-postgres -e POSTGRES_PASSWORD=pass -e POSTGRES_DB=benchmark -p 5432:5432 -d postgres:16
BENCHMARK_DATABASE_URL=postgres://postgres:pass@localhost/benchmark cargo bench --bench crud_benchmarks
```

## Complete Attribute Reference

### Struct-Level Attributes

```rust
#[crudcrate(
    // Router & Documentation
    generate_router,                  // Auto-generate Axum router
    api_struct = "CustomerAPI",       // Override API struct name
    description = "Customer data",    // OpenAPI description
    
    // Resource Naming
    name_singular = "customer",       // URL singular name
    name_plural = "customers",        // URL plural name
    
    // Type Overrides
    active_model = "CustomModel",     // ActiveModel path
    entity_type = "Entity",           // Entity type
    column_type = "Column",           // Column type
    
    // Search Configuration
    fulltext_language = "english",    // Default fulltext language
    
    // Custom Functions
    fn_get_one = custom::get_one,     // Override CRUD functions
    fn_get_all = custom::get_all,
    fn_create = custom::create,
    fn_update = custom::update,
    fn_delete = custom::delete,
    fn_delete_many = custom::delete_many,
    
    // Development
    debug_output,                     // Print generated code
)]
```

### Field-Level Attributes

```rust
#[crudcrate(
    // Database Properties
    primary_key,                      // Primary key field
    non_db_attr,                      // Non-database field
    enum_field,                       // Enum field (required for enum filtering)
    
    // Query Capabilities
    sortable,                         // Enable sorting
    filterable,                       // Enable filtering  
    fulltext,                         // Include in fulltext search
    
    // Model Inclusion/Exclusion
    exclude(create),                  // Function-style exclusion
    exclude(update),                  
    exclude(list),
    exclude(create, update),          // Multiple exclusions
    exclude(create, update, list),    // All models
    
    // Boolean equivalents
    create_model = false,             // Exclude from Create model
    update_model = false,             // Exclude from Update model
    list_model = false,               // Exclude from List model
    
    // Auto-Generation
    on_create = Uuid::new_v4(),       // Auto-generate on create
    on_update = Utc::now(),           // Auto-update on modification
    default = vec![],                 // Default for non-DB fields
    
    // Join Configuration
    join(one),                        // Load in get_one() only
    join(all),                        // Load in get_all() only
    join(one, all),                   // Load in both
    join(one, all, depth = 2),        // Custom depth (planned)
    
    // Search Configuration
    fulltext_language = "spanish",    // Field-level language override
)]
```

### Common Patterns

```rust
// Primary key
#[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
pub id: Uuid,

// Searchable text field
#[crudcrate(sortable, filterable, fulltext)]
pub title: String,

// Auto-managed timestamps
#[crudcrate(sortable, exclude(create, update), on_create = Utc::now())]
pub created_at: DateTime<Utc>,

#[crudcrate(sortable, exclude(create, update), on_create = Utc::now(), on_update = Utc::now())]
pub updated_at: DateTime<Utc>,

// Relationship loading
#[sea_orm(ignore)]
#[crudcrate(non_db_attr, join(one, all))]
pub related_items: Vec<RelatedItem>,

// Enum fields
#[crudcrate(filterable, enum_field)]
pub status: TaskStatus,
```

## Examples & Integration

### React Admin Integration

Follows React Admin REST conventions out of the box:

```javascript
// These endpoints work automatically with React Admin:
GET    /api/todos                    // List with pagination
GET    /api/todos?filter={"completed":false}  // Filtered list
GET    /api/todos/123                // Get one
POST   /api/todos                    // Create
PUT    /api/todos/123                // Update
DELETE /api/todos/123                // Delete
```

### Security & Production

Built-in security measures with additional production recommendations:

```rust
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use axum_helmet::Helmet;

let app = Router::new()
    .nest("/api", your_crud_routes)
    .layer(Helmet::default())           // Security headers
    .layer(TraceLayer::new_for_http())  // Request logging  
    .layer(CorsLayer::permissive());    // CORS configuration
```

### Migration Integration

Works seamlessly with Sea-ORM migrations:

```rust
use sea_orm_migration::prelude::*;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(Table::create()
                .table(Todo::Table)
                .col(ColumnDef::new(Todo::Id).uuid().primary_key())
                .col(ColumnDef::new(Todo::Title).string().not_null())
                .to_owned())
            .await
    }
}
```

## Testing

### Quick Testing

```bash
# Run all tests (SQLite default)
cargo test --workspace

# Feature-specific tests  
cargo test --test crud_operations_test
cargo test --test model_generation_test
cargo test --test filtering_search_test
cargo test --test relationship_loading_test
cargo test --test database_optimization_test
cargo test --test dev_experience_test
```

### Multi-Database Testing

```bash
# PostgreSQL (requires running instance)
docker run --name test-postgres -e POSTGRES_PASSWORD=pass -e POSTGRES_DB=test_db -p 5432:5432 -d postgres:16
DATABASE_URL=postgres://postgres:pass@localhost/test_db cargo test

# MySQL (requires single-threaded execution)
docker run --name test-mysql -e MYSQL_ROOT_PASSWORD=pass -e MYSQL_DATABASE=test_db -p 3306:3306 -d mysql:8
sleep 20  # MySQL initialization time
DATABASE_URL=mysql://root:pass@127.0.0.1:3306/test_db cargo test -- --test-threads=1
```

### Benchmarking

```bash
# SQLite performance baseline
cargo bench --bench crud_benchmarks -- --verbose

# PostgreSQL comparison
docker run --name benchmark-postgres -e POSTGRES_PASSWORD=pass -e POSTGRES_DB=benchmark -p 5432:5432 -d postgres:16  
BENCHMARK_DATABASE_URL=postgres://postgres:pass@localhost/benchmark cargo bench --bench crud_benchmarks -- --verbose
```

## License & Support

**MIT License** - See [LICENSE](./LICENSE) for details.

**Development Status**: Active development - API may change between versions.

**Security**: Built-in protection against common vulnerabilities. Users responsible for production security configuration.

## Related Crates

- **[sea-orm](https://crates.io/crates/sea-orm)**: Database ORM and query builder
- **[axum](https://crates.io/crates/axum)**: Web application framework  
- **[utoipa](https://crates.io/crates/utoipa)**: OpenAPI documentation
- **[serde](https://crates.io/crates/serde)**: Serialization framework
- **[tower-http](https://crates.io/crates/tower-http)**: Production HTTP middleware