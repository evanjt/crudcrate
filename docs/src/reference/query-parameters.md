# Query Parameters Reference

Complete reference for all supported query parameters.

## FilterOptions Struct

```rust
pub struct FilterOptions {
    pub filter: Option<String>,      // JSON filter object
    pub sort: Option<String>,        // Sort specification
    pub order: Option<String>,       // Sort order (ASC/DESC)
    pub range: Option<String>,       // Pagination range [start, end]
    pub page: Option<u64>,           // Page number
    pub per_page: Option<u64>,       // Items per page
    pub q: Option<String>,           // Fulltext search query
    // Plus dynamic field-specific filters
}
```

## Filtering Parameters

### `filter` (JSON Object)

Filter by exact field values.

```bash
# Single field
?filter={"status":"active"}

# Multiple fields (AND)
?filter={"status":"active","priority":5}

# Null check
?filter={"deleted_at":null}
```

### Field-Specific Operators

```bash
# Exact match (default)
?status=active
?status_eq=active

# Not equal
?status_ne=deleted

# Greater than
?priority_gt=5

# Greater than or equal
?priority_gte=5

# Less than
?priority_lt=10

# Less than or equal
?priority_lte=10

# Contains (LIKE)
?title_like=urgent

# In list
?status_in=active,pending,review
```

**Operator Reference:**

| Suffix | SQL | Example |
|--------|-----|---------|
| `_eq` | `=` | `?status_eq=active` |
| `_ne` | `!=` | `?status_ne=deleted` |
| `_gt` | `>` | `?age_gt=18` |
| `_gte` | `>=` | `?age_gte=18` |
| `_lt` | `<` | `?price_lt=100` |
| `_lte` | `<=` | `?price_lte=100` |
| `_like` | `LIKE` | `?name_like=john` |
| `_in` | `IN` | `?status_in=a,b,c` |

## Sorting Parameters

### `sort` (JSON Array - React Admin)

```bash
# Field and direction
?sort=["created_at","DESC"]

# Field only (defaults to ASC)
?sort=["name"]
```

### `sort` + `order` (Standard)

```bash
?sort=created_at&order=DESC
?sort=name&order=ASC
```

### Combined Format

```bash
?sort=created_at_desc
?sort=name_asc
```

**Order Values:**
- `ASC` / `asc` - Ascending (A-Z, 0-9, oldest)
- `DESC` / `desc` - Descending (Z-A, 9-0, newest)

## Pagination Parameters

### `range` (React Admin)

```bash
# Items 0-9 (first 10)
?range=[0,9]

# Items 20-29
?range=[20,29]

# Items 0-99 (first 100)
?range=[0,99]
```

### `page` + `per_page` (Standard)

```bash
# Page 1, 20 per page
?page=1&per_page=20

# Page 5, 50 per page
?page=5&per_page=50
```

**Limits:**
- Maximum `per_page`: 1,000
- Maximum offset: 1,000,000

## Search Parameters

### `q` (Fulltext Search)

```bash
# Search all fulltext fields
?q=meeting notes

# Combined with filters
?q=urgent&filter={"status":"open"}
```

## Response Headers

### Content-Range

```http
Content-Range: items 0-19/150
```

Format: `{resource} {start}-{end}/{total}`

## Complete Examples

### Basic List

```bash
GET /articles
```

### Filtered List

```bash
GET /articles?filter={"status":"published","author_id":5}
```

### Sorted List

```bash
GET /articles?sort=["created_at","DESC"]
```

### Paginated List

```bash
GET /articles?range=[0,19]
```

### Search

```bash
GET /articles?q=rust programming
```

### Combined Query

```bash
GET /articles?filter={"status":"published"}&sort=["views","DESC"]&range=[0,9]&q=tutorial
```

### Complex Filter

```bash
GET /articles?status=published&views_gte=1000&created_at_gte=2024-01-01
```

## Parsing Functions

### `apply_filters`

Build Sea-ORM condition from query parameters.

```rust
use crudcrate::filtering::apply_filters;

let condition = apply_filters::<Entity>(&params)?;
```

### `parse_pagination`

Extract offset and limit from parameters.

```rust
use crudcrate::filtering::parse_pagination;

let (offset, limit) = parse_pagination(&params);
// Default: (0, 20)
```

### `parse_sorting`

Extract column and order from parameters.

```rust
use crudcrate::filtering::parse_sorting;

let (column, order) = parse_sorting::<Entity>(&params);
// Default: (primary_key_column, Order::Asc)
```

### `build_fulltext_condition`

Build fulltext search condition.

```rust
use crudcrate::filtering::build_fulltext_condition;

let condition = build_fulltext_condition(
    query,
    &["title", "content"],
    db.get_database_backend()
);
```

## URL Encoding

Special characters in values must be URL-encoded:

```bash
# Space → %20 or +
?q=hello+world
?q=hello%20world

# Brackets → %5B %5D
?range=%5B0,9%5D

# Curly braces → %7B %7D
?filter=%7B"status":"active"%7D

# Comma in value → %2C
?tags_in=one%2Ctwo%2Cthree
```

## Error Responses

### Invalid Filter Field

```bash
GET /articles?unknown_field=value
```

```json
{"error": "Invalid filter field: unknown_field"}
```

### Invalid Filter Value

```bash
GET /articles?priority_gte=not-a-number
```

```json
{"error": "Invalid filter value for field 'priority': expected number"}
```

### Invalid Sort Field

Invalid sort fields are silently ignored (falls back to default).

### Invalid Range Format

```bash
GET /articles?range=invalid
```

Falls back to default pagination.

## See Also

- [Filtering](../features/filtering.md)
- [Sorting](../features/sorting.md)
- [Pagination](../features/pagination.md)
- [Fulltext Search](../features/fulltext-search.md)
