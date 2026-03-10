# Struct Attributes Reference

Complete reference for `#[crudcrate(...)]` attributes on structs.

## Syntax

```rust
#[derive(EntityToModels)]
#[crudcrate(attribute1, attribute2 = value, ...)]
pub struct Model { }
```

## Attributes

### `generate_router`

Generates an Axum router function for all CRUD operations.

```rust
#[crudcrate(generate_router)]
pub struct Model { }

// Generates:
pub fn model_router() -> Router { }
```

**Type:** Flag (no value)

---

### `api_struct`

Override the name of generated API structs.

```rust
#[crudcrate(api_struct = "Product")]
pub struct Model { }

// Generates: Product, ProductCreate, ProductUpdate, ProductList
```

**Type:** String literal
**Default:** Derives from table name (e.g., "products" → "Product")

---

### `name_singular`

Override the singular resource name for routing and headers.

```rust
#[crudcrate(name_singular = "person")]
pub struct Model { }

// Used in: Content-Range header
// Content-Range: person 0-9/100
```

**Type:** String literal
**Default:** Lowercase struct name

---

### `name_plural`

Override the plural resource name for routing.

```rust
#[crudcrate(name_plural = "people")]
pub struct Model { }

// Routes: GET /people, POST /people, etc.
```

**Type:** String literal
**Default:** `{name_singular}s`

---

### `operations`

Specify a custom `CRUDOperations` implementation.

```rust
#[crudcrate(operations = MyOperations)]
pub struct Model { }

// MyOperations must implement CRUDOperations trait
pub struct MyOperations;

impl CRUDOperations for MyOperations {
    type Resource = Model;
    // ...
}
```

**Type:** Type path
**Default:** `DefaultCRUDOperations<Self>`

---

### `description`

Add description for OpenAPI documentation.

```rust
#[crudcrate(description = "Blog articles with comments")]
pub struct Model { }
```

**Type:** String literal
**Default:** None

---

### `fulltext_language`

Set language for PostgreSQL fulltext search.

```rust
#[crudcrate(fulltext_language = "spanish")]
pub struct Model { }
```

**Type:** String literal
**Default:** `"english"`
**Options:** `"english"`, `"spanish"`, `"german"`, `"french"`, `"simple"`, etc.

---

### `batch_limit`

Set the maximum number of items for batch create/update/delete operations.

```rust
#[crudcrate(batch_limit = 500)]
pub struct Model { }
```

**Type:** Integer
**Default:** `100`
**Runtime override:** Implement `fn batch_limit() -> usize` on your `CRUDResource` impl for dynamic values (env vars, config).

---

### `max_page_size`

Set the maximum items per page for pagination.

```rust
#[crudcrate(max_page_size = 500)]
pub struct Model { }
```

**Type:** Integer
**Default:** `1000`
**Runtime override:** Implement `fn max_page_size() -> u64` on your `CRUDResource` impl for dynamic values.

---

---

### `aggregate(...)`

Configure time-series aggregation endpoints (requires `aggregation` feature + TimescaleDB).

```rust
#[crudcrate(aggregate(
    time_column = "recorded_at",
    intervals("1h", "1d", "1w"),
    metrics("value", "temperature"),
    group_by("site_id"),
    aggregates(avg, min, max, first, last),
    continuous_aggregates(
        view("1h", "readings_hourly"),
        view("1d", "readings_daily"),
    ),
))]
```

| Sub-attribute | Required | Default | Description |
|---|---|---|---|
| `time_column = "col"` | Yes | — | The `TIMESTAMPTZ` column for time bucketing |
| `intervals("1h", "1d")` | Yes | — | Allowed interval values (short form) |
| `metrics("value")` | Yes | — | Numeric columns to aggregate |
| `group_by("site_id")` | No | `[]` | Additional grouping columns |
| `aggregates(avg, min)` | No | `avg, min, max` | Aggregate functions per metric |
| `continuous_aggregates(...)` | No | `[]` | Pre-computed view mappings |

#### `continuous_aggregates(...)` sub-attributes

| Sub-attribute | Description |
|---|---|
| `view("interval", "view_name")` | Map an interval to a TimescaleDB continuous aggregate view |

**Compile-time checks:**
- `time_column` must reference an existing `DateTime` field
- `metrics` must reference existing numeric fields
- `group_by` must reference existing fields
- CA intervals must be in the `intervals` list
- CA view names must be valid SQL identifiers

See [Time-Series Aggregation](../advanced/aggregation.md) for the full guide.

---

## Lifecycle Hook Attributes

### `create::one::pre`

Function called before create operation.

```rust
#[crudcrate(create::one::pre = validate_create)]

async fn validate_create(
    db: &DatabaseConnection,
    data: &mut ModelCreate,
) -> Result<(), ApiError> { }
```

---

### `create::one::post`

Function called after successful create.

