/*!
# CRUD Benchmarks

Comprehensive benchmarks for crudcrate CRUD operations.

## Usage

```bash
# Run all benchmarks (SQLite by default)
cargo bench --bench crud_benchmarks

# Run PostgreSQL benchmarks (requires Docker)
docker run --name benchmark-postgres -e POSTGRES_PASSWORD=pass -e POSTGRES_DB=benchmark -p 5432:5432 -d postgres:16
BENCHMARK_DATABASE_URL=postgres://postgres:pass@localhost/benchmark cargo bench --bench crud_benchmarks
docker stop benchmark-postgres && docker rm benchmark-postgres

# Run specific benchmark group
cargo bench --bench crud_benchmarks -- "CRUD Operations"

# Quick benchmark with fewer samples
cargo bench --bench crud_benchmarks -- --quick

# Verbose output with statistics
cargo bench --bench crud_benchmarks -- --verbose
```

HTML reports are generated in `target/criterion/report/index.html`.
*/

use axum::{
    Router,
    body::Body,
    http::{Method, Request},
};
use chrono::{DateTime, Utc};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use crudcrate::{EntityToModels, crud_handlers, traits::CRUDResource};
use sea_orm::{Database, DatabaseConnection, entity::prelude::*};
use sea_orm_migration::{prelude::*, sea_query::ColumnDef};
use std::time::Duration;
use tokio::runtime::Runtime;
use tower::ServiceExt;
use uuid::Uuid;

/// Benchmark entity with comprehensive field types
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "benchmark_posts")]
#[crudcrate(api_struct = "BenchmarkPost", active_model = "ActiveModel")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, create_model = false, update_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(fulltext, filterable, sortable)]
    pub title: String,

    #[sea_orm(column_type = "Text")]
    #[crudcrate(fulltext, filterable)]
    pub content: String,

    #[crudcrate(fulltext, filterable, sortable)]
    pub author: String,

    #[sea_orm(column_type = "Text", nullable)]
    #[crudcrate(fulltext)]
    pub tags: Option<String>,

    #[crudcrate(filterable)]
    pub published: bool,

    #[crudcrate(filterable, sortable)]
    pub category: String,

    #[crudcrate(filterable, sortable)]
    pub view_count: i32,

    #[crudcrate(filterable, sortable)]
    pub priority: i32,

    #[crudcrate(sortable, create_model = false, update_model = false, on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Generate CRUD handlers
crud_handlers!(BenchmarkPost, BenchmarkPostUpdate, BenchmarkPostCreate);

// Migration for the benchmark database
pub struct BenchmarkMigrator;

#[async_trait::async_trait]
impl MigratorTrait for BenchmarkMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(CreateBenchmarkTable),
            Box::new(CreateBenchmarkIndexes),
        ]
    }
}

pub struct CreateBenchmarkTable;

