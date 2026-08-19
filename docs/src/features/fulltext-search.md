# Fulltext Search

CRUDCrate provides case-insensitive substring search across multiple fields.

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

The search term is matched as a substring of the concatenated searchable fields.
There is no fuzzy or similarity matching: a typo ("progamming") does not match
"programming". The term is bound as a query parameter, and LIKE wildcards in it
(`%`, `_`) are escaped with `!` so they match literally.

### PostgreSQL

Uses `ILIKE` for case-insensitive matching:

```sql
-- Generated query (simplified)
SELECT * FROM items
WHERE (COALESCE(title::text, '') || ' ' || COALESCE(description::text, ''))
    ILIKE '%rust programming%' ESCAPE '!'
```

### MySQL & SQLite

Uses `UPPER(...) LIKE` with the pattern uppercased on the Rust side:

```sql
-- Generated query (simplified)
SELECT * FROM items
WHERE UPPER(CAST(title AS TEXT) || ' ' || CAST(description AS TEXT))
    LIKE '%RUST PROGRAMMING%' ESCAPE '!'
```

The uppercasing of the search pattern is ASCII-reliable; non-ASCII case folding
can differ between Rust and the database collation on these backends.

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
| PostgreSQL | Optional: a `pg_trgm` GIN index (`gin_trgm_ops`) accelerates `ILIKE '%term%'` scans |
| MySQL | Standard B-tree on searched columns |
| SQLite | Standard indexes on searched columns |

### Query Optimization

1. **Limit results**: Always paginate search results
2. **Use filters**: Narrow results before fulltext search
3. **Cache common searches**: For popular queries

### Example: Optimized Search

```bash
# Slow: fulltext search on all items
GET /items?q=rust

# Fast: filter first, then search
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
