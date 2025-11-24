# Fulltext Search

CRUDCrate provides fulltext search across multiple fields with database-specific optimizations.

## Enabling Fulltext Search

Mark fields to include in search:

```rust
#[derive(EntityToModels)]
pub struct Model {
    #[crudcrate(fulltext)]
    pub title: String,

    #[crudcrate(fulltext)]
    pub description: String,

    #[crudcrate(fulltext)]
    pub tags: String,

    // Not searchable
    pub internal_code: String,
}
```

## Search Syntax

Use the `q` parameter:

```bash
# Simple search
GET /items?q=rust programming

# Search with other parameters
GET /items?q=async&filter={"status":"published"}&sort=["created_at","DESC"]
```

## How It Works

### PostgreSQL (Trigram Similarity)

Uses `ILIKE` combined with `pg_trgm` similarity for fuzzy matching:

```sql
-- Generated query (simplified)
SELECT * FROM items
WHERE (
    UPPER(COALESCE(title::text, '') || ' ' || COALESCE(description::text, ''))
    LIKE UPPER('%rust programming%') ESCAPE '\'
    OR SIMILARITY(COALESCE(title::text, '') || ' ' || COALESCE(description::text, ''), 'rust programming') > 0.1
)
```

This approach provides:
- **Substring matching**: Finds "rust" inside "rusty" or "trusty"
- **Fuzzy matching**: Handles typos via trigram similarity
- **Case insensitivity**: Automatically case-insensitive

**Setup for best performance:**

```sql
-- Enable pg_trgm extension (required)
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Create trigram index for faster similarity searches
CREATE INDEX idx_items_title_trgm ON items USING gin (title gin_trgm_ops);
CREATE INDEX idx_items_description_trgm ON items USING gin (description gin_trgm_ops);
```

### MySQL & SQLite (LIKE Fallback)

Uses case-insensitive LIKE queries:

```sql
-- Generated query
SELECT * FROM items
WHERE UPPER(CAST(title AS TEXT) || ' ' || CAST(description AS TEXT))
    LIKE UPPER('%rust programming%') ESCAPE '\'
```

The query is treated as a single phrase, matching records where the concatenated fields contain the search string.

## Search Behavior

### Single Phrase Search

The entire query is treated as a single search term:

```bash
GET /items?q=rust programming

# Matches items containing the phrase "rust programming"
# Does NOT split into separate "rust" AND "programming" terms
```

### Case Insensitivity

All searches are case-insensitive:

```bash
GET /items?q=RUST
GET /items?q=rust
GET /items?q=Rust

# All return the same results
```

### Partial Matching (Substring)

All databases support substring matching via LIKE:

```bash
GET /items?q=rust

# Matches: "rust", "rusty", "trustworthy", "Rust Programming"
```

PostgreSQL additionally uses trigram similarity for fuzzy matching, which helps with:
- Typos (e.g., "progamming" may still find "programming")
- Similar words

## Combining with Filters

Search works with other query parameters:

```bash
# Search within active items
GET /items?q=tutorial&filter={"status":"active"}

# Search + sort + paginate
GET /items?q=rust&sort=["created_at","DESC"]&range=[0,9]
```

## Performance Tips

### Index Strategy

| Database | Recommended Index |
|----------|-------------------|
| PostgreSQL | GIN with pg_trgm (see setup above) |
| MySQL | Standard B-tree on searched columns |
| SQLite | Standard indexes on searched columns |

### Query Optimization

1. **Limit results**: Always paginate search results
2. **Use filters**: Narrow results before fulltext search
3. **Cache common searches**: For popular queries

### Example: Optimized Search

```bash
# ❌ Slow: fulltext search on all items
GET /items?q=rust

# ✅ Fast: filter first, then search
GET /items?q=rust&filter={"category":"programming"}&range=[0,19]
```

## Highlighting Results

For search result highlighting, implement post-processing:

```rust
fn highlight_matches(text: &str, query: &str) -> String {
    let terms: Vec<&str> = query.split_whitespace().collect();
    let mut result = text.to_string();

    for term in terms {
        let pattern = regex::Regex::new(&format!("(?i)({})", regex::escape(term))).unwrap();
        result = pattern.replace_all(&result, "<mark>$1</mark>").to_string();
    }

    result
}
```

## Empty Search

Empty or whitespace-only queries return all items:

```bash
GET /items?q=
GET /items?q=

# Both return unfiltered results (with pagination)
```

## Special Characters

Search queries are sanitized:

```bash
# Special characters are escaped
GET /items?q=c++
GET /items?q=node.js
GET /items?q=user@email

# All work safely
```

## Next Steps

- Configure [Relationships](./relationships.md)
- Learn about [Filtering](./filtering.md)
- Set up [Error Handling](./error-handling.md)
