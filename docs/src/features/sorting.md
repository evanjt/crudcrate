# Sorting

CRUDCrate supports flexible sorting with multiple format options.

## Enabling Sorting

Mark fields as sortable:

```rust
#[derive(EntityToModels)]
pub struct Model {
    #[crudcrate(sortable)]
    pub name: String,

    #[crudcrate(sortable)]
    pub created_at: DateTimeUtc,

    #[crudcrate(sortable)]
    pub priority: i32,

    // Not sortable
    pub description: String,
}
```

## Sort Syntax

### JSON Array Format (React Admin)

```bash
# Sort by single field with direction
GET /items?sort=["created_at","DESC"]

# Default order is ASC when direction omitted
GET /items?sort=["name"]
```

### REST Query Parameters

Two REST-style formats are supported:

```bash
# Using sort_by + order (preferred)
GET /items?sort_by=created_at&order=DESC

# Using sort + order (alternative)
GET /items?sort=created_at&order=DESC

# Default order is ASC
GET /items?sort_by=name
```

> **Note**: The `sort_by` parameter takes priority over `sort` if both are provided.

## Sort Directions

| Direction | Description |
|-----------|-------------|
| `ASC` | Ascending (A-Z, 0-9, oldest first) |
| `DESC` | Descending (Z-A, 9-0, newest first) |

Case-insensitive: `ASC`, `asc`, `Asc` all work.

## Examples by Type

### Strings

```bash
# A to Z
GET /items?sort=["name","ASC"]

# Z to A
GET /items?sort=["name","DESC"]
```

### Numbers

```bash
# Lowest to highest
GET /items?sort=["priority","ASC"]

# Highest to lowest
GET /items?sort=["priority","DESC"]
```

### Dates

```bash
# Oldest first
GET /items?sort=["created_at","ASC"]

# Newest first (common for feeds)
GET /items?sort=["created_at","DESC"]
```

### Booleans

```bash
# false first (0), then true (1)
GET /items?sort=["is_active","ASC"]

# true first (1), then false (0)
GET /items?sort=["is_active","DESC"]
```

## Default Sort Order

When no sort is specified, CRUDCrate uses the primary key:

```rust
// Default: ORDER BY id ASC
GET /items
```

Configure a different default in your handler or operations.

## Multiple Column Sorting

For multi-column sorting, implement a custom handler:

```rust
use sea_orm::{Order, QueryOrder};

async fn list_items(
    Query(params): Query<FilterOptions>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Vec<ItemList>>, ApiError> {
    let query = Entity::find()
        .order_by(Column::Priority, Order::Desc)  // Primary sort
        .order_by(Column::CreatedAt, Order::Desc); // Secondary sort

    let items = query.all(&db).await?;
    Ok(Json(items.into_iter().map(Into::into).collect()))
}
```

## Null Handling

Null values sort based on database:

| Database | NULL Position |
|----------|---------------|
| PostgreSQL | NULLS LAST (default) |
| MySQL | NULLS FIRST (in ASC) |
| SQLite | NULLS FIRST |

For consistent behavior, consider using `COALESCE` in custom queries.

## Programmatic Sorting

Use sorting in code:

```rust
use crudcrate::filtering::{parse_sorting, FilterOptions};
use sea_orm::{Order, QueryOrder};

async fn custom_list(
    Query(params): Query<FilterOptions>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Vec<Item>>, ApiError> {
    let (column, order) = parse_sorting::<Entity>(&params);

    let items = Entity::find()
        .order_by(column, order)
        .all(&db)
        .await?;

    Ok(Json(items.into_iter().map(Into::into).collect()))
}
```

### Manual Sorting

```rust
use sea_orm::{Order, QueryOrder};

let items = Entity::find()
    .order_by(Column::Priority, Order::Desc)
    .order_by(Column::Name, Order::Asc)
    .all(&db)
    .await?;
```

## Security

### Field Validation

Only `sortable` fields can be used:

```rust
#[crudcrate(sortable)]
pub name: String,  // Allowed

pub secret: String,  // Not sortable - ignored
```

Invalid sort fields are silently ignored (falls back to default).

### No SQL Injection

Sort fields are validated against entity columns:

```bash
# Attempted injection
GET /items?sort=["name; DROP TABLE items;--","ASC"]

# Result: field not found, uses default sort
# No SQL executed with injected content
```

## Performance Tips

### Index Sorted Fields

```sql
-- Index for sorted field
CREATE INDEX idx_items_created_at ON items(created_at);

-- Composite index for filter + sort
CREATE INDEX idx_items_status_created
    ON items(status, created_at DESC);
```

### Consider Sort + Filter Combinations

The most efficient queries have indexes covering both filter and sort:

```bash
# Common query pattern
GET /items?filter={"status":"active"}&sort=["created_at","DESC"]
```

```sql
-- Optimal index
CREATE INDEX idx_items_status_created
    ON items(status, created_at DESC);
```

## Common Patterns

### Newest First (Default for Lists)

```rust
#[crudcrate(sortable, exclude(create, update))]
pub created_at: DateTimeUtc,
```

```bash
GET /items?sort=["created_at","DESC"]
```

### Priority Queue

```rust
#[crudcrate(sortable, filterable)]
pub priority: i32,
```

```bash
# High priority first, then by date
GET /items?sort=["priority","DESC"]
```

### Alphabetical Lists

```rust
#[crudcrate(sortable, fulltext)]
pub name: String,
```

```bash
GET /items?sort=["name","ASC"]
```

## Error Handling

Invalid sort parameters don't cause errors - they fall back to defaults:

```bash
# Invalid field - falls back to default
GET /items?sort=["nonexistent","ASC"]

# Invalid format - falls back to default
GET /items?sort=not-an-array
```

This design prevents client errors from breaking functionality.

## Next Steps

- Configure [Pagination](./pagination.md)
- Enable [Fulltext Search](./fulltext-search.md)
- Learn about [Filtering](./filtering.md)