#[async_trait::async_trait]
impl MigrationName for CreateBenchmarkTable {
    fn name(&self) -> &'static str {
        "m20240101_000001_create_benchmark_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for CreateBenchmarkTable {
    async fn up(&self, manager: &SchemaManager) -> Result<(), sea_orm::DbErr> {
        let table = Table::create()
            .table(BenchmarkEntity)
            .if_not_exists()
            .col(
                ColumnDef::new(BenchmarkColumn::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            )
            .col(ColumnDef::new(BenchmarkColumn::Title).string().not_null())
            .col(ColumnDef::new(BenchmarkColumn::Content).text().not_null())
            .col(ColumnDef::new(BenchmarkColumn::Author).string().not_null())
            .col(ColumnDef::new(BenchmarkColumn::Tags).text().null())
            .col(
                ColumnDef::new(BenchmarkColumn::Published)
                    .boolean()
                    .not_null()
                    .default(false),
            )
            .col(
                ColumnDef::new(BenchmarkColumn::Category)
                    .string()
                    .not_null(),
            )
            .col(
                ColumnDef::new(BenchmarkColumn::ViewCount)
                    .integer()
                    .not_null()
                    .default(0),
            )
            .col(
                ColumnDef::new(BenchmarkColumn::Priority)
                    .integer()
                    .not_null()
                    .default(0),
            )
            .col(
                ColumnDef::new(BenchmarkColumn::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .to_owned();

        manager.create_table(table).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), sea_orm::DbErr> {
        manager
            .drop_table(Table::drop().table(BenchmarkEntity).to_owned())
            .await?;
        Ok(())
    }
}

pub struct CreateBenchmarkIndexes;

#[async_trait::async_trait]
impl MigrationName for CreateBenchmarkIndexes {
    fn name(&self) -> &'static str {
        "m20240101_000002_create_benchmark_indexes"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for CreateBenchmarkIndexes {
    async fn up(&self, manager: &SchemaManager) -> Result<(), sea_orm::DbErr> {
        let db = manager.get_connection();

        // Detect database backend to create appropriate indexes
        if db.get_database_backend() == sea_orm::DatabaseBackend::Postgres {
            // PostgreSQL: Create GIN indexes for fulltext search
            // This creates a combined fulltext search index
            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_fulltext ON benchmark_posts USING GIN (to_tsvector('english', title || ' ' || content || ' ' || author || ' ' || COALESCE(tags, '')))"
                )
                .await?;

            // Regular B-tree indexes for filtering and sorting
            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_published ON benchmark_posts (published)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_category ON benchmark_posts (category)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_view_count ON benchmark_posts (view_count)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_priority ON benchmark_posts (priority)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_created_at ON benchmark_posts (created_at)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_author ON benchmark_posts (author)",
                )
                .await?;
        } else if db.get_database_backend() == sea_orm::DatabaseBackend::MySql {
            // MySQL: Create FULLTEXT indexes for fulltext search
            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE FULLTEXT INDEX idx_benchmark_posts_fulltext ON benchmark_posts (title, content, author, tags)",
                )
                .await?;

            // Regular B-tree indexes for filtering and sorting
            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_published ON benchmark_posts (published)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_category ON benchmark_posts (category)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_view_count ON benchmark_posts (view_count)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_priority ON benchmark_posts (priority)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_created_at ON benchmark_posts (created_at)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_author ON benchmark_posts (author)",
                )
                .await?;
        } else {
            // SQLite: Create regular indexes (SQLite doesn't have native fulltext in our setup)
            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_published ON benchmark_posts (published)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_category ON benchmark_posts (category)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_view_count ON benchmark_posts (view_count)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_priority ON benchmark_posts (priority)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_created_at ON benchmark_posts (created_at)",
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_author ON benchmark_posts (author)",
                )
                .await?;

            // SQLite individual field indexes for text searches
            manager
                .get_connection()
                .execute_unprepared(
                    "CREATE INDEX idx_benchmark_posts_title ON benchmark_posts (title)",
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), sea_orm::DbErr> {
        // Drop indexes (works for both PostgreSQL and SQLite)
        let indexes = vec![
            "idx_benchmark_posts_fulltext",
            "idx_benchmark_posts_published",
            "idx_benchmark_posts_category",
            "idx_benchmark_posts_view_count",
            "idx_benchmark_posts_priority",
            "idx_benchmark_posts_created_at",
            "idx_benchmark_posts_author",
            "idx_benchmark_posts_title",
        ];

        for index in indexes {
            // Ignore errors when dropping indexes (some may not exist depending on backend)
            let _ = manager
                .get_connection()
                .execute_unprepared(&format!("DROP INDEX IF EXISTS {index}"))
                .await;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum BenchmarkColumn {
    Id,
    Title,
    Content,
    Author,
    Tags,
    Published,
    Category,
    ViewCount,
    Priority,
    CreatedAt,
}

impl Iden for BenchmarkColumn {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(
            s,
            "{}",
            match self {
                Self::Id => "id",
                Self::Title => "title",
                Self::Content => "content",
                Self::Author => "author",
                Self::Tags => "tags",
                Self::Published => "published",
                Self::Category => "category",
                Self::ViewCount => "view_count",
                Self::Priority => "priority",
                Self::CreatedAt => "created_at",
            }
        )
        .unwrap();
    }
}

#[derive(Debug)]
pub struct BenchmarkEntity;

impl Iden for BenchmarkEntity {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(s, "benchmark_posts").unwrap();
    }
}

// Helper function to get database URL from environment or default to SQLite
fn get_database_url() -> String {
    std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("BENCHMARK_DATABASE_URL"))
        .unwrap_or_else(|_| "sqlite::memory:".to_string())
}

// Helper function to set up benchmark database with various data sizes
async fn setup_benchmark_db(record_count: usize) -> Result<DatabaseConnection, sea_orm::DbErr> {
    let database_url = get_database_url();
    let db = Database::connect(&database_url).await?;

    // Run migrations
    BenchmarkMigrator::up(&db, None).await?;

    // Run index analysis on first setup (commented out to avoid spam during benchmarks)
    // Uncomment to see index recommendations:
    // BenchmarkPost::analyze_and_display_indexes(&db).await?;

    // Insert sample data for benchmarking
    for i in 0..record_count {
        let post = BenchmarkPostCreate {
            title: format!("Benchmark Post Title {i}"),
            content: format!(
                "This is benchmark content for post {i}. It contains various keywords like performance, testing, database, queries, and optimization to test fulltext search capabilities."
            ),
            author: format!("Author{}", i % 10), // 10 different authors
            tags: if i % 3 == 0 {
                Some(format!("tag{}, tag{}, benchmark", i % 5, (i + 1) % 5))
            } else {
                None
            },
            published: i % 2 == 0,
            category: format!("Category{}", i % 5), // 5 different categories
            view_count: <i32 as std::convert::TryFrom<_>>::try_from(i * 10).unwrap_or(i32::MAX),
            priority: <i32 as std::convert::TryFrom<_>>::try_from(i % 10).unwrap_or(0),
        };

        BenchmarkPost::create(&db, post).await?;
    }

    Ok(db)
}

// Set up the benchmark API router
fn setup_benchmark_app(db: DatabaseConnection) -> Router {
    let api = Router::new()
        .route(
            "/benchmark_posts",
            axum::routing::get(get_all_handler).post(create_one_handler),
        )
        .route(
            "/benchmark_posts/{id}",
            axum::routing::get(get_one_handler)
                .put(update_one_handler)
                .delete(delete_one_handler),
        )
        .with_state(db);

    Router::new().nest("/api/v1", api)
}

// Benchmark simple GET all request
async fn benchmark_get_all(app: Router) -> Result<Vec<BenchmarkPost>, Box<dyn std::error::Error>> {
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/benchmark_posts")
        .body(Body::empty())?;

    let response = app.oneshot(request).await?;
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let posts: Vec<BenchmarkPost> = serde_json::from_slice(&body)?;
    Ok(posts)
}

// Benchmark filtered request
async fn benchmark_filtered_query(
    app: Router,
    filter: &str,
) -> Result<Vec<BenchmarkPost>, Box<dyn std::error::Error>> {
    let encoded_filter = url_escape::encode_component(filter);
    let uri = format!("/api/v1/benchmark_posts?filter={encoded_filter}");

    let request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())?;

