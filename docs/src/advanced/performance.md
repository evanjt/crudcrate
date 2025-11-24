# Performance Optimization

Optimize your CRUDCrate API for production workloads.

## Database Indexing

### Index Filtered Fields

```sql
-- Single column indexes for filterable fields
CREATE INDEX idx_articles_status ON articles(status);
CREATE INDEX idx_articles_author_id ON articles(author_id);
CREATE INDEX idx_articles_created_at ON articles(created_at);

-- Composite index for common filter combinations
CREATE INDEX idx_articles_status_created ON articles(status, created_at DESC);
```

### Index Sorted Fields

```sql
-- DESC index for newest-first queries
CREATE INDEX idx_articles_created_at_desc ON articles(created_at DESC);

-- Composite for filter + sort
CREATE INDEX idx_articles_author_created ON articles(author_id, created_at DESC);
```

### Fulltext Indexes

```sql
-- PostgreSQL GIN index
CREATE INDEX idx_articles_search ON articles USING GIN(
    to_tsvector('english', coalesce(title, '') || ' ' || coalesce(content, ''))
);

-- MySQL FULLTEXT index
ALTER TABLE articles ADD FULLTEXT INDEX idx_articles_fulltext (title, content);
```

## Query Optimization

### Use Selective Filters

```bash
# ❌ Full table scan
GET /articles

# ✅ Filtered query uses index
GET /articles?filter={"status":"published"}
```

### Limit Result Size

```bash
# ✅ Always paginate
GET /articles?range=[0,19]

# Built-in limit: max 1000 items per request
```

### Avoid Deep Joins

```rust
// ❌ Deep recursion
#[crudcrate(non_db_attr, join(one, all, depth = 10))]
pub comments: Vec<Comment>,

// ✅ Limited depth
#[crudcrate(non_db_attr, join(one, depth = 2))]
pub comments: Vec<Comment>,
```

## Connection Pooling

Configure Sea-ORM connection pool:

```rust
use sea_orm::{Database, ConnectOptions};

let mut opt = ConnectOptions::new(database_url);
opt.max_connections(100)
   .min_connections(5)
   .connect_timeout(Duration::from_secs(8))
   .acquire_timeout(Duration::from_secs(8))
   .idle_timeout(Duration::from_secs(8))
   .max_lifetime(Duration::from_secs(8))
   .sqlx_logging(false);  // Disable query logging in production

let db = Database::connect(opt).await?;
```

## Caching Strategies

### Response Caching

```rust
use axum::http::header;
use tower_http::set_header::SetResponseHeaderLayer;

// Cache static-ish data
let app = Router::new()
    .route("/categories", get(list_categories))
    .layer(SetResponseHeaderLayer::if_not_present(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=300")  // 5 minutes
    ));
```

### Query Caching with Redis

```rust
use redis::AsyncCommands;

async fn get_articles_cached(
    db: &DatabaseConnection,
    redis: &redis::Client,
    filter: &FilterOptions,
) -> Result<Vec<Article>, ApiError> {
    let cache_key = format!("articles:{}", hash_filter(filter));

    // Try cache first
    if let Ok(mut conn) = redis.get_async_connection().await {
        if let Ok(cached) = conn.get::<_, String>(&cache_key).await {
            if let Ok(articles) = serde_json::from_str(&cached) {
                return Ok(articles);
            }
        }
    }

    // Cache miss - query database
    let articles = Article::get_all(db, /* ... */).await?;

    // Store in cache
    if let Ok(mut conn) = redis.get_async_connection().await {
        let _ = conn.set_ex::<_, _, ()>(
            &cache_key,
            serde_json::to_string(&articles).unwrap(),
            300  // 5 minute TTL
        ).await;
    }

    Ok(articles)
}
```

### Count Caching

Counting large tables is expensive:

```rust
// Cache total counts
async fn get_total_count_cached(
    db: &DatabaseConnection,
    redis: &redis::Client,
    entity: &str,
) -> u64 {
    let cache_key = format!("count:{}", entity);

    if let Ok(mut conn) = redis.get_async_connection().await {
        if let Ok(count) = conn.get::<_, u64>(&cache_key).await {
            return count;
        }
    }

    // Cache miss
    let count = Entity::find().count(db).await.unwrap_or(0);

    // Cache for 60 seconds
    if let Ok(mut conn) = redis.get_async_connection().await {
        let _ = conn.set_ex::<_, _, ()>(&cache_key, count, 60).await;
    }

    count
}
```

