# How It Works

Understanding CRUDCrate's architecture helps you use it effectively and extend it when needed.

## The Big Picture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Your Sea-ORM Entity                          │
│   #[derive(DeriveEntityModel, EntityToModels)]                  │
│   pub struct Model { ... }                                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                 CRUDCrate Proc Macros                           │
│  (Compile-time code generation)                                 │
│                                                                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐        │
│  │ Create   │  │ Update   │  │ List     │  │ Response │        │
│  │ Model    │  │ Model    │  │ Model    │  │ Model    │        │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘        │
│                                                                 │
│  ┌──────────────────────────────────────────────────────┐      │
│  │              CRUDResource Implementation              │      │
│  │  (get_one, get_all, create, update, delete)          │      │
│  └──────────────────────────────────────────────────────┘      │
│                                                                 │
│  ┌──────────────────────────────────────────────────────┐      │
│  │              Axum Router (optional)                   │      │
│  │  GET /items, POST /items, PUT /items/:id, etc.       │      │
│  └──────────────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                 CRUDCrate Runtime                               │
│  (Query parsing, filtering, pagination, error handling)         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Sea-ORM                                     │
│  (Database abstraction, queries, migrations)                    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              PostgreSQL / MySQL / SQLite                        │
└─────────────────────────────────────────────────────────────────┘
```

## Two Crates, One System

CRUDCrate consists of two crates:

### 1. `crudcrate-derive` (Procedural Macros)

This crate runs at **compile time**. It:

- Parses your entity struct and `#[crudcrate(...)]` attributes
- Generates model structs (Create, Update, List, Response)
- Implements the `CRUDResource` trait
- Optionally generates an Axum router

The generated code is type-safe and verified by the Rust compiler.

### 2. `crudcrate` (Runtime Library)

This crate runs at **runtime**. It provides:

- `CRUDResource` trait definition
- Query parameter parsing (`FilterOptions`)
- SQL condition building (filtering, sorting)
- Pagination utilities
- Error handling (`ApiError`)

## Code Generation Flow

When you write:

```rust
#[derive(EntityToModels)]
#[crudcrate(generate_router)]
pub struct Model {
    #[crudcrate(primary_key, exclude(create))]
    pub id: i32,

    #[crudcrate(filterable, sortable)]
    pub name: String,
}
```

CRUDCrate generates (conceptually):

```rust
// 1. Response model (your struct name without "Model")
pub struct Item {
    pub id: i32,
    pub name: String,
}

// 2. Create model (excludes `id`)
pub struct ItemCreate {
    pub name: String,
}

// 3. Update model (all optional)
pub struct ItemUpdate {
    pub name: Option<String>,
}

// 4. List model
pub struct ItemList {
    pub id: i32,
    pub name: String,
}

// 5. CRUDResource implementation
impl CRUDResource for Item {
    type EntityType = Entity;
    type CreateModel = ItemCreate;
    type UpdateModel = ItemUpdate;
    type ListModel = ItemList;

    async fn get_one(db: &DatabaseConnection, id: i32) -> Result<Self, ApiError> {
        // Generated query logic
    }

    async fn get_all(
        db: &DatabaseConnection,
        condition: Condition,
        order: (Column, Order),
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Self::ListModel>, ApiError> {
        // Generated query logic with filtering
    }

    async fn create(db: &DatabaseConnection, data: ItemCreate) -> Result<Self, ApiError> {
        // Generated insert logic
    }

    async fn update(db: &DatabaseConnection, id: i32, data: ItemUpdate) -> Result<Self, ApiError> {
        // Generated update logic
    }

    async fn delete(db: &DatabaseConnection, id: i32) -> Result<(), ApiError> {
        // Generated delete logic
    }
}

// 6. Router (if generate_router enabled)
pub fn item_router() -> Router {
    Router::new()
        .route("/items", get(list_handler).post(create_handler))
        .route("/items/:id", get(get_handler).put(update_handler).delete(delete_handler))
}
```

## Request Flow

Here's what happens when a request hits your API:

```
HTTP Request: GET /items?filter={"name":"test"}&sort=["id","DESC"]&range=[0,9]
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  1. Axum Router                                                 │
│     - Matches route /items                                      │
│     - Extracts query parameters                                 │
│     - Calls list_handler                                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  2. Query Parsing (CRUDCrate Runtime)                           │
│     - FilterOptions::from_query_params()                        │
│     - Parses filter JSON: {"name": "test"}                      │
│     - Parses sort: ["id", "DESC"]                               │
│     - Parses range: [0, 9] → offset=0, limit=10                │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  3. Condition Building                                          │
│     - apply_filters() converts JSON to Condition                │
│     - Validates "name" is marked filterable                     │
│     - Builds: Column::Name.eq("test")                          │
│     - SQL injection prevention applied                          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  4. CRUDResource::get_all()                                     │
│     - Entity::find()                                            │
│     - .filter(condition)                                        │
│     - .order_by(Column::Id, Order::Desc)                       │
│     - .offset(0).limit(10)                                      │
│     - .all(db).await                                            │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  5. Sea-ORM                                                     │
│     - Builds SQL: SELECT * FROM items                           │
│                   WHERE name = $1                               │
│                   ORDER BY id DESC                              │
│                   LIMIT 10 OFFSET 0                             │
│     - Executes against database                                 │
│     - Returns Vec<Model>                                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  6. Response Building                                           │
│     - Convert Vec<Model> to Vec<ItemList>                       │
│     - Add Content-Range header                                  │
│     - Serialize to JSON                                         │
│     - Return HTTP 200 with body                                 │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
HTTP Response: 200 OK
Content-Range: items 0-9/42
[{"id": 1, "name": "test"}, ...]
```

## Compile-Time vs Runtime

| Aspect | Compile-Time (Macros) | Runtime (Library) |
|--------|----------------------|-------------------|
| **When** | `cargo build` | Request handling |
| **What** | Code generation | Query execution |
| **Errors** | Compilation errors | HTTP error responses |
| **Cost** | Build time | Request latency |
| **Examples** | Missing attributes, type mismatches | Invalid filters, DB errors |

## Extension Points

CRUDCrate provides hooks at multiple levels:

### 1. Attribute Configuration
Configure behavior at compile time:
```rust
#[crudcrate(filterable, exclude(list))]
pub field: String,
```

### 2. CRUDOperations Trait
Add business logic:
```rust
impl CRUDOperations for MyOps {
    async fn before_create(&self, data: &mut CreateModel) -> Result<(), ApiError> {
        // Validation, transformation
    }
}
```

### 3. Lifecycle Hooks
Per-operation customization:
```rust
#[crudcrate(
    create::one::pre = validate_fn,
    create::one::post = notify_fn,
)]
```

### 4. Full Handler Override
Complete control when needed:
```rust
async fn custom_create(
    Extension(db): Extension<DatabaseConnection>,
    Json(data): Json<ItemCreate>,
) -> Result<Json<Item>, ApiError> {
    // Your custom logic
}
```

## Performance Characteristics

- **Zero runtime overhead** for generated code (no reflection)
- **Compile-time type checking** catches errors early
- **Database-native features** used when available (indexes, fulltext)
- **Parameterized queries** prevent SQL injection without string escaping
- **Pagination limits** prevent DoS attacks (max 1000 items)

## Next Steps

- Understand [The Entity Model](./entity-model.md)
- Learn about [Generated Models](./generated-models.md)
- Explore the [CRUDResource Trait](./crudresource-trait.md)
