# Filtering

CRUDCrate provides powerful, type-safe filtering through JSON query parameters.

## Enabling Filtering

Mark fields as filterable:

```rust
#[derive(EntityToModels)]
pub struct Model {
    #[crudcrate(filterable)]
    pub status: String,

    #[crudcrate(filterable)]
    pub priority: i32,

    #[crudcrate(filterable)]
    pub created_at: DateTimeUtc,

    // Not filterable
    pub description: String,
}
```

## Filter Syntax

### JSON Filter Format (React Admin Compatible)

All filtering uses the JSON `filter` query parameter:

```bash
# Exact match
GET /items?filter={"status":"active"}

# Multiple conditions (AND)
GET /items?filter={"status":"active","priority":5}

# Null check
GET /items?filter={"deleted_at":null}

# Array for IN queries
GET /items?filter={"status":["active","pending","review"]}
```

### Comparison Operators

Use field name suffixes within the JSON filter for comparisons:

```bash
# Not equals
GET /items?filter={"status_neq":"inactive"}

# Greater than
GET /items?filter={"priority_gt":3}

# Greater than or equal
GET /items?filter={"priority_gte":3}

# Less than
GET /items?filter={"priority_lt":10}

# Less than or equal
GET /items?filter={"priority_lte":10}
```

## Supported Operators

| Operator | SQL | Example |
|----------|-----|---------|
| (none) | `=` | `{"status":"active"}` |
| `_neq` | `!=` | `{"status_neq":"deleted"}` |
| `_gt` | `>` | `{"priority_gt":5}` |
| `_gte` | `>=` | `{"priority_gte":5}` |
| `_lt` | `<` | `{"priority_lt":10}` |
| `_lte` | `<=` | `{"priority_lte":10}` |
| (array) | `IN` | `{"status":["a","b","c"]}` |

## Type-Specific Filtering

### Strings

```bash
# Exact match (case-insensitive)
GET /items?filter={"name":"John"}

# Multiple values (IN)
GET /items?filter={"status":["active","pending"]}
```

### Numbers

```bash
# Exact
GET /items?filter={"quantity":10}

# Range (combine multiple operators)
GET /items?filter={"quantity_gte":5,"quantity_lte":20}

# Comparison
GET /items?filter={"price_gt":100}
```

### Booleans

```bash
# Exact match
GET /items?filter={"active":true}
GET /items?filter={"active":false}
```

### Dates

```bash
# Exact date
GET /items?filter={"created_at":"2024-01-15"}

# Date range
GET /items?filter={"created_at_gte":"2024-01-01","created_at_lte":"2024-12-31"}

# ISO 8601 format
GET /items?filter={"created_at_gte":"2024-01-15T10:30:00Z"}
```

### Enums

```rust
#[derive(EnumIter, DeriveActiveEnum)]
pub enum Status {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "active")]
    Active,
}

// In entity
#[crudcrate(filterable)]
pub status: Status,
```

```bash
# Filter by enum value (use the string_value)
GET /items?filter={"status":"active"}
GET /items?filter={"status":["pending","active"]}
```

### UUIDs

```bash
# Exact match
GET /items?filter={"user_id":"550e8400-e29b-41d4-a716-446655440000"}

# Multiple UUIDs
GET /items?filter={"user_id":["uuid1","uuid2","uuid3"]}
```

### Optional Fields (Null Checks)

```bash
# Field is null
GET /items?filter={"deleted_at":null}
```

> **Note**: Checking for "not null" requires custom filtering logic via lifecycle hooks.

## Complex Filters

### Combining Conditions

All conditions in the JSON filter are combined with AND:

```bash
# status = "active" AND priority >= 5
GET /items?filter={"status":"active","priority_gte":5}
```

## Security

### SQL Injection Prevention

All filters are parameterized. User input is never interpolated into SQL:

```rust
// User provides: {"name": "'; DROP TABLE users; --"}

// CRUDCrate generates parameterized query:
// SELECT * FROM items WHERE name = $1
// With parameter: "'; DROP TABLE users; --"

// Safe! The value is treated as data, not SQL
```

### Field Validation

Only fields marked `filterable` can be filtered:

```rust
#[crudcrate(filterable)]
pub status: String,  // Allowed

pub secret: String,  // Not filterable - filter will be ignored
```

For security, unknown or non-filterable fields are silently ignored rather than causing errors. This prevents information disclosure about your schema.

## Programmatic Filtering

Use filters directly in code:

```rust
use crudcrate::filtering::{apply_filters, FilterOptions};
use sea_orm::Condition;

async fn custom_handler(
    Query(params): Query<FilterOptions>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Vec<Item>>, ApiError> {
    // Build condition from query params
    let condition = apply_filters::<Entity>(&params)?;

    // Add additional conditions
    let condition = condition.add(Column::Deleted.eq(false));

    // Use with Sea-ORM
    let items = Entity::find()
        .filter(condition)
        .all(&db)
        .await?;

    Ok(Json(items.into_iter().map(Into::into).collect()))
}
```

### Building Conditions Manually

```rust
use sea_orm::Condition;

let condition = Condition::all()
    .add(Column::Status.eq("active"))
    .add(Column::Priority.gte(5))
    .add(Column::DeletedAt.is_null());

let items = Entity::find()
    .filter(condition)
    .all(&db)
    .await?;
```

## Performance Tips

### Index Your Filtered Fields

```sql
-- PostgreSQL
CREATE INDEX idx_items_status ON items(status);
CREATE INDEX idx_items_created_at ON items(created_at);

-- Composite index for common filter combinations
CREATE INDEX idx_items_status_priority ON items(status, priority);
```

### Limit Filter Complexity

Complex filters can impact performance. Consider:

1. **Pagination**: Always paginate filtered results
2. **Indexes**: Index frequently filtered columns
3. **Caching**: Cache common filter results
4. **Limits**: Set maximum result limits

## LIKE-Filterable Fields (Partial Matching)

For fields that need partial/substring matching instead of exact equality, implement `like_filterable_columns()` in your `CRUDResource` trait:

```rust
impl CRUDResource for YourEntity {
    // ... other methods ...

    fn like_filterable_columns() -> Vec<&'static str> {
        vec!["title", "description", "name"]
    }
}
```

When a field is in this list, filters will use case-insensitive `LIKE '%value%'` matching:

```bash
# With title in like_filterable_columns():
GET /items?filter={"title":"urgent"}
# Matches: "This is urgent", "URGENT: Please review", "Not so urgent task"
```

This is useful for fields where users expect partial matching behavior.

## Error Handling

CRUDCrate handles invalid filters gracefully:

- **Invalid JSON**: Returns all results (filter is ignored)
- **Unknown fields**: Silently ignored for security
- **Invalid values**: Field filter is skipped
- **Malformed operators**: Falls back to equality check

This defensive approach prevents information disclosure about your schema while maintaining API stability.

## Next Steps

- Learn about [Sorting](./sorting.md)
- Configure [Pagination](./pagination.md)
- Enable [Fulltext Search](./fulltext-search.md)
