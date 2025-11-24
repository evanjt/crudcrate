# Attribute System

CRUDCrate uses attributes to configure code generation. This guide covers all available attributes.

## Attribute Syntax

Attributes use the `#[crudcrate(...)]` syntax:

```rust
// Struct-level
#[crudcrate(generate_router, api_struct = "Item")]
pub struct Model {
    // Field-level
    #[crudcrate(primary_key, filterable, sortable)]
    pub id: i32,
}
```

## Struct-Level Attributes

Applied to the struct definition:

### `generate_router`

Generates an Axum router function:

```rust
#[crudcrate(generate_router)]
pub struct Model { }

// Generates:
pub fn model_router() -> Router { }
```

### `api_struct`

Override the generated struct name:

```rust
#[crudcrate(api_struct = "Product")]
pub struct Model { }

// Generates: Product, ProductCreate, ProductUpdate, ProductList
// Instead of: Model, ModelCreate, etc.
```

### `name_singular` / `name_plural`

Override resource names (for routing and Content-Range headers):

```rust
#[crudcrate(name_singular = "person", name_plural = "people")]
pub struct Model { }

// Routes: /people, /people/:id
// Content-Range: people 0-9/100
```

### `operations`

Specify a custom CRUDOperations implementation:

```rust
#[crudcrate(operations = MyOperations)]
pub struct Model { }

// MyOperations must implement CRUDOperations
```

### `description`

Add OpenAPI description:

```rust
#[crudcrate(description = "User accounts for the application")]
pub struct Model { }
```

### `fulltext_language`

Set fulltext search language (PostgreSQL):

```rust
#[crudcrate(fulltext_language = "spanish")]
pub struct Model { }

// Uses Spanish stemming for fulltext search
```

## Field-Level Attributes

Applied to individual fields:

### `primary_key`

Marks the primary key field:

```rust
#[crudcrate(primary_key)]
pub id: i32,

// Required: exactly one field must have this
```

### `exclude(...)`

Exclude field from specific models:

```rust
// Exclude from single model
#[crudcrate(exclude(create))]
pub id: i32,

// Exclude from multiple models
#[crudcrate(exclude(one, list))]
pub password: String,

// Available targets:
// - one: Response model (GET /items/:id)
// - create: Create model (POST /items)
// - update: Update model (PUT /items/:id)
// - list: List model (GET /items)
```

### `filterable`

Enable filtering on this field:

```rust
#[crudcrate(filterable)]
pub status: String,

// Allows: GET /items?filter={"status":"active"}
// Allows: GET /items?status_eq=active
// Allows: GET /items?status_ne=inactive
```

### `sortable`

Enable sorting on this field:

```rust
#[crudcrate(sortable)]
pub created_at: DateTimeUtc,

// Allows: GET /items?sort=["created_at","DESC"]
// Allows: GET /items?sort=created_at&order=desc
```

### `fulltext`

Include in fulltext search:

```rust
#[crudcrate(fulltext)]
pub title: String,

#[crudcrate(fulltext)]
pub description: String,

// Allows: GET /items?q=search terms
// Searches across all fulltext fields
```

### `on_create`

Default value when creating:

```rust
#[crudcrate(on_create = Uuid::new_v4())]
pub id: Uuid,

#[crudcrate(on_create = chrono::Utc::now())]
pub created_at: DateTimeUtc,

#[crudcrate(on_create = "pending".to_string())]
pub status: String,

// Expression is evaluated at insert time
```

### `on_update`

Default value when updating:

```rust
#[crudcrate(on_update = chrono::Utc::now())]
pub updated_at: DateTimeUtc,

// Expression is evaluated on every update
```

### `non_db_attr`

Marks non-database fields (for relationships):

```rust
#[sea_orm(ignore)]  // Sea-ORM: ignore in queries
#[crudcrate(non_db_attr)]  // CRUDCrate: not a DB column
pub related_items: Vec<Item>,
```

### `join(...)`

Configure relationship loading:

```rust
// Load in get_one only
#[crudcrate(non_db_attr, join(one))]
pub comments: Vec<Comment>,

// Load in both get_one and get_all
#[crudcrate(non_db_attr, join(one, all))]
pub author: Option<User>,

// Limit recursion depth
#[crudcrate(non_db_attr, join(one, all, depth = 2))]
pub nested: Vec<Nested>,

// Full syntax
join(
    one,          // Include in get_one response
    all,          // Include in get_all response
    depth = 3,    // Max recursion depth (default: unlimited up to 5)
)
```

## Lifecycle Hooks

Hook into CRUD operations:

```rust
#[crudcrate(
    // Pre-operation hooks (before DB operation)
    create::one::pre = validate_create,
    update::one::pre = validate_update,
    delete::one::pre = check_delete_permission,

    // Post-operation hooks (after DB operation)
    create::one::post = send_welcome_email,
    update::one::post = invalidate_cache,
    delete::one::post = cleanup_related,

    // Full body replacement
    create::one::body = custom_create_handler,
)]
pub struct Model { }

// Hook function signatures:
async fn validate_create(db: &DatabaseConnection, data: &mut ModelCreate) -> Result<(), ApiError>;
async fn send_welcome_email(db: &DatabaseConnection, created: &Model) -> Result<(), ApiError>;
async fn custom_create_handler(db: &DatabaseConnection, data: ModelCreate) -> Result<Model, ApiError>;
```

## Combining Attributes

Multiple attributes on one field:

```rust
#[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
pub id: Uuid,

#[crudcrate(filterable, sortable, fulltext)]
pub title: String,

#[crudcrate(non_db_attr, join(one, all, depth = 1))]
pub author: Option<User>,
```

## Complete Example

```rust
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, DeriveEntityModel, Serialize, Deserialize, EntityToModels)]
#[crudcrate(
    generate_router,
    api_struct = "Article",
    name_singular = "article",
    name_plural = "articles",
    operations = ArticleOperations,
    create::one::post = notify_subscribers,
)]
#[sea_orm(table_name = "articles")]
pub struct Model {
    // Primary key with auto-generation
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    // Searchable, filterable, sortable title
    #[crudcrate(filterable, sortable, fulltext)]
    pub title: String,

    // Searchable content, excluded from lists
    #[crudcrate(fulltext, exclude(list))]
    pub content: String,

    // Short summary for lists
    pub summary: Option<String>,

    // Filterable status enum
    #[crudcrate(filterable)]
    pub status: ArticleStatus,

    // Author relationship
    #[crudcrate(filterable)]
    pub author_id: Uuid,

    // Sortable publication date
    #[crudcrate(sortable, filterable)]
    pub published_at: Option<DateTimeUtc>,

    // Auto-managed timestamps
    #[crudcrate(sortable, exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTimeUtc,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
    pub updated_at: DateTimeUtc,

    // Loaded relationships
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one))]
    pub author: Option<User>,

    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one))]
    pub comments: Vec<Comment>,
}
```

## Attribute Reference Table

| Attribute | Level | Description |
|-----------|-------|-------------|
| `generate_router` | Struct | Generate Axum router |
| `api_struct` | Struct | Custom struct name |
| `name_singular` | Struct | Singular resource name |
| `name_plural` | Struct | Plural resource name |
| `operations` | Struct | Custom operations trait |
| `description` | Struct | OpenAPI description |
| `fulltext_language` | Struct | Fulltext search language |
| `primary_key` | Field | Mark as primary key |
| `exclude(...)` | Field | Exclude from models |
| `filterable` | Field | Enable filtering |
| `sortable` | Field | Enable sorting |
| `fulltext` | Field | Include in fulltext search |
| `on_create` | Field | Default value on create |
| `on_update` | Field | Default value on update |
| `non_db_attr` | Field | Non-database field |
| `join(...)` | Field | Relationship loading config |

## Next Steps

- Learn about [Filtering](../features/filtering.md)
- Configure [Relationships](../features/relationships.md)
- Implement [Custom Operations](../advanced/custom-operations.md)