    let response = app.oneshot(request).await?;
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let posts: Vec<BenchmarkPost> = serde_json::from_slice(&body)?;
    Ok(posts)
}

// Benchmark fulltext search
async fn benchmark_fulltext_search(
    app: Router,
    query: &str,
) -> Result<Vec<BenchmarkPost>, Box<dyn std::error::Error>> {
    let filter = format!("{{\"q\":\"{query}\"}}");
    benchmark_filtered_query(app, &filter).await
}

// Benchmark sorted request
async fn benchmark_sorted_query(
    app: Router,
    sort_field: &str,
    order: &str,
) -> Result<Vec<BenchmarkPost>, Box<dyn std::error::Error>> {
    let uri = format!("/api/v1/benchmark_posts?sort={sort_field}&order={order}");

    let request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())?;

    let response = app.oneshot(request).await?;
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let posts: Vec<BenchmarkPost> = serde_json::from_slice(&body)?;
    Ok(posts)
}

// Benchmark paginated request
async fn benchmark_paginated_query(
    app: Router,
    page: usize,
    per_page: usize,
) -> Result<Vec<BenchmarkPost>, Box<dyn std::error::Error>> {
    let uri = format!("/api/v1/benchmark_posts?page={page}&per_page={per_page}");

    let request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())?;

    let response = app.oneshot(request).await?;
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let posts: Vec<BenchmarkPost> = serde_json::from_slice(&body)?;
    Ok(posts)
}