## Pagination Optimization

### Keyset Pagination

For large datasets, use cursor-based pagination:

```rust
// Instead of OFFSET (slow for large values)
// Use WHERE id > last_id (fast with index)

async fn list_articles_keyset(
    db: &DatabaseConnection,
    after_id: Option<Uuid>,
    limit: u64,
) -> Result<Vec<Article>, ApiError> {
    let mut query = Entity::find()
        .order_by(Column::Id, Order::Asc);

    if let Some(id) = after_id {
        query = query.filter(Column::Id.gt(id));
    }

    let articles = query
        .limit(limit)
        .all(db)
        .await?;

    Ok(articles.into_iter().map(Into::into).collect())
}
```

### Skip Count for Infinite Scroll

```rust
async fn list_articles_no_count(
    db: &DatabaseConnection,
    offset: u64,
    limit: u64,
) -> Result<(Vec<Article>, bool), ApiError> {
    // Fetch one extra to check for more
    let articles = Entity::find()
        .offset(offset)
        .limit(limit + 1)
        .all(db)
        .await?;

    let has_more = articles.len() > limit as usize;
    let articles: Vec<Article> = articles
        .into_iter()
        .take(limit as usize)
        .map(Into::into)
        .collect();

    Ok((articles, has_more))
}
```

## List Optimization

### Exclude Heavy Fields from Lists

```rust
// Full content not needed in lists
#[crudcrate(exclude(list))]
pub content: String,

// Relationships only in detail view
#[crudcrate(non_db_attr, join(one))]  // NOT join(all)
pub comments: Vec<Comment>,
```

### Select Only Needed Columns

```rust
// Custom list handler with column selection
async fn list_articles_optimized(
    Query(params): Query<FilterOptions>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Vec<ArticleListItem>>, ApiError> {
    let articles = Entity::find()
        .select_only()
        .column(Column::Id)
        .column(Column::Title)
        .column(Column::Excerpt)
        .column(Column::CreatedAt)
        // Omit content, relationships
        .into_model::<ArticleListItem>()
        .all(&db)
        .await?;

    Ok(Json(articles))
}
```

## Async Best Practices

### Batch Database Operations

```rust
// ❌ Sequential queries
for id in ids {
    let item = Entity::find_by_id(id).one(db).await?;
    results.push(item);
}

// ✅ Batch query
let items = Entity::find()
    .filter(Column::Id.is_in(ids))
    .all(db)
    .await?;
```

### Parallel Independent Queries

```rust
use tokio::join;

async fn get_article_with_stats(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<ArticleWithStats, ApiError> {
    // Run queries in parallel
    let (article, comment_count, view_count) = join!(
        Entity::find_by_id(id).one(db),
        comment::Entity::find().filter(comment::Column::ArticleId.eq(id)).count(db),
        get_view_count(id),
    );

    let article = article?.ok_or(ApiError::NotFound)?;

    Ok(ArticleWithStats {
        article: article.into(),
        comment_count: comment_count?,
        view_count: view_count?,
    })
}
```

## Monitoring

### Query Logging

```rust
// Enable in development
let mut opt = ConnectOptions::new(database_url);
opt.sqlx_logging(true)
   .sqlx_logging_level(tracing::log::LevelFilter::Debug);
```

### Slow Query Detection

```rust
use tracing::{info, warn};
use std::time::Instant;

async fn timed_query<T, F, Fut>(name: &str, f: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let start = Instant::now();
    let result = f().await;
    let elapsed = start.elapsed();

    if elapsed > Duration::from_millis(100) {
        warn!(query = name, elapsed_ms = elapsed.as_millis(), "Slow query");
    } else {
        info!(query = name, elapsed_ms = elapsed.as_millis(), "Query completed");
    }

    result
}
```

## Performance Checklist

- [ ] Indexes on all filtered columns
- [ ] Indexes on all sorted columns
- [ ] Composite indexes for common query patterns
- [ ] Fulltext indexes for search fields
- [ ] Connection pool properly sized
- [ ] Pagination enforced
- [ ] Heavy fields excluded from lists
- [ ] Join depth limited
- [ ] Query caching for hot paths
- [ ] Count caching for large tables
- [ ] Query logging enabled (dev) / disabled (prod)
- [ ] Slow query monitoring

## Next Steps

- Configure [Multi-Database Support](./multi-database.md)
- Set up [Security](./security.md)
- Learn about [Custom Operations](./custom-operations.md)
