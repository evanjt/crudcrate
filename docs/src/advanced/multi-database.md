# Multi-Database Support

CRUDCrate works with PostgreSQL, MySQL, and SQLite, with database-specific optimizations.

## Supported Databases

| Database | Support Level | Fulltext Search | Best For |
|----------|--------------|-----------------|----------|
| **PostgreSQL** | ✅ Full | GIN + tsvector | Complex queries, high concurrency |
| **MySQL** | ✅ Full | FULLTEXT index | Traditional deployments |
| **SQLite** | ✅ Full | LIKE fallback | Development, embedded apps |

## Configuration

### PostgreSQL

```toml
[dependencies]
sea-orm = { version = "1.0", features = ["runtime-tokio-rustls", "sqlx-postgres"] }
```

```rust
let db = Database::connect("postgres://user:pass@localhost/mydb").await?;
```

**Optimizations:**
- GIN indexes for fulltext
- `tsvector` query optimization
- Array operations for `IN` clauses

### MySQL

```toml
[dependencies]
sea-orm = { version = "1.0", features = ["runtime-tokio-rustls", "sqlx-mysql"] }
```

```rust
let db = Database::connect("mysql://user:pass@localhost/mydb").await?;
```

**Optimizations:**
- FULLTEXT indexes
- `MATCH AGAINST` queries
- Optimized `LIKE` patterns

### SQLite

```toml
[dependencies]
sea-orm = { version = "1.0", features = ["runtime-tokio-rustls", "sqlx-sqlite"] }
```

```rust
// File-based
let db = Database::connect("sqlite:./data.db").await?;

// In-memory (for testing)
let db = Database::connect("sqlite::memory:").await?;
```

**Limitations:**
- No native fulltext (uses LIKE)
- Single-writer limitation
- Limited concurrent access

## Database-Specific Features

### Fulltext Search

PostgreSQL (recommended):

```sql
-- Create tsvector column and index
ALTER TABLE articles ADD COLUMN search_vector tsvector;

CREATE INDEX idx_articles_search ON articles
    USING GIN(search_vector);

-- Update trigger
CREATE FUNCTION articles_search_trigger() RETURNS trigger AS $$
BEGIN
    NEW.search_vector :=
        setweight(to_tsvector('english', coalesce(NEW.title, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(NEW.content, '')), 'B');
    RETURN NEW;
END $$ LANGUAGE plpgsql;

CREATE TRIGGER articles_search_update
    BEFORE INSERT OR UPDATE ON articles
    FOR EACH ROW
    EXECUTE FUNCTION articles_search_trigger();
```

MySQL:

```sql
ALTER TABLE articles
    ADD FULLTEXT INDEX idx_articles_fulltext (title, content);
```

SQLite:

```sql
-- For better fulltext, use FTS5
CREATE VIRTUAL TABLE articles_fts USING fts5(
    title, content, content=articles, content_rowid=id
);
```

### JSON Types

PostgreSQL has native JSONB:

```rust
use sea_orm::prelude::Json;

pub metadata: Json,  // Uses JSONB in PostgreSQL
```

MySQL uses JSON type:

```rust
pub metadata: Json,  // Uses JSON in MySQL
```

SQLite stores as TEXT:

```rust
pub metadata: String,  // Store JSON as string, parse in application
```

### UUID Types

PostgreSQL has native UUID:

```rust
use uuid::Uuid;

#[sea_orm(primary_key, column_type = "Uuid")]
pub id: Uuid,
```

MySQL/SQLite store as CHAR(36) or BINARY(16):

```rust
#[sea_orm(primary_key, column_type = "String(StringLen::N(36))")]
pub id: Uuid,
```

## Migration Strategy

### Development → Production

1. Develop with SQLite
2. Test with PostgreSQL locally
3. Deploy to PostgreSQL in production

```rust
fn get_database_url() -> String {
    match std::env::var("RUST_ENV").as_deref() {
        Ok("production") => std::env::var("DATABASE_URL").unwrap(),
        _ => "sqlite::memory:".to_string(),
    }
}
```

### Database Detection

```rust
use sea_orm::DatabaseBackend;

fn get_backend(db: &DatabaseConnection) -> DatabaseBackend {
    db.get_database_backend()
}

// Use in queries
match db.get_database_backend() {
    DatabaseBackend::Postgres => {
        // PostgreSQL-specific query
    },
    DatabaseBackend::MySql => {
        // MySQL-specific query
    },
    DatabaseBackend::Sqlite => {
        // SQLite-specific query
    },
}
```

## Connection Strings

### PostgreSQL

```
# Basic
postgres://user:password@host:5432/database

# With SSL
postgres://user:password@host:5432/database?sslmode=require

# With connection pool settings
postgres://user:password@host:5432/database?pool_max=10
```

### MySQL

```
# Basic
mysql://user:password@host:3306/database

# With charset
mysql://user:password@host:3306/database?charset=utf8mb4
```

### SQLite

```
# File
sqlite:./path/to/database.db

# In-memory
sqlite::memory:

# In-memory with shared cache
sqlite:file::memory:?cache=shared
```

## Testing Across Databases

```rust
#[cfg(test)]
mod tests {
    use sea_orm::Database;

    async fn setup_test_db() -> DatabaseConnection {
        // Use SQLite for fast tests
        let db = Database::connect("sqlite::memory:").await.unwrap();

        // Run migrations
        Migrator::up(&db, None).await.unwrap();

        db
    }

    #[tokio::test]
    async fn test_create_article() {
        let db = setup_test_db().await;
        // Test code using SQLite
    }
}

// Integration tests with real database
#[cfg(test)]
mod integration_tests {
    #[tokio::test]
    #[ignore] // Run with: cargo test -- --ignored
    async fn test_postgres_specific() {
        let db = Database::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        // PostgreSQL-specific tests
    }
}
```

## Performance by Database

### PostgreSQL Strengths

- Excellent for complex queries
- Best fulltext search
- JSONB operations
- Concurrent access
- Advanced indexing (GIN, BRIN, partial)

### MySQL Strengths

- Widely deployed
- Good FULLTEXT support
- Simpler replication
- Broad hosting support

### SQLite Strengths

- Zero configuration
- Embedded deployment
- Fast for small datasets
- Perfect for testing

## Recommendations

### For Production

**PostgreSQL** is recommended for:
- Complex filtering
- Fulltext search
- JSON data
- High concurrency

### For Development

**SQLite** is recommended for:
- Quick setup
- Unit tests
- Local development
- Prototyping

### Migration Path

```
Development: SQLite (fast, easy setup)
    ↓
Staging: PostgreSQL (production-like)
    ↓
Production: PostgreSQL (optimized)
```

## Next Steps

- Set up [Security](./security.md)
- Configure [Performance Optimization](./performance.md)
- Learn about [Custom Operations](./custom-operations.md)