```rust
#[crudcrate(create::one::post = notify_created)]

async fn notify_created(
    db: &DatabaseConnection,
    created: &Model,
) -> Result<(), ApiError> { }
```

---

### `create::one::body`

Replace entire create logic.

```rust
#[crudcrate(create::one::body = custom_create)]

async fn custom_create(
    db: &DatabaseConnection,
    data: ModelCreate,
) -> Result<Model, ApiError> { }
```

---

### `update::one::pre`

Function called before update operation.

```rust
#[crudcrate(update::one::pre = check_update_permission)]

async fn check_update_permission(
    db: &DatabaseConnection,
    id: PrimaryKeyType,
    data: &mut ModelUpdate,
) -> Result<(), ApiError> { }
```

---

### `update::one::post`

Function called after successful update.

```rust
#[crudcrate(update::one::post = invalidate_cache)]

async fn invalidate_cache(
    db: &DatabaseConnection,
    updated: &Model,
) -> Result<(), ApiError> { }
```

---

### `update::one::body`

Replace entire update logic.

```rust
#[crudcrate(update::one::body = custom_update)]

async fn custom_update(
    db: &DatabaseConnection,
    id: PrimaryKeyType,
    data: ModelUpdate,
) -> Result<Model, ApiError> { }
```

---

### `delete::one::pre`

Function called before delete operation.

```rust
#[crudcrate(delete::one::pre = check_delete_permission)]

async fn check_delete_permission(
    db: &DatabaseConnection,
    id: PrimaryKeyType,
) -> Result<(), ApiError> { }
```

---

### `delete::one::post`

Function called after successful delete.

```rust
#[crudcrate(delete::one::post = cleanup_related)]

async fn cleanup_related(
    db: &DatabaseConnection,
    id: PrimaryKeyType,
) -> Result<(), ApiError> { }
```

---

### `delete::one::body`

Replace entire delete logic (e.g., for soft delete).

```rust
#[crudcrate(delete::one::body = soft_delete)]

async fn soft_delete(
    db: &DatabaseConnection,
    id: PrimaryKeyType,
) -> Result<(), ApiError> { }
```

---

### `get::one::pre`

Function called before get_one operation.

```rust
#[crudcrate(get::one::pre = check_view_permission)]

async fn check_view_permission(
    db: &DatabaseConnection,
    id: PrimaryKeyType,
) -> Result<(), ApiError> { }
```

---

### `get::all::pre`

Function called before get_all operation, can modify condition.

```rust
#[crudcrate(get::all::pre = filter_by_tenant)]

async fn filter_by_tenant(
    db: &DatabaseConnection,
    condition: &mut Condition,
) -> Result<(), ApiError> { }
```

---

## Complete Example

```rust
#[derive(EntityToModels)]
#[crudcrate(
    generate_router,
    api_struct = "Article",
    name_singular = "article",
    name_plural = "articles",
    operations = ArticleOperations,
    description = "Blog articles with comments",
    fulltext_language = "english",
    create::one::pre = validate_article,
    create::one::post = index_for_search,
    update::one::pre = check_edit_permission,
    delete::one::body = soft_delete_article,
    get::all::pre = filter_published_only,
)]
#[sea_orm(table_name = "articles")]
pub struct Model {
    // ...
}
```

## Foreign Key Naming Convention

When using `join()` for batch loading in `get_all()`, CRUDCrate derives the foreign key column name from the parent struct name using PascalCase convention:

- **Parent struct** `Customer` → **FK column** `CustomerId` (SeaORM Column enum) / `customer_id` (field name)
- **Parent struct** `VehiclePart` → **FK column** `VehiclePartId` / `vehicle_part_id`

Your related entity's SeaORM model must have a matching foreign key field. For example, if `Customer` has `vehicles: Vec<Vehicle>`, the `Vehicle` model must have a `customer_id: Uuid` field and a `Column::CustomerId` variant.

> **Note**: Custom FK names (e.g., `owner_id` instead of `customer_id`) are not yet supported via attributes. If your FK name doesn't follow the convention, use a custom `read::many::body` hook to implement the loading logic.

### Batch Loading Query Behavior

For `get_all()` with `join(all)` or `join(one, all)`, CRUDCrate uses batch loading:

- **Depth=1 joins**: 2 queries total (1 for parents + 1 per join field using `WHERE fk IN (...)`)
- **Depth > 1 joins**: Additional per-item queries for nested children (falls back to `get_one()` calls)
- **`get_one()` with `join(one)`**: Per-item queries (single entity, no batching needed)

> **Note**: Batch loading currently requires UUID primary keys, consistent with the `CRUDResource` trait contract.

See [Security](../advanced/security.md) for partial success and batch limit configuration.

## See Also

- [Field Attributes Reference](./field-attributes.md)
- [Custom Logic with Hooks](../tutorial/hooks.md)
- [Custom Operations](../advanced/custom-operations.md)
