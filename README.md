# crudcrate

[![Tests](https://github.com/evanjt/crudcrate/actions/workflows/test.yml/badge.svg)](https://github.com/evanjt/crudcrate/actions/workflows/test.yml)
[![codecov](https://codecov.io/gh/evanjt/crudcrate/branch/main/graph/badge.svg)](https://codecov.io/gh/evanjt/crudcrate)
[![Crates.io](https://img.shields.io/crates/v/crudcrate.svg)](https://crates.io/crates/crudcrate)
[![Documentation](https://docs.rs/crudcrate/badge.svg)](https://docs.rs/crudcrate)

**`crudcrate`** provides procedural macros that reduce CRUD API boilerplate in Rust. It seamlessly integrates with **[SeaORM](https://crates.io/crates/sea-orm)** for database interactions and **[Axum](https://crates.io/crates/axum)** for building web services.

🚀 **NEW**: The `EntityToModels` macro now generates complete CRUD APIs directly from your Sea-ORM entities with **function injection** support for custom logic!

⚡ **Ultra-Fast Development**: Create a complete CRUD API with OpenAPI documentation in **under 60 lines** of code (including imports and server setup)!

---

## 📚 Table of Contents

- [Features](#✨-features)
- [Installation](#🚀-installation)
- [Quick Start](#⚡-quick-start)
- [EntityToModels Macro (Recommended)](#🎯-entitytomodels-macro-recommended)
  - [Basic Usage](#basic-usage)
  - [All Available Attributes](#all-available-attributes)
  - [Function Injection](#function-injection)
  - [Complete Example](#complete-example)
- [Traditional Approach](#🔧-traditional-approach)
- [CRUD Handlers](#✅-crud-handlers)
- [Query Specifications](#📋-query-specifications)
  - [Filtering](#🔍-filtering)
  - [Sorting](#📊-sorting)
  - [Pagination](#📄-pagination)
  - [React Admin Compatibility](#🌐-react-admin-compatibility)
- [License](#📜-license-and-disclaimer)

---

## ✨ Features

- **⚡ Ultra-Minimal Setup**: Complete CRUD API in under 60 lines of code (no migrations required!)
- **🎯 EntityToModels Macro**: Generate complete CRUD APIs from Sea-ORM entities.
- **🚀 Auto-Router Generation**: Single `generate_router` attribute eliminates all router boilerplate.
- **🔧 Function Injection**: Override any CRUD operation with custom business logic.
- **📊 Smart Defaults**: Auto-generate primary keys, timestamps, and resource metadata.
- **🔍 Sortable/Filterable**: Built-in support for sorting and filtering columns.
- **📖 OpenAPI Integration**: Automatic API documentation through `Utoipa`.
- **🌐 React-Admin Compatible**: Built-in pagination, sorting, and filtering.
- **⚡ Ultimate Convenience**: 95% less boilerplate while maintaining full flexibility.

---

## 🚀 Installation

```bash
cargo add crudcrate
```

or,

```toml
// Cargo.toml

[dependencies]
crudcrate = "0.4.0"
```

---

## ⚡ Quick Start

Transform this verbose manual approach:

📝 BEFORE: Manual structs

```rust
#[derive(ToSchema, Serialize, Deserialize)]
pub struct Todo { /* manual field definitions */ }

#[derive(ToSchema, Serialize, Deserialize)]
pub struct TodoCreate { /* manual create fields */ }

#[derive(ToSchema, Serialize, Deserialize)]
pub struct TodoUpdate { /* manual update fields with double-Option */ }

impl From<TodoCreate> for ActiveModel { /* manual conversion */ }
impl TodoUpdate {
    pub fn merge_into_activemodel(/* manual merge logic */) { /* ... */ }
}

#[async_trait]
impl CRUDResource for Todo {
    // Manual trait implementation (50+ lines)
}
```


Into this single macro (SeaORM generated entity model with some additions):

✨ AFTER: EntityToModels macro

```rust
use chrono::{DateTime, Utc};
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, EntityToModels)]
#[sea_orm(table_name = "todos")]
#[crudcrate(description = "Manages todo items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, sortable, create_model = false, update_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,
    #[crudcrate(sortable, filterable)]
    pub title: String,
    #[crudcrate(filterable, on_create = false)]
    pub completed: bool,
    #[crudcrate(sortable, create_model = false, update_model = false, on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
    pub last_updated: DateTime<Utc>,
}
```



**That's it!** This generates:
- `Todo` API struct with all fields
- `TodoCreate` and `TodoUpdate` models
  - The fields we choose for the DB may not be desired for creation (such as ID), or update (such as a field storing the last updated timestamp).
- Complete `CRUDResource` implementation with documented get, create, update, and delete operations.
- Sortable/filterable column definitions.
- Complete OpenAPI documentation with [Utoipa](https://crates.io/crates/utoipa) and can be used with UIs such as [Scalar](https://scalar.com/).

---

## 🎯 EntityToModels Macro (Recommended)

The `EntityToModels` macro is the **ultimate boilerplate reducer**. It generates complete CRUD APIs directly from your Sea-ORM entity definitions.

### Basic Usage

See the Quick Start example above for basic usage.

### All Available Attributes

#### 📋 Struct-Level Attributes (all optional)

```rust
#[crudcrate(
    api_struct = "TodoItem",        // Override API struct name (default: table name in PascalCase)
    name_singular = "todo",         // Resource name singular (default: table name)
    name_plural = "todos",          // Resource name plural (default: singular + "s")
    description = "Manages todos",  // Resource description for docs

    // 🔧 Function injection to override builtin CRUD operations
    fn_get_one = self::custom_get_one,       // Custom get_one function
    fn_get_all = self::custom_get_all,       // Custom get_all function
    fn_create = self::custom_create,         // Custom create function
    fn_update = self::custom_update,         // Custom update function
    fn_delete = self::custom_delete,         // Custom delete function
    fn_delete_many = self::custom_delete_many, // Custom batch delete function
)]
```

#### 🏷️ Field-Level Attributes

```rust
#[crudcrate(
    // 🎯 CRUDResource Generation
    primary_key,                     // Mark as primary key (only one allowed)
    sortable,                        // Include in sortable_columns()
    filterable,                      // Include in filterable_columns()

    // 📝 Create/Update Model Control
    create_model = false,            // Exclude from Create model (default: true)
    update_model = false,            // Exclude from Update model (default: true)

    // ⚡ Auto-Generation
    on_create = Uuid::new_v4(),      // Expression to run on create
    on_update = chrono::Utc::now(),  // Expression to run on update

    // 💾 Non-Database Fields
    non_db_attr = true,              // Field not in database (default: false)
    default = vec![],                // Default value for non-DB fields
                                     // ⚠️  Requires #[sea_orm(ignore)] when using DeriveEntityModel

    // 🚀 Router Generation
    generate_router,                 // Auto-generate router function (no parameters needed!)
)]
```

### Non-Database Fields (Enhanced API Models)

Add fields to your API that don't exist in the database for computed values, metadata, or auxiliary data:

```rust
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, EntityToModels)]
#[sea_orm(table_name = "todo")]
#[crudcrate(description = "Manages todo items", generate_router)]
pub struct Model {
    #[sea_orm(primary_key)]
    #[crudcrate(primary_key, create_model = false, update_model = false)]
    pub id: Uuid,
    pub title: String,

    // Non-database field - excluded from DB but included in API
    #[sea_orm(ignore)]    // ← Required: tells Sea-ORM to skip this field
    #[crudcrate(          // ← Includes in API with default value
        non_db_attr = true,
        default = vec![]
    )]
    pub tags: Vec<String>,

    // Another example: computed field
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr = true, default = 0)]
    pub comment_count: i32,
}
```

> **⚠️ Important**: When using non-DB fields, you'll typically need to implement custom endpoint overrides (at minimum for `get_one` and likely `update`) to populate or handle these fields. See [Function Injection](#function-injection) below.

### Automatic Router Generation

The `generate_router` attribute completely eliminates router boilerplate by automatically generating a `router()` function:

```rust
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, EntityToModels)]
#[sea_orm(table_name = "todos")]
#[crudcrate(
    description = "Simple todo management",
    generate_router  // ← This single attribute generates everything!
)]
pub struct Model {
    #[sea_orm(primary_key)]
    #[crudcrate(primary_key, create_model = false, update_model = false)]
    pub id: Uuid,
    pub title: String,
    pub completed: bool,
}

// Router function is automatically generated - use it like this:
let app = OpenApiRouter::new()
    .nest("/todos", router(&db))  // ← router() function auto-generated!
    .with_state(db.clone());
```

**What gets generated:**
- ✅ Complete `router()` function with all CRUD endpoints
- ✅ All CRUD handlers (`get_one_handler`, `get_all_handler`, etc.)
- ✅ Proper OpenAPI integration with `utoipa_axum::routes!()`
- ✅ Database state management

**Before vs After:**
- **Before**: ~30 lines of router boilerplate per entity
- **After**: 1 attribute (`generate_router`)

### Function Injection

Override any CRUD operation with custom business logic while maintaining all macro benefits:

```rust
#[derive(EntityToModels)]
#[crudcrate(
    description = "Todo management with custom logic",
    fn_get_one = self::get_one_custom_example,
)]
pub struct Model { /* Your DB entity as shown above */ }

// We can define our own custom get_one callback should we wish to override the default implementation
async fn get_one_custom_example(db: &DatabaseConnection, id: Uuid) -> Result<Todo, DbErr> {
    println!("Custom get_one called for id: {id}");

    let todo: Todo = Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(DbErr::RecordNotFound(format!(
            "Todo item with id {id} not found"
        )))?
        .into();

    Ok(todo)
}
```

#### Function signatures for overriding

As CRUDResource generates the struct name, using the above example of `Todo`,
the following struct names match:

- `Todo`: `Self`
- `TodoCreate`: `Self::CreateModel`
- `TodoUpdate`: `Self::UpdateModel`

are based on the struct being `Todo`, and thus the generated create and update
models as `TodoCreate` and `TodoUpdate`.

---

**See the [full trait definitions here](./src/traits.rs).**

```rust
fn_get_one:
    async fn get_one(
        db: &DatabaseConnection,
        id: Uuid
    ) -> Result<Self, DbErr> {}

fn_get_all:
    async fn get_all(
        db: &DatabaseConnection,
        condition: Condition,
        order_column: Self::ColumnType,
        order_direction: Order,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Self>, DbErr> {}

fn_create:
    async fn create(
        db: &DatabaseConnection,
        create_model: Self::CreateModel,
    ) -> Result<Self, DbErr> {}

fn_update:
    async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        update_model: Self::UpdateModel,
    ) -> Result<Self, DbErr> {}

fn_delete:
    async fn delete(
        db: &DatabaseConnection,
        id: Uuid
    ) -> Result<Uuid, DbErr> {}

fn_delete_many:
    async fn delete_many(
        db: &DatabaseConnection,
        ids: Vec<Uuid>
    ) -> Result<Vec<Uuid>, DbErr> {}

```


### Complete Example

## Examples

- **[Minimal Example](https://github.com/evanjt/crudcrate-example-minimal)**: Complete CRUD API in under 60 lines
- **[Full Example](https://github.com/evanjt/crudcrate-example)**: Production-ready API with migrations and advanced features

---

## 🔧 Traditional Approach

If you prefer more control, you can still use the individual macros or none
at all:

### ToCreateModel and ToUpdateModel

```rust
use crudcrate::{ToCreateModel, ToUpdateModel};

#[derive(ToSchema, Serialize, Deserialize, ToUpdateModel, ToCreateModel, Clone)]
#[active_model = "super::db::ActiveModel"]
pub struct Todo {
    #[crudcrate(create_model = false, on_create = Uuid::new_v4())]
    id: Uuid,
    title: String,
    #[crudcrate(on_create = false)]
    completed: bool,
}
```

### Manual CRUDResource Implementation

```rust
// models.rs

#[async_trait]
impl CRUDResource for Todo {
    type EntityType = super::db::Entity;
    type ColumnType = super::db::Column;
    type ActiveModelType = super::db::ActiveModel;
    type CreateModel = TodoCreate;
    type UpdateModel = TodoUpdate;

    const ID_COLUMN: Self::ColumnType = super::db::Column::Id;
    const RESOURCE_NAME_SINGULAR: &'static str = "todo";
    const RESOURCE_NAME_PLURAL: &'static str = "todos";
    const RESOURCE_DESCRIPTION: &'static str = "Todo management API";

    // Any functions that you wish to override from the default (illustrated above)
    fn get_one(db: &DatabaseConnection, id: Uuid) -> Result<Self, DbErr> {
        let todo: Todo = Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound(format!(
                "Todo item with id {id} not found"
            )))?
            .into();

        Ok(todo)
    }

    // get_all, update_one, etc..

    fn sortable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        vec![
            ("id", Self::ColumnType::Id),
            ("title", Self::ColumnType::Title),
            ("last_updated", Self::ColumnType::LastUpdated),
        ]
    }

    fn filterable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        vec![
            ("title", Self::ColumnType::Title),
            ("completed", Self::ColumnType::Completed),
        ]
    }
}
```

---

## ✅ CRUD Handlers

The crud_handlers macro autogenerates the API handlers from your models. Here
you can decide to use the ones generated by the macro, implement your own, or
write completely unrelated endpoints for your API as we are just using the Axum
router with Utoipa for documentation!

```rust
// views.rs

use super::models;
use crudcrate::crud_handlers;
use utoipa_axum::{router::OpenApiRouter, routes};

// Generate all CRUD handlers
crud_handlers!(models::Todo, models::TodoUpdate, models::TodoCreate);

pub fn router(db: &DatabaseConnection) -> OpenApiRouter {
    OpenApiRouter::new()
        .routes(routes!(get_one_handler))     // GET /{id}
        .routes(routes!(get_all_handler))     // GET /
        .routes(routes!(create_one_handler))  // POST /
        .routes(routes!(update_one_handler))  // PUT /{id}
        .routes(routes!(delete_one_handler))  // DELETE /{id}
        .routes(routes!(delete_many_handler)) // DELETE /batch
        .routes(routes!(say_hi_handler))      // GET /hi (example custom route)
        .with_state(db.clone())
}

// Custom route in case you want to add more functionality
#[utoipa::path(
    get,
    path = "/hi",
    responses((status = axum::http::StatusCode::OK)),
)]
async fn say_hi_handler() -> &'static str {
    "Hello 👋"
}


```

## 📋 Query Specifications

crudcrate provides comprehensive query capabilities supporting both standard REST conventions and React Admin compatibility. All list endpoints (`GET /`) support filtering, sorting, and pagination through query parameters.

### 🔍 Filtering

Filters are provided via the `filter` query parameter as a JSON object. The filtering behavior varies by data type:

#### String Fields
String fields support **substring matching** by default (case-sensitive):
```bash
# Finds all records where title contains "Todo"
GET /todos?filter={"title":"Todo"}
```

For **exact matching**, append `_eq` to the field name:
```bash
# Finds only records where title exactly equals "Todo"
GET /todos?filter={"title_eq":"Todo"}
```

#### Numeric Fields (Integer, Float, SmallInt)
Numeric fields support **exact matching** and **comparison operators**:
```bash
# Exact match
GET /todos?filter={"priority":5}

# Comparison operators
GET /todos?filter={"score_gte":10}      # score >= 10
GET /todos?filter={"score_lte":20}      # score <= 20
GET /todos?filter={"score_gt":5}        # score > 5
GET /todos?filter={"score_lt":30}       # score < 30
GET /todos?filter={"score_neq":15}      # score != 15
```

#### Boolean Fields
Boolean fields support exact matching:
```bash
GET /todos?filter={"completed":true}
GET /todos?filter={"is_public":false}
```

#### Enum Fields
Enum fields are **case-insensitive by default** and support exact matching:
```bash
# All of these will match "Urgent" priority
GET /todos?filter={"priority":"urgent"}
GET /todos?filter={"priority":"URGENT"}
GET /todos?filter={"priority":"Urgent"}
```

To enable case-sensitive enum matching, use the `enum_case_sensitive` attribute:
```rust
#[crudcrate(enum_case_sensitive)]
```

#### UUID Fields
UUID fields support exact matching:
```bash
GET /todos?filter={"user_id":"550e8400-e29b-41d4-a716-446655440000"}
```

#### Null Values
Filter for null or non-null values:
```bash
GET /todos?filter={"description":null}           # Find records where description IS NULL
GET /todos?filter={"description_neq":null}       # Find records where description IS NOT NULL
```

#### Special Filters

**Full-text search** across all searchable columns:
```bash
GET /todos?filter={"q":"search term"}
```

**Filter by multiple IDs** (React Admin GetMany):
```bash
GET /todos?filter={"id":["id1","id2","id3"]}
```

#### Combining Filters
Multiple filters are combined with AND logic:
```bash
GET /todos?filter={"completed":false,"priority":"high","title":"Important"}
```

### 📊 Sorting

crudcrate supports two sorting formats:

#### Standard REST Format
```bash
GET /todos?sort=title&order=ASC
GET /todos?sort=created_at&order=DESC
```

#### React Admin Format
```bash
GET /todos?sort=["title","ASC"]
GET /todos?sort=["created_at","DESC"]
```

**Note**: Only columns marked with `#[crudcrate(sortable)]` can be used for sorting.

### 📄 Pagination

crudcrate supports two pagination formats:

#### Standard REST Format
Uses `page` (0-based) and `per_page` parameters:
```bash
GET /todos?page=0&per_page=10    # First 10 items
GET /todos?page=1&per_page=10    # Items 11-20
GET /todos?page=2&per_page=5     # Items 11-15
```

Default: `page=0, per_page=10`

#### React Admin Format
Uses a `range` parameter with `[start, end]` format:
```bash
GET /todos?range=[0,9]     # First 10 items (0-9)
GET /todos?range=[10,19]   # Items 11-20
GET /todos?range=[20,24]   # Items 21-25
```

### 🌐 React Admin Compatibility

crudcrate is designed to work seamlessly with [React Admin's Simple REST Data Provider](https://marmelab.com/react-admin/DataProviderWriting.html#simple-rest).

Key compatibility features:
- **GetList**: Filtering, sorting, and pagination with `Content-Range` headers
- **GetOne**: Direct ID-based retrieval
- **GetMany**: Batch retrieval using `filter={"id":[...]}`
- **Create**: Returns created resource with 201 status
- **Update**: Returns updated resource with 200 status
- **Delete**: Returns 204 No Content on success
- **DeleteMany**: Batch deletion via `DELETE /batch` endpoint

Example React Admin data provider configuration:
```javascript
import simpleRestProvider from 'ra-data-simple-rest';

const dataProvider = simpleRestProvider('http://localhost:8000/api');
```

### 📝 Complete Example

```bash
# Complex query with filtering, sorting, and pagination
GET /todos?filter={"completed":false,"priority":"high","score_gte":50}&sort=created_at&order=DESC&page=0&per_page=20

# This query:
# - Filters for incomplete todos with high priority and score >= 50
# - Sorts by creation date (newest first)
# - Returns the first 20 results
```

### ⚙️ Configuration

Configure query behavior through the EntityToModels macro:

```rust
#[derive(EntityToModels)]
#[crudcrate(
    description = "Todo management",
    enum_case_sensitive,  // Enable case-sensitive enum filtering
)]
pub struct Model {
    #[crudcrate(sortable, filterable)]  // Enable sorting and filtering
    pub title: String,

    #[crudcrate(filterable)]  // Enable filtering only
    pub status: Status,

    #[crudcrate(sortable)]  // Enable sorting only
    pub created_at: DateTime<Utc>,
}
```

## 🎯 Benefits

- **🚀 95% Less Boilerplate**: Single macro replaces significant amount of duplicated manual code
- **🔗 Full IDE Linking**: Navigate to functions, expressions, and types
- **🔧 Function Injection**: Bypass or override any operation with custom logic
- **📊 React-Admin Ready**: Built-in sorting, filtering, pagination
- **📖 Auto-Documentation**: OpenAPI docs generated automatically


## 📜 License and disclaimer

This project is licensed under the MIT License. See [LICENSE](./LICENSE) for
more details.

`Crudcrate` was developed to reduce the boilerplate in several projects and
offer an easy step up into Rust APIs, therefore, it gets constant testing.
However, it is very possible it contains bugs or edge cases that have not been
addressed yet. I am not responsible for any issues that may arise. Please do
your own testing, use at your own discretion (and report any issues you
encounter!).


## 🤖 AI Disclosure

Development of `crudcrate` and `crudcrate-derive` has occasionally been powered
by the questionable wisdom of large language models. They have been consulted
for prototyping, code suggestions, test generation, and the overuse of emojis
in documentation. This has resulted in perhaps more verbose and less optimal
implementations.

If you find this project useful and have a way to improve it, please help
defeat the bots by contributing! 🤓


## 🔗 Related Crates

- **[crudcrate-derive](https://crates.io/crates/crudcrate-derive)**: Procedural macros (implementation detail)
- **[Minimal Example](https://github.com/evanjt/crudcrate-example-minimal)**: Complete CRUD API in under 60 lines
- **[Full Example](https://github.com/evanjt/crudcrate-example)**: Production-ready API with migrations and advanced features
- **[SeaORM](https://crates.io/crates/sea-orm)**: Database ORM integration
- **[Axum](https://crates.io/crates/axum)**: Web framework integration
- **[Utoipa](https://crates.io/crates/utoipa)**: OpenAPI documentation