// Benchmark CREATE operations
async fn benchmark_create_post(
    app: Router,
    post_data: BenchmarkPostCreate,
) -> Result<BenchmarkPost, Box<dyn std::error::Error>> {
    let json_body = serde_json::to_string(&post_data)?;

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/benchmark_posts")
        .header("content-type", "application/json")
        .body(Body::from(json_body))?;

    let response = app.oneshot(request).await?;
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let post: BenchmarkPost = serde_json::from_slice(&body)?;
    Ok(post)
}

// Benchmark complex queries (combining filters, sorting, pagination)
async fn benchmark_complex_query(
    app: Router,
) -> Result<Vec<BenchmarkPost>, Box<dyn std::error::Error>> {
    let filter = r#"{"published":true,"priority_gte":5}"#;
    let encoded_filter = url_escape::encode_component(filter);
    let uri = format!(
        "/api/v1/benchmark_posts?filter={encoded_filter}&sort=view_count&order=DESC&page=0&per_page=20"
    );

    let request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())?;

    let response = app.oneshot(request).await?;
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let posts: Vec<BenchmarkPost> = serde_json::from_slice(&body)?;
    Ok(posts)
}

fn bench_crud_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    // Determine database backend for labeling
    let database_url = get_database_url();
    let backend_name = if database_url.starts_with("postgres") {
        "PostgreSQL"
    } else if database_url.starts_with("mysql") {
        "MySQL"
    } else {
        "SQLite"
    };

    // Test with smaller dataset sizes for quicker benchmarks
    let dataset_sizes = vec![100, 500];

    for size in dataset_sizes {
        let db = rt.block_on(setup_benchmark_db(size)).unwrap();
        let app = setup_benchmark_app(db);

        let mut group =
            c.benchmark_group(format!("CRUD Operations {backend_name} ({size}records)"));
        group.measurement_time(Duration::from_secs(10));

        // Simple GET all benchmark
        group.bench_with_input(BenchmarkId::new("get_all", size), &size, |b, _| {
            b.iter(|| rt.block_on(std::hint::black_box(benchmark_get_all(app.clone()))));
        });

        // Filtered queries benchmark
        let filters = vec![
            r#"{"published":true}"#,
            r#"{"author":"Author1"}"#,
            r#"{"category":"Category2"}"#,
            r#"{"view_count_gte":500}"#,
        ];

        for filter in filters {
            group.bench_with_input(
                BenchmarkId::new("filtered_query", filter),
                &filter,
                |b, filter| {
                    b.iter(|| {
                        rt.block_on(std::hint::black_box(benchmark_filtered_query(
                            app.clone(),
                            filter,
                        )))
                    });
                },
            );
        }

        // Fulltext search benchmark
        let search_queries = vec![
            "performance",
            "benchmark content",
            "database optimization",
            "testing queries",
        ];

        for query in search_queries {
            group.bench_with_input(
                BenchmarkId::new("fulltext_search", query),
                &query,
                |b, query| {
                    b.iter(|| {
                        rt.block_on(std::hint::black_box(benchmark_fulltext_search(
                            app.clone(),
                            query,
                        )))
                    });
                },
            );
        }

        // Sorting benchmark
        let sort_operations = vec![
            ("title", "ASC"),
            ("view_count", "DESC"),
            ("created_at", "DESC"),
            ("priority", "ASC"),
        ];

        for (field, order) in sort_operations {
            group.bench_with_input(
                BenchmarkId::new("sorted_query", format!("{field}_{order}")),
                &(field, order),
                |b, (field, order)| {
                    b.iter(|| {
                        rt.block_on(std::hint::black_box(benchmark_sorted_query(
                            app.clone(),
                            field,
                            order,
                        )))
                    });
                },
            );
        }

        // Pagination benchmark
        let pagination_sizes = vec![10, 50, 100];
        for page_size in pagination_sizes {
            group.bench_with_input(
                BenchmarkId::new("paginated_query", page_size),
                &page_size,
                |b, page_size| {
                    b.iter(|| {
                        rt.block_on(std::hint::black_box(benchmark_paginated_query(
                            app.clone(),
                            0,
                            *page_size,
                        )))
                    });
                },
            );
        }

        // Complex query benchmark (combines multiple operations)
        group.bench_with_input(BenchmarkId::new("complex_query", size), &size, |b, _| {
            b.iter(|| rt.block_on(std::hint::black_box(benchmark_complex_query(app.clone()))));
        });

        group.finish();
    }
}

