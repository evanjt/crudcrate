# The Entity Model

Your Sea-ORM entity is the foundation. CRUDCrate extends it with attributes to control API behavior.

## Anatomy of a CRUDCrate Entity

```rust
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(
    Clone,              // Required by Sea-ORM
    Debug,              // Useful for debugging
    PartialEq,          // Optional, for comparisons
    DeriveEntityModel,  // Sea-ORM: generates Entity, Column, etc.
    Serialize,          // For JSON responses
    Deserialize,        // For JSON requests
    EntityToModels,     // CRUDCrate: generates CRUD infrastructure
)]
#[crudcrate(generate_router)]       // CRUDCrate struct-level attribute
#[sea_orm(table_name = "products")] // Sea-ORM table mapping
pub struct Model {
    // Primary key - required
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    // Regular field with filtering
    #[crudcrate(filterable, sortable)]
    pub name: String,

    // Optional field
    pub description: Option<String>,

    // Excluded from responses
    #[crudcrate(exclude(one, list))]
    pub internal_notes: String,

    // Auto-managed timestamps
    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTimeUtc,
}

// Required by Sea-ORM
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

## Required Derives

| Derive | Purpose | Required? |
|--------|---------|-----------|
| `Clone` | Sea-ORM requirement | Yes |
| `Debug` | Debugging output | Recommended |
| `DeriveEntityModel` | Sea-ORM entity generation | Yes |
| `Serialize` | JSON response serialization | Yes |
| `Deserialize` | JSON request deserialization | Yes |
| `EntityToModels` | CRUDCrate generation | Yes |

## Naming Conventions

CRUDCrate follows these naming conventions:

| Your Code | Generated |
|-----------|-----------|
| `pub struct Model` | `Product` (from table name) |
| Table: `products` | Response: `Product` |
| | Create: `ProductCreate` |
| | Update: `ProductUpdate` |
| | List: `ProductList` |
| | Router: `product_router()` |

### Custom Naming

Override with `api_struct`:

```rust
#[crudcrate(api_struct = "Item")]
pub struct Model {
    // ...
}

// Generates: Item, ItemCreate, ItemUpdate, ItemList
```

## Field Types

CRUDCrate handles common Rust and Sea-ORM types:

### Scalar Types

```rust
pub id: i32,           // Integers
pub id: i64,
pub price: f64,        // Floats
pub name: String,      // Strings
pub active: bool,      // Booleans
pub id: Uuid,          // UUIDs
```

### Optional Types

```rust
pub description: Option<String>,  // Nullable in DB
pub parent_id: Option<i32>,       // Optional foreign key
```

### Date/Time Types

```rust
use sea_orm::prelude::*;

pub created_at: DateTime,           // Without timezone
pub updated_at: DateTimeUtc,        // With UTC timezone
pub deleted_at: Option<DateTimeUtc>, // Soft delete
```

### Enums

```rust
#[derive(EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum Status {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "archived")]
    Archived,
}

// In your entity:
#[crudcrate(filterable)]
pub status: Status,
```

### JSON Types

```rust
use sea_orm::prelude::Json;

pub metadata: Json,                 // Arbitrary JSON
pub tags: Vec<String>,              // JSON array (with proper DB type)
```

## Primary Key Configuration

Every entity needs exactly one primary key:

```rust
// Integer auto-increment
#[sea_orm(primary_key)]
#[crudcrate(primary_key, exclude(create, update))]
pub id: i32,

// UUID (recommended)
#[sea_orm(primary_key, auto_increment = false)]
#[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
pub id: Uuid,

// String ID
#[sea_orm(primary_key, auto_increment = false)]
#[crudcrate(primary_key)]
pub slug: String,  // Client provides the ID
```

## Relationships

Define relationships for join loading:

```rust
// In your entity file
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::comment::Entity")]
    Comments,

    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::AuthorId",
        to = "super::user::Column::Id"
    )]
    Author,
}

// Implement Related trait
impl Related<super::comment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Comments.def()
    }
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Author.def()
    }
}

// In your Model struct, add join fields
#[sea_orm(ignore)]
#[crudcrate(non_db_attr, join(one, all))]
pub comments: Vec<Comment>,

#[sea_orm(ignore)]
#[crudcrate(non_db_attr, join(one))]
pub author: Option<User>,
```

## Complete Example

```rust
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum PostStatus {
    #[sea_orm(string_value = "draft")]
    Draft,
    #[sea_orm(string_value = "published")]
    Published,
    #[sea_orm(string_value = "archived")]
    Archived,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, EntityToModels)]
#[crudcrate(generate_router, name_singular = "post", name_plural = "posts")]
#[sea_orm(table_name = "posts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable, fulltext)]
    pub title: String,

    #[crudcrate(fulltext)]
    pub content: String,

    pub excerpt: Option<String>,

    #[crudcrate(filterable)]
    pub status: PostStatus,

    #[crudcrate(filterable)]
    pub author_id: Uuid,

    #[crudcrate(sortable, filterable)]
    pub published_at: Option<DateTimeUtc>,

    #[crudcrate(sortable, exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTimeUtc,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
    pub updated_at: DateTimeUtc,

    // Relationships
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all, depth = 1))]
    pub author: Option<super::user::User>,

    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one))]
    pub comments: Vec<super::comment::Comment>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::AuthorId",
        to = "super::user::Column::Id"
    )]
    Author,

    #[sea_orm(has_many = "super::comment::Entity")]
    Comments,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Author.def()
    }
}

impl Related<super::comment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Comments.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
```

## Next Steps

- Learn about [Generated Models](./generated-models.md)
- Understand the [Attribute System](./attributes.md)
- Configure [Relationships](../features/relationships.md)
