# crudcrate

[![Tests](https://github.com/evanjt/crudcrate/actions/workflows/test.yml/badge.svg)](https://github.com/evanjt/crudcrate/actions/workflows/test.yml)
[![codecov](https://codecov.io/gh/evanjt/crudcrate/branch/main/graph/badge.svg)](https://codecov.io/gh/evanjt/crudcrate)
[![Crates.io](https://img.shields.io/crates/v/crudcrate.svg)](https://crates.io/crates/crudcrate)
[![Documentation](https://docs.rs/crudcrate/badge.svg)](https://docs.rs/crudcrate)

**Zero-boilerplate CRUD APIs for Sea-ORM and Axum.**

`crudcrate` generates complete CRUD endpoints from your entities while working seamlessly alongside custom queries and handlers. No lock-in, just less repetitive code - use it where it helps, write custom logic where you need it.

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

Available features: `sqlite` (default), `mysql`, `postgresql`

```rust
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "todos")]
#[crudcrate(generate_router)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, create_model = false, update_model = false)]
    pub id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub title: String,

    #[crudcrate(filterable)]
    pub completed: bool,
}

// Generates: Todo, TodoCreate, TodoUpdate structs + complete CRUD handlers + router() function
```

Use the generated router:

```rust
// During application startup - analyze indexes for optimization recommendations
let _ = crudcrate::analyse_all_registered_models(&db, false).await;  // Compact output
// let _ = crudcrate::analyse_all_registered_models(&db, true).await; // With SQL commands

let app = Router::new()
    .nest("/api/todos", router(&db))  // Generated router function
    .with_state(db);
```

## Core Features

### Entity Generation

Generate API structs, CRUD operations, and handlers from Sea-ORM entities.

```rust
#[crudcrate(api_struct = "Task", description = "Task management")]
```

### Field Attributes

Control how fields behave in the generated API. [See all field attributes](#advanced-field-control)

```rust
#[crudcrate(
    primary_key,                    // Mark as primary key
    filterable,                     // Enable filtering
    sortable,                       // Enable sorting
    fulltext,                       // Include in fulltext search
    create_model = false,           // Exclude from create operations
    update_model = false,           // Exclude from update operations
    on_create = Uuid::new_v4(),     // Auto-generate on create
    on_update = Utc::now()          // Auto-update on modification
)]
```

### Fulltext Search

Multi-field search with database optimizations. [See fulltext search architecture](#fulltext-search-architecture)

```rust
#[crudcrate(fulltext)]
pub title: String,

#[crudcrate(fulltext)]
pub content: String,
```

```bash
GET /api/todos?filter={"q":"search term"}
```

### Filtering & Sorting

React Admin compatible query parameters.

```bash
# Filtering
GET /api/todos?filter={"completed":false,"priority":"high"}

# Sorting
GET /api/todos?sort=created_at&order=DESC

# Pagination
GET /api/todos?page=0&per_page=20
```

### Function Injection

Override default CRUD operations with custom logic. [See custom function injection](#custom-function-injection)

```rust
#[crudcrate(fn_get_one = custom_get_one)]
pub struct Model { /* ... */ }

async fn custom_get_one(db: &DatabaseConnection, id: Uuid) -> Result<Todo, DbErr> {
    // Custom implementation
}
```

## Generated Code

The `EntityToModels` macro generates:

- **API Struct**: `Todo` with all public fields
- **Create Model**: `TodoCreate` for POST requests
- **Update Model**: `TodoUpdate` with `Option<Option<T>>` pattern
- **CRUD Handlers**: Complete HTTP handlers for all operations
- **Router Function**: `router(db)` with all endpoints configured
- **OpenAPI Documentation**: Automatic API docs via utoipa

## Security

`crudcrate` includes essential CRUD security (SQL injection prevention, input validation). For production applications, add:

```toml
[dependencies]
tower-http = { version = "0.6", features = ["cors", "trace"] }
axum-helmet = "0.1"
```

See `tests/external_security_integration_test.rs` for a complete example.

## Performance

Sub-millisecond responses for typical operations:

- GET requests: ~200-300µs (both backends)
- Fulltext search: ~400µs (SQLite), ~2-100ms (PostgreSQL with network)
- CREATE operations: ~110-175µs (both backends)

[See detailed performance characteristics](#performance-characteristics)

### Automatic Index Analysis

`crudcrate` automatically registers all models and analyzes your database for missing indexes at startup:

```rust
// Automatic registration - all models with EntityToModels are included
let _ = crudcrate::analyse_all_registered_models(&db, false).await;  // Compact summary
let _ = crudcrate::analyse_all_registered_models(&db, true).await;   // With SQL commands
```

Output:

```
crudcrate Index Analysis
═══════════════════════════

HIGH High Priority:
  todos - Fulltext search on 2 columns without proper index
    CREATE INDEX idx_todos_fulltext ON todos USING GIN (to_tsvector('english', title || ' ' || content));

MEDIUM Medium Priority:
  todos - Field 'completed' is filterable but not indexed
    CREATE INDEX idx_todos_completed ON todos (completed);

Copy and paste the SQL commands above to create missing indexes
```

## Testing

`crudcrate` includes a comprehensive test suite with multi-database support to ensure compatibility across different database backends.

### Quick Testing

```bash
# Run all tests with SQLite (default, fastest)
cargo test

# Run specific test categories
cargo test --test crud_integration_test
cargo test --test fulltext_search_test
cargo test --test index_analysis_test
```

### Multi-Database Testing

Test your application against multiple database backends to ensure compatibility:

> **Note**: If you encounter test failures with PostgreSQL/MySQL due to schema conflicts, add `-- --test-threads=1` to run tests sequentially.

```bash
# PostgreSQL testing (requires running PostgreSQL instance)
DATABASE_URL=postgres://postgres:pass@localhost/test_db cargo test

# MySQL testing (requires running MySQL instance)
DATABASE_URL=mysql://root:pass@127.0.0.1:3306/test_db cargo test -- --test-threads=1

# Test specific functionality on PostgreSQL
DATABASE_URL=postgres://postgres:pass@localhost/test_db cargo test --test fulltext_search_test
```

### Database Setup for Testing

#### PostgreSQL

```bash
# Using Docker
docker run --name test-postgres \
  -e POSTGRES_PASSWORD=pass \
  -e POSTGRES_DB=test_db \
  -p 5432:5432 -d postgres:16

# Run tests
DATABASE_URL=postgres://postgres:pass@localhost/test_db cargo test

# Cleanup
docker stop test-postgres && docker rm test-postgres
```

#### MySQL

```bash
# Using Docker
docker run --name test-mysql \
  -e MYSQL_ROOT_PASSWORD=pass \
  -e MYSQL_DATABASE=test_db \
  -p 3306:3306 -d mysql:8

# Wait for MySQL to fully initialize (important!)
echo "Waiting for MySQL to initialize..."
sleep 20

# Verify connection works
mysql -h 127.0.0.1 -P 3306 -u root -ppass -e "SELECT 1;" test_db

# Run tests (use single thread to avoid migration conflicts)
DATABASE_URL=mysql://root:pass@127.0.0.1:3306/test_db cargo test -- --test-threads=1

# Cleanup
docker stop test-mysql && docker rm test-mysql
```

> **Note**: MySQL requires additional startup time and single-threaded testing to avoid migration conflicts. The `sleep 20` ensures MySQL is fully ready, and `--test-threads=1` prevents race conditions with database migrations.

### Test Categories

The test suite covers multiple areas:

- **Integration Tests**: Full HTTP API testing with realistic scenarios
- **CRUD Operations**: Create, Read, Update, Delete functionality
- **Filtering & Search**: Query parameter handling and fulltext search
- **Data Type Tests**: Comprehensive coverage of all Rust/SQL data types
- **Edge Cases**: Error handling, malformed requests, boundary conditions
- **Performance**: Response time and throughput characteristics
- **Security**: SQL injection prevention and input validation
- **Multi-Database**: Compatibility across SQLite, PostgreSQL, and MySQL

### Database-Specific Features Testing

Some tests demonstrate database-specific optimizations:

```bash
# Test PostgreSQL GIN index recommendations
DATABASE_URL=postgres://postgres:pass@localhost/test_db cargo test test_fulltext_columns_recommendations -- --nocapture

# Test SQLite fallback behavior
cargo test test_fulltext_search_sqlite_fallback

# Test MySQL fulltext capabilities
DATABASE_URL=mysql://root:pass@127.0.0.1:3306/test_db cargo test test_fulltext_search_with_different_data_types -- --test-threads=1
```

### Running Benchmarks

```bash
# SQLite benchmarks (default)
cargo bench --bench crud_benchmarks

# PostgreSQL benchmarks (requires Docker)
docker run --name benchmark-postgres -e POSTGRES_PASSWORD=pass -e POSTGRES_DB=benchmark -p 5432:5432 -d postgres:16
BENCHMARK_DATABASE_URL=postgres://postgres:pass@localhost/benchmark cargo bench --bench crud_benchmarks
docker stop benchmark-postgres && docker rm benchmark-postgres
```

## Examples

- **[Minimal Example](https://github.com/evanjt/crudcrate-example-minimal)**: Complete API in 60 lines
- **[Full Example](https://github.com/evanjt/crudcrate-example)**: Production-ready implementation

### Debug Generated Code

See exactly what code the macros generate with colourised, formatted output:

```bash
cargo run --example minimal_debug --features=debug
```

Add `debug_output` to any `EntityToModels` struct to see the generated API structs, CRUD implementations, and router code.

## Detailed Documentation

### Entity Generation Explained

The `EntityToModels` macro analyzes your Sea-ORM entity and generates three main structures:

1. **API Struct**: A clean representation of your data for API responses
2. **Create Model**: Optimized for POST requests, excluding auto-generated fields
3. **Update Model**: Uses `Option<Option<T>>` pattern to distinguish between "don't update this field" (`None`) and "set this field to null" (`Some(None)`)

```rust
// Your entity
#[derive(EntityToModels)]
#[crudcrate(api_struct = "Todo")]
pub struct Model {
    pub id: Uuid,           // Excluded from Create model automatically
    pub title: String,      // Required in Create, optional in Update
    pub completed: bool,    // Required in Create, optional in Update
}

// Generated structures:
pub struct Todo {         // API response struct
    pub id: Uuid,
    pub title: String,
    pub completed: bool,
}

pub struct TodoCreate {   // POST request body
    pub title: String,
    pub completed: bool,
    // id excluded automatically
}

pub struct TodoUpdate {   // PUT request body
    pub title: Option<String>,              // Some("new") = update, None = don't change
    pub completed: Option<Option<bool>>,    // Some(Some(true)) = set true, Some(None) = set null, None = don't change
}
```

### Advanced Field Control

Field attributes give you precise control over how each field behaves in different contexts:

#### Core Attributes

```rust
#[crudcrate(
    primary_key,                    // Marks this field as the primary identifier (only one per struct)
    filterable,                     // Enables filtering: ?filter={"status":"active"}
    sortable,                       // Enables sorting: ?sort=created_at&order=DESC
    fulltext,                       // Includes in fulltext search: ?filter={"q":"search terms"}
)]
```

#### Model Generation Control

```rust
#[crudcrate(
    create_model = false,           // Excludes from TodoCreate struct (default: true)
    update_model = false,           // Excludes from TodoUpdate struct (default: true)
)]
```

#### Auto-Generation

```rust
#[crudcrate(
    on_create = Uuid::new_v4(),     // Expression to run on create operations
    on_update = Utc::now(),         // Expression to run on update operations
)]
```

#### Non-Database Fields

```rust
#[crudcrate(
    non_db_attr = true,             // Field not in database (default: false)
    default = vec![],               // Default value for non-DB fields
                                    // Requires #[sea_orm(ignore)] when using DeriveEntityModel
)]
```

#### Type-Specific Attributes

```rust
#[crudcrate(
    enum_field,                     // Mark field as enum for filtering (required for enum filtering)
    enum_case_sensitive,            // Enable case-sensitive enum matching (default: case-insensitive)
)]
```

**Enum Field Requirements**: For enum fields to work with filtering, you must explicitly mark them with `enum_field`:

```rust
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "products")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,

    #[crudcrate(filterable, enum_field)]  // ← Required for enum filtering
    pub category: ProductCategory,        // Sea-ORM enum type
}
```

This enables case-insensitive enum filtering where users can search for "elec" to find "Electronics".

#### Struct-Level Attributes

Applied to the entire struct:

```rust
#[crudcrate(
    api_struct = "TodoItem",        // Override API struct name (default: table name in PascalCase)
    name_singular = "todo",         // Resource name singular (default: table name)
    name_plural = "todos",          // Resource name plural (default: singular + "s")
    description = "Manages todos",  // Resource description for OpenAPI docs
    generate_router,                // Auto-generate router function
    fulltext_language = "simple",   // PostgreSQL fulltext language (default: "english")

    // Function injection - override default CRUD operations
    fn_get_one = self::custom_get_one,       // Custom get_one function
    fn_get_all = self::custom_get_all,       // Custom get_all function
    fn_create = self::custom_create,         // Custom create function
    fn_update = self::custom_update,         // Custom update function
    fn_delete = self::custom_delete,         // Custom delete function
    fn_delete_many = self::custom_delete_many, // Custom batch delete function
)]
```

### Fulltext Search Architecture

Fulltext search automatically optimizes based on your database backend:

**PostgreSQL**: Uses native `tsvector` and `plainto_tsquery` with GIN indexes for high-performance text search

```sql
-- Generated query for PostgreSQL (with GIN index support)
WHERE to_tsvector('english', title || ' ' || content) @@ plainto_tsquery('english', 'search terms')

-- Recommended index for optimal performance
CREATE INDEX idx_posts_fulltext ON posts USING GIN (to_tsvector('english', title || ' ' || content));
```

**Language Configuration**: Configure fulltext language per model for internationalization:

```rust
#[crudcrate(fulltext_language = "simple")]    // Language-agnostic
#[crudcrate(fulltext_language = "spanish")]   // Spanish text processing
#[crudcrate(fulltext_language = "french")]    // French text processing
// Default: "english"
```

**SQLite**: Falls back to case-insensitive LIKE queries across all fulltext fields

```sql
-- Generated query for SQLite
WHERE (UPPER(title) LIKE UPPER('%search%') OR UPPER(content) LIKE UPPER('%terms%'))
```

**MySQL**: Uses MATCH AGAINST for fulltext indexes where available

### Security & Production Considerations

`crudcrate` includes built-in protection against common vulnerabilities:

- **SQL Injection Prevention**: All user input is parameterized through Sea-ORM
- **Input Validation**: Field names and values are validated before query construction
- **Query Sanitization**: Search terms are escaped and sanitized automatically

For production deployments, add these security layers:

```rust
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use axum_helmet::Helmet;

let app = Router::new()
    .nest("/api", your_crud_routes)
    .layer(Helmet::default())           // Security headers
    .layer(TraceLayer::new_for_http())  // Request logging
    .layer(CorsLayer::permissive());    // CORS (configure for production)
```

### Performance Characteristics

`crudcrate` is optimized for high-throughput applications:

- **Sub-millisecond responses**: Most operations complete in 200-300µs
- **Database connection pooling**: Leverages Sea-ORM's efficient connection management
- **Query optimization**: Generates efficient SQL with proper indexing hints
- **Minimal allocations**: Zero-copy deserialization where possible

Benchmark your setup:

```bash
# Quick SQLite benchmark
cargo bench --bench crud_benchmarks -- --verbose

# Compare SQLite vs PostgreSQL performance
docker run --name benchmark-postgres -e POSTGRES_PASSWORD=pass -e POSTGRES_DB=benchmark -p 5432:5432 -d postgres:16
BENCHMARK_DATABASE_URL=postgres://postgres:pass@localhost/benchmark cargo bench --bench crud_benchmarks -- --verbose
docker stop benchmark-postgres && docker rm benchmark-postgres
```

**Performance Differences**:

- **SQLite**: Faster for small datasets (~400µs fulltext search), no network overhead, ideal for development
- **PostgreSQL**: Better for production with proper GIN indexes (~2-100ms), scales better with dataset size and concurrent users
- **Network Impact**: PostgreSQL has network latency but superior concurrent performance
- **Indexing**: PostgreSQL supports advanced fulltext search with `tsvector` and ranking

### React Admin Integration

`crudcrate` follows React Admin's REST conventions out of the box:

```javascript
// React Admin automatically understands these endpoints:
GET    /api/todos                    // List with pagination
GET    /api/todos?filter={"completed":false}  // Filtered list
GET    /api/todos/123                // Get one
POST   /api/todos                    // Create
PUT    /api/todos/123                // Update
DELETE /api/todos/123                // Delete

// Pagination parameters
GET /api/todos?page=0&per_page=25

// Sorting parameters
GET /api/todos?sort=created_at&order=DESC

// Complex filtering
GET /api/todos?filter={"title":"urgent","completed":false}
```

### Custom Function Injection

Override default CRUD behavior with your own implementations:

```rust
#[crudcrate(fn_get_one = custom_get_todo)]
pub struct Model { /* ... */ }

async fn custom_get_todo(db: &DatabaseConnection, id: Uuid) -> Result<Todo, DbErr> {
    // Add custom logic: permissions, caching, audit trails, etc.
    let todo = Entity::find_by_id(id)
        .filter(Column::UserId.eq(current_user_id()))  // Permission check
        .one(db)
        .await?;

    // Log access for audit trail
    audit::log_access("todo", id, current_user_id()).await;

    todo.ok_or(DbErr::RecordNotFound("Todo not found"))
}
```

### Migration Integration

`crudcrate` works seamlessly with Sea-ORM's migration system:

```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Todo::Table)
                    .col(ColumnDef::new(Todo::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Todo::Title).string().not_null())
                    .col(ColumnDef::new(Todo::Completed).boolean().not_null().default(false))
                    .to_owned(),
            )
            .await
    }
}
```

## AI Disclosure

Development of `crudcrate` and `crudcrate-derive` has occasionally been powered by the questionable wisdom of large language models. They have been consulted for prototyping, code suggestions, test generation, and the overuse of emojis in documentation. This has resulted in perhaps more verbose and less optimal implementations.

If you find this project useful and have a way to improve it, please help defeat the bots by contributing! 🤓

## License & Disclaimer

**MIT License**. See [LICENSE](./LICENSE) for details.

**Disclaimer**: This software is provided "as is" without warranty of any kind. **This library is in heavy development - use at your own risk.** While `crudcrate` includes security measures for CRUD operations, users are responsible for implementing comprehensive security appropriate for their specific use case and environment.

## Related Crates

- **[sea-orm](https://crates.io/crates/sea-orm)**: Database ORM and query builder
- **[axum](https://crates.io/crates/axum)**: Web application framework
- **[utoipa](https://crates.io/crates/utoipa)**: OpenAPI documentation generation
- **[serde](https://crates.io/crates/serde)**: Serialization framework
- **[tower-http](https://crates.io/crates/tower-http)**: HTTP middleware for production security
- **[tower_governor](https://crates.io/crates/tower_governor)**: Rate limiting middleware