fn bench_create_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    // Determine database backend for labeling
    let database_url = get_database_url();
    let backend_name = if database_url.starts_with("postgres") {
        "PostgreSQL"
    } else if database_url.starts_with("mysql") {
        "MySQL"
    } else {
        "SQLite"
    };

    let mut group = c.benchmark_group(format!("Create Operations {backend_name}"));
    group.measurement_time(Duration::from_secs(8));

    // Test creating posts in clean databases of different sizes
    let initial_sizes = vec![0, 100, 500];

    for initial_size in initial_sizes {
        let db = rt.block_on(setup_benchmark_db(initial_size)).unwrap();
        let app = setup_benchmark_app(db);

        let create_data = BenchmarkPostCreate {
            title: "New Benchmark Post".to_string(),
            content: "This is new content for benchmarking create operations with various field types and lengths.".to_string(),
            author: "BenchmarkAuthor".to_string(),
            tags: Some("new, benchmark, create".to_string()),
            published: true,
            category: "NewCategory".to_string(),
            view_count: 0,
            priority: 5,
        };

        group.bench_with_input(
            BenchmarkId::new("create_post", initial_size),
            &initial_size,
            |b, _| {
                b.iter(|| {
                    rt.block_on(std::hint::black_box(benchmark_create_post(
                        app.clone(),
                        create_data.clone(),
                    )))
                });
            },
        );
    }

    group.finish();
}

fn bench_stress_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    // Determine database backend for labeling
    let database_url = get_database_url();
    let backend_name = if database_url.starts_with("postgres") {
        "PostgreSQL"
    } else if database_url.starts_with("mysql") {
        "MySQL"
    } else {
        "SQLite"
    };

    let mut group = c.benchmark_group(format!("Stress Operations {backend_name}"));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(10); // Fewer samples for stress tests

    // Medium dataset stress test (reduced from 2000 for faster benchmarks)
    let db = rt.block_on(setup_benchmark_db(1000)).unwrap();
    let app = setup_benchmark_app(db);

    // Stress test with complex fulltext searches
    let complex_searches = vec![
        "performance database optimization testing",
        "benchmark content queries various keywords",
        "author title tags category published",
    ];

    for query in complex_searches {
        group.bench_with_input(
            BenchmarkId::new("stress_fulltext", query),
            &query,
            |b, query| {
                b.iter(|| {
                    rt.block_on(std::hint::black_box(benchmark_fulltext_search(
                        app.clone(),
                        query,
                    )))
                });
            },
        );
    }

    // Stress test with large pagination (reduced sizes for faster benchmarks)
    let large_page_sizes = vec![250, 500];
    for page_size in large_page_sizes {
        group.bench_with_input(
            BenchmarkId::new("stress_pagination", page_size),
            &page_size,
            |b, page_size| {
                b.iter(|| {
                    rt.block_on(std::hint::black_box(benchmark_paginated_query(
                        app.clone(),
                        0,
                        *page_size,
                    )))
                });
            },
        );
    }

    group.finish();
}

fn configure_criterion() -> Criterion {
    Criterion::default()
        .sample_size(30) // Reduced for faster benchmarks
        .measurement_time(std::time::Duration::from_secs(5)) // Quick measurement
        .warm_up_time(std::time::Duration::from_secs(1))
        .with_plots()
        .with_output_color(true)
}

criterion_group! {
    name = benches;
    config = configure_criterion();
    targets = bench_crud_operations, bench_create_operations, bench_stress_operations
}
criterion_main!(benches);
