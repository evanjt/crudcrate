# Filtering

CRUDCrate filters list endpoints via JSON in the `?filter=` query parameter.

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

## Filtering on Related Entities (Join Filtering)

CRUDCrate supports filtering parents by columns on their related children
using dot-notation syntax. The built-in handler resolves each joined filter
into a sub-query on the child table and intersects matching parent IDs with
the main query; no custom hook required.

### Enabling Join Filtering

Declare the whitelisted child columns with `filterable(...)` inside `join(...)`
on the parent's relationship field:

```rust
#[derive(EntityToModels)]
pub struct Model {
    #[sea_orm(primary_key)]
    #[crudcrate(primary_key)]
    pub id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub name: String,

    #[sea_orm(ignore)]
    #[crudcrate(
        non_db_attr,
        join(one, all, depth = 1, filterable("make", "year"))
    )]
    pub vehicles: Vec<Vehicle>,
}
```

### Dot-Notation Syntax

Filter with `relation.column` and the standard operator suffixes:

```bash
# Customers whose vehicles include at least one BMW
GET /customers?filter={"vehicles.make":"BMW"}

# Customers with at least one vehicle built in 2020 or later
GET /customers?filter={"vehicles.year_gte":2020}

# Intersection of two joined filters on the same relation
GET /customers?filter={"vehicles.make":"Toyota","vehicles.year_gte":2018}

# Combine with main-entity filters
GET /customers?filter={"name":"Alice","vehicles.make":"BMW"}
```

### Supported Operators

All standard operator suffixes work on joined columns:

| Operator | Example |
|----------|---------|
| (none) | `{"vehicles.make":"BMW"}` |
| `_neq` | `{"vehicles.make_neq":"BMW"}` |
| `_gt` | `{"vehicles.year_gt":2019}` |
| `_gte` | `{"vehicles.year_gte":2020}` |
| `_lt` | `{"vehicles.year_lt":2020}` |
| `_lte` | `{"vehicles.year_lte":2020}` |

### How It's Resolved

For every joined filter in a request the handler runs one sub-query on the
child entity and adds its result to the main condition:

```sql
SELECT * FROM customers
 WHERE id IN (
     SELECT customer_id
       FROM vehicles
      WHERE make = 'BMW'
        AND <child scope_condition()>  -- e.g. is_private = false
   )
   AND <main-entity filters>
   AND <parent scope_condition if middleware scope is active>
```

Two to three queries per request: one per joined-filter field plus the main
list query plus the count query, both of which reuse the augmented
condition. No row multiplication, no `DISTINCT`, no JOIN in the main query.

### Scope Safety

When the child entity declares `exclude(scoped)` privacy flags (booleans),
the generated code applies the child's
`ScopeFilterable::scope_condition()` to the sub-query. A parent cannot be
surfaced through a private child.

When you use **middleware-injected scope** (dynamic `ScopeCondition` from
request extensions, e.g. tenant_id from a verified JWT claim), the
injected condition is applied to the **parent** query. Transitive safety
relies on the standard schema invariant that a child belongs to the
parent referenced by its FK. If your schema allows children to reference
parents from a different tenant, filter those rows out at the middleware
level or add an explicit privacy flag on the child.

### Security (Whitelist Validation)

Only columns explicitly listed in `filterable(...)` can be filtered:

```rust
// Only make and year can be filtered
#[crudcrate(join(one, all, filterable("make", "year")))]
pub vehicles: Vec<Vehicle>,
```

```bash
# Allowed: year is in filterable
GET /customers?filter={"vehicles.year":2020}

# Ignored: model is NOT in filterable
GET /customers?filter={"vehicles.model":"Civic"}

# Ignored: unknown join field
GET /customers?filter={"fake.column":"value"}
```

Silent drop (rather than 400) is deliberate. It prevents:

- SQL injection via crafted dot-notation
- Schema discovery through filter probing
- Accidental exposure of internal columns

### Runnable Example

The `joined_filter` example seeds three customers with distinct vehicle
makes and demonstrates each variation end-to-end:

```bash
cargo run --example joined_filter

# in another shell:
curl 'http://localhost:3000/customers'
curl 'http://localhost:3000/customers?filter=%7B%22vehicles.make%22%3A%22BMW%22%7D'
curl 'http://localhost:3000/customers?filter=%7B%22vehicles.year_gte%22%3A2020%7D'
```

The end-to-end behaviour described on this page is backed by
`test_suite/tests/joined_filter_http_test.rs` (run with
`cargo test --manifest-path test_suite/Cargo.toml --test joined_filter_http_test`).

### Limitations

- Joined-column string comparisons are sent to the database as written, without the case folding and enum casting applied to main-entity filters. Whether `{"vehicles.make":"bmw"}` matches `BMW` therefore depends on the column collation.

**Single-level joins**: nested paths (`vehicles.parts.name`) are rejected
by the parser.

**`Vec<Child>` only**: joined filtering currently applies to `has_many`
relationships (`Vec<Child>` fields). `Option<Child>` (`belongs_to`) fields
that declare joined filterable columns are silently ignored, because the
FK is on the parent and the sub-query direction is reversed.

**FK column naming convention**: the child's FK column must follow the
convention `{parent_struct_name}_id` (snake_case) unless overridden via
`#[crudcrate(join(..., fk_column = "..."))]`.

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

How CRUDCrate handles bad filter input depends on the active
[`SecurityProfile`](../advanced/security.md#securityprofile):

- **Invalid JSON**: under `secure()` (the 0.9.0 default), returns
  `400 Bad Request`. Under `legacy()` / `react_admin()`, the filter is
  silently dropped and the unfiltered list is returned.
- **Unknown fields**: silently ignored under every profile, so callers
  can't probe the schema by trial.
- **Invalid values**: the offending field's filter is skipped; other
  fields still apply.
- **Malformed operator suffix**: falls back to equality on the base
  column.

## Next Steps

- Learn about [Sorting](./sorting.md)
- Configure [Pagination](./pagination.md)
- Enable [Fulltext Search](./fulltext-search.md)
