# Default Values

Automatically set field values on create and update operations.

## Syntax

```rust
// Set on create only
#[crudcrate(on_create = expression)]
pub field: Type,

// Set on update only
#[crudcrate(on_update = expression)]
pub field: Type,

// Set on both
#[crudcrate(on_create = expr1, on_update = expr2)]
pub field: Type,
```

## Common Patterns

### UUID Primary Keys

```rust
#[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
pub id: Uuid,
```

### Timestamps

```rust
// Created timestamp - set once
#[crudcrate(exclude(create, update), on_create = chrono::Utc::now())]
pub created_at: DateTimeUtc,

// Updated timestamp - set on every change
#[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
pub updated_at: DateTimeUtc,
```

### Default Status

```rust
#[crudcrate(on_create = Status::Pending)]
pub status: Status,

#[crudcrate(on_create = "draft".to_string())]
pub status: String,
```

### Counters

```rust
#[crudcrate(exclude(create, update), on_create = 0)]
pub view_count: i32,

#[crudcrate(exclude(create, update), on_create = 0.0)]
pub rating: f64,
```

### Boolean Flags

```rust
#[crudcrate(on_create = false)]
pub is_verified: bool,

#[crudcrate(on_create = true)]
pub is_active: bool,
```

## Expression Types

### Literals

```rust
#[crudcrate(on_create = 0)]
pub count: i32,

#[crudcrate(on_create = "default")]
pub category: String,

#[crudcrate(on_create = true)]
pub active: bool,
```

### Function Calls

```rust
#[crudcrate(on_create = Uuid::new_v4())]
pub id: Uuid,

#[crudcrate(on_create = chrono::Utc::now())]
pub created_at: DateTimeUtc,
```

### Method Chains

```rust
#[crudcrate(on_create = "pending".to_string())]
pub status: String,

#[crudcrate(on_create = Vec::new())]
pub tags: Vec<String>,
```

### Enum Variants

```rust
#[crudcrate(on_create = Status::Pending)]
pub status: Status,

#[crudcrate(on_create = Priority::Normal)]
pub priority: Priority,
```

### Constants

```rust
const DEFAULT_LIMIT: i32 = 100;

#[crudcrate(on_create = DEFAULT_LIMIT)]
pub rate_limit: i32,
```

## Complete Example

```rust
use chrono::{DateTime, Utc};
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum ArticleStatus {
    #[sea_orm(string_value = "draft")]
    Draft,
    #[sea_orm(string_value = "review")]
    Review,
    #[sea_orm(string_value = "published")]
    Published,
    #[sea_orm(string_value = "archived")]
    Archived,
}

#[derive(Clone, Debug, DeriveEntityModel, Serialize, Deserialize, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "articles")]
pub struct Model {
    // Auto-generated UUID
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable, fulltext)]
    pub title: String,

    pub content: String,

    // Default to Draft status
    #[crudcrate(filterable, on_create = ArticleStatus::Draft)]
    pub status: ArticleStatus,

    // Start with zero views
    #[crudcrate(sortable, exclude(create, update), on_create = 0)]
    pub view_count: i32,

    // Track creation time
    #[crudcrate(sortable, exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTime<Utc>,

    // Track all modifications
    #[crudcrate(
        exclude(create, update),
        on_create = chrono::Utc::now(),
        on_update = chrono::Utc::now()
    )]
    pub updated_at: DateTime<Utc>,

    // Publish date (null until published)
    #[crudcrate(sortable)]
    pub published_at: Option<DateTime<Utc>>,
}
```

## How It Works

### on_create

When creating a new record:

```rust
// User provides
let create_data = ArticleCreate {
    title: "My Article".into(),
    content: "Content here...".into(),
    // status, view_count, created_at, updated_at: not provided
};

// CRUDCrate generates ActiveModel with defaults
let active_model = ActiveModel {
    id: Set(Uuid::new_v4()),           // on_create
    title: Set(create_data.title),
    content: Set(create_data.content),
    status: Set(ArticleStatus::Draft), // on_create
    view_count: Set(0),                // on_create
    created_at: Set(chrono::Utc::now()), // on_create
    updated_at: Set(chrono::Utc::now()), // on_create
    published_at: NotSet,
};
```

### on_update

When updating an existing record:

```rust
// User provides partial update
let update_data = ArticleUpdate {
    title: Some("Updated Title".into()),
    content: None,  // Don't change
    // ...
};

// CRUDCrate applies updates + on_update defaults
active_model.title = Set("Updated Title".into());
active_model.updated_at = Set(chrono::Utc::now());  // on_update
// Other fields: unchanged
```

## Combining with exclude

Common pattern: exclude from client input, provide default

```rust
// Client can't set, but has default
#[crudcrate(exclude(create, update), on_create = Uuid::new_v4())]
pub id: Uuid,

// Client can't set these timestamps
#[crudcrate(exclude(create, update), on_create = chrono::Utc::now())]
pub created_at: DateTimeUtc,
```

## Client Override

If a field is NOT excluded, client can override the default:

```rust
// Default status, but client CAN override
#[crudcrate(on_create = ArticleStatus::Draft)]
pub status: ArticleStatus,
```

```bash
# Uses default (Draft)
POST /articles
{"title": "My Article", "content": "..."}

# Client overrides default
POST /articles
{"title": "My Article", "content": "...", "status": "published"}
```

To prevent client override, combine with `exclude`:

```rust
// Always Draft on create, client cannot override
#[crudcrate(exclude(create), on_create = ArticleStatus::Draft)]
pub status: ArticleStatus,
```

## Complex Defaults with Operations

For complex default logic, use `CRUDOperations`:

```rust
pub struct ArticleOperations;

impl CRUDOperations for ArticleOperations {
    type Resource = Article;

    async fn before_create(
        &self,
        db: &DatabaseConnection,
        data: &mut ArticleCreate,
    ) -> Result<(), ApiError> {
        // Complex default logic
        if data.slug.is_none() {
            data.slug = Some(slugify(&data.title));
        }

        // Validate uniqueness
        if slug_exists(db, &data.slug.as_ref().unwrap()).await? {
            data.slug = Some(format!("{}-{}", data.slug.unwrap(), Uuid::new_v4()));
        }

        Ok(())
    }
}
```

## Database Defaults vs CRUDCrate Defaults

| Feature | Database Default | CRUDCrate Default |
|---------|------------------|-------------------|
| Where | SQL schema | Rust code |
| When | DB insert time | Before insert |
| Visibility | Not in API model | Part of workflow |
| Complexity | Limited SQL | Full Rust |

Use database defaults for:
- Simple values
- Database-specific functions
- Constraints

Use CRUDCrate defaults for:
- Rust-generated values (UUIDs)
- Complex logic
- Values visible in API flow

## Next Steps

- Set up [Error Handling](./error-handling.md)
- Learn about [Relationships](./relationships.md)
- Implement [Custom Operations](../advanced/custom-operations.md)
