# Pagination

CRUDCrate provides secure pagination with multiple format support.

## Pagination Formats

### React Admin Format (Range)

```bash
# First 10 items (0-9)
GET /items?range=[0,9]

# Items 10-19
GET /items?range=[10,19]

# Items 50-74 (25 items)
GET /items?range=[50,74]
```

### Standard Page Format

```bash
# Page 1 with 20 items per page
GET /items?page=1&per_page=20

# Page 3 with 10 items per page
GET /items?page=3&per_page=10
```

## Response Headers

CRUDCrate adds RFC 7233 compliant headers:

```http
HTTP/1.1 200 OK
Content-Range: items 0-9/42
```

| Header | Format | Description |
|--------|--------|-------------|
| `Content-Range` | `{resource} {start}-{end}/{total}` | Current range and total count |

### Parsing Content-Range

```javascript
// JavaScript example
const contentRange = response.headers.get('Content-Range');
const match = contentRange.match(/(\w+) (\d+)-(\d+)\/(\d+)/);
const [_, resource, start, end, total] = match;

console.log(`Showing ${start}-${end} of ${total} ${resource}`);
// "Showing 0-9 of 42 items"
```

## Security Limits

CRUDCrate enforces limits to prevent abuse:

| Limit | Value | Purpose |
|-------|-------|---------|
| Max page size | 1,000 | Prevent memory exhaustion |
| Max offset | 1,000,000 | Prevent excessive DB load |

```bash
# Requesting too many items
GET /items?per_page=10000

# Response: limited to 1000 items
Content-Range: items 0-999/5000
```

## Examples

### Basic Pagination

```bash
# First page
GET /items?range=[0,19]
# Content-Range: items 0-19/150

# Second page
GET /items?range=[20,39]
# Content-Range: items 20-39/150

# Last page (partial)
GET /items?range=[140,159]
# Content-Range: items 140-149/150
```

### With Filtering

```bash
# Filter + paginate
GET /items?filter={"status":"active"}&range=[0,9]
# Content-Range: items 0-9/42  (42 active items total)
```

### With Sorting

```bash
# Sort + paginate
GET /items?sort=["created_at","DESC"]&range=[0,9]
# Returns newest 10 items
```

### Combined

```bash
# Filter + sort + paginate
GET /items?filter={"status":"active"}&sort=["priority","DESC"]&range=[0,9]
# Returns top 10 highest priority active items
```

## Programmatic Pagination

### Using FilterOptions

```rust
use crudcrate::filtering::{parse_pagination, FilterOptions};

async fn list_handler(
    Query(params): Query<FilterOptions>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<(HeaderMap, Json<Vec<ItemList>>), ApiError> {
    // Parse pagination from query params
    let (offset, limit) = parse_pagination(&params);

    let items = Entity::find()
        .offset(offset)
        .limit(limit)
        .all(&db)
        .await?;

    // Calculate Content-Range
    let total = Entity::find().count(&db).await?;
    let end = (offset + items.len() as u64).saturating_sub(1);

    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Range",
        format!("items {}-{}/{}", offset, end, total).parse().unwrap()
    );

    Ok((headers, Json(items.into_iter().map(Into::into).collect())))
}
```

### Manual Pagination

```rust
use sea_orm::QuerySelect;

// Direct offset/limit
let items = Entity::find()
    .offset(20)
    .limit(10)
    .all(&db)
    .await?;
```

### Using Paginator

```rust
use sea_orm::PaginatorTrait;

// Sea-ORM's built-in paginator
let paginator = Entity::find()
    .filter(condition)
    .paginate(&db, 20);  // 20 items per page

// Get specific page
let items = paginator.fetch_page(2).await?;  // Page 2 (0-indexed)

// Get total count
let total = paginator.num_items().await?;
let pages = paginator.num_pages().await?;
```

## Offset vs Cursor Pagination

CRUDCrate uses offset pagination by default. For large datasets, consider cursor pagination:

### Offset Pagination (Default)
- ✅ Simple to implement
- ✅ Random page access
- ❌ Inconsistent with concurrent writes
- ❌ Slow for large offsets

### Cursor Pagination (Custom Implementation)
- ✅ Consistent with concurrent writes
- ✅ Fast for any position
- ❌ No random page access
- ❌ Requires ordered, unique field

Example cursor pagination:

```rust
async fn list_with_cursor(
    Query(params): Query<CursorParams>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<CursorResponse<Item>>, ApiError> {
    let limit = params.limit.unwrap_or(20).min(100);

    let mut query = Entity::find()
        .order_by(Column::CreatedAt, Order::Desc);

    // If cursor provided, filter to items after cursor
    if let Some(cursor) = &params.cursor {
        let cursor_time = parse_cursor(cursor)?;
        query = query.filter(Column::CreatedAt.lt(cursor_time));
    }

    let items: Vec<Model> = query
        .limit(limit + 1)  // Fetch one extra to check for more
        .all(&db)
        .await?;

    let has_more = items.len() > limit as usize;
    let items: Vec<Item> = items.into_iter()
        .take(limit as usize)
        .map(Into::into)
        .collect();

    let next_cursor = if has_more {
        items.last().map(|i| encode_cursor(&i.created_at))
    } else {
        None
    };

    Ok(Json(CursorResponse {
        items,
        next_cursor,
    }))
}
```

## Performance Tips

### For Large Tables

```sql
-- Ensure indexes on sorted columns
CREATE INDEX idx_items_created_at ON items(created_at DESC);

-- For filtered + sorted pagination
CREATE INDEX idx_items_status_created
    ON items(status, created_at DESC);
```

### Total Count Optimization

Counting can be slow on large tables:

```rust
// Option 1: Cached counts
let total = get_cached_count_or_query(&db).await?;

// Option 2: Approximate counts (PostgreSQL)
// SELECT reltuples FROM pg_class WHERE relname = 'items';

// Option 3: Skip count for infinite scroll
let items = Entity::find()
    .limit(limit + 1)  // Fetch extra to detect more
    .all(&db)
    .await?;
let has_more = items.len() > limit;
```

## Empty Results

When no items match:

```bash
GET /items?filter={"status":"nonexistent"}&range=[0,9]

# Response
HTTP/1.1 200 OK
Content-Range: items 0-0/0
[]
```

## Next Steps

- Learn about [Fulltext Search](./fulltext-search.md)
- Configure [Filtering](./filtering.md)
- Set up [Sorting](./sorting.md)
