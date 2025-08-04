use axum::{
    body::Body,
    http::{Method, Request},
    Router,
};
use chrono::{DateTime, Utc};
use crudcrate::{crud_handlers, traits::CRUDResource};
use crudcrate_derive::EntityToModels;
use sea_orm::{entity::prelude::*, Database, DatabaseConnection};
use sea_orm_migration::{prelude::*, sea_query::ColumnDef};
use tower::ServiceExt;
use uuid::Uuid;

// Define a test entity with fulltext search capabilities for integration testing
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "blog_posts")]
#[crudcrate(api_struct = "BlogPost", active_model = "ActiveModel")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, create_model = false, update_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,
    
    #[crudcrate(fulltext, filterable, sortable)]
    pub title: String,
    
    #[sea_orm(column_type = "Text")]
    #[crudcrate(fulltext)]
    pub content: String,
    
    #[crudcrate(fulltext, filterable)]
    pub author: String,
    
    #[sea_orm(column_type = "Text", nullable)]
    #[crudcrate(fulltext)]
    pub tags: Option<String>,
    
    #[crudcrate(filterable)]
    pub published: bool,
    
    #[crudcrate(fulltext)]
    pub view_count: i32,
    
    #[crudcrate(sortable, create_model = false, update_model = false, on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Generate CRUD handlers using the macro
crud_handlers!(BlogPost, BlogPostUpdate, BlogPostCreate);

// Migration for the test database
pub struct BlogPostMigrator;

#[async_trait::async_trait]
impl MigratorTrait for BlogPostMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(CreateBlogPostTable)]
    }
}

pub struct CreateBlogPostTable;

#[async_trait::async_trait]
impl MigrationName for CreateBlogPostTable {
    fn name(&self) -> &'static str {
        "m20240101_000001_create_blog_post_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for CreateBlogPostTable {
    async fn up(&self, manager: &SchemaManager) -> Result<(), sea_orm::DbErr> {
        let table = Table::create()
            .table(BlogPostEntity)
            .if_not_exists()
            .col(ColumnDef::new(BlogPostColumn::Id).uuid().not_null().primary_key())
            .col(ColumnDef::new(BlogPostColumn::Title).string().not_null())
            .col(ColumnDef::new(BlogPostColumn::Content).text().not_null())
            .col(ColumnDef::new(BlogPostColumn::Author).string().not_null())
            .col(ColumnDef::new(BlogPostColumn::Tags).text().null())
            .col(ColumnDef::new(BlogPostColumn::Published).boolean().not_null().default(false))
            .col(ColumnDef::new(BlogPostColumn::ViewCount).integer().not_null().default(0))
            .col(ColumnDef::new(BlogPostColumn::CreatedAt).timestamp_with_time_zone().not_null())
            .to_owned();

        manager.create_table(table).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), sea_orm::DbErr> {
        manager.drop_table(Table::drop().table(BlogPostEntity).to_owned()).await?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum BlogPostColumn {
    Id,
    Title,
    Content,
    Author,
    Tags,
    Published,
    ViewCount,
    CreatedAt,
}

impl Iden for BlogPostColumn {
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
                Self::ViewCount => "view_count",
                Self::CreatedAt => "created_at",
            }
        ).unwrap();
    }
}

#[derive(Debug)]
pub struct BlogPostEntity;

impl Iden for BlogPostEntity {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(s, "blog_posts").unwrap();
    }
}

// Helper function to set up test database with sample data for fulltext testing
async fn setup_fulltext_test_db() -> Result<DatabaseConnection, sea_orm::DbErr> {
    let db = Database::connect("sqlite::memory:").await?;
    
    // Run migrations
    BlogPostMigrator::up(&db, None).await?;
    
    // Insert sample data optimized for fulltext search testing
    let sample_posts = vec![
        BlogPostCreate {
            title: "Getting Started with Rust Programming".to_string(),
            content: "Rust is a systems programming language that runs blazingly fast, prevents segfaults, and guarantees thread safety. This comprehensive guide covers the basics of Rust programming including ownership, borrowing, and lifetimes.".to_string(),
            author: "Jane Smith".to_string(),
            tags: Some("rust, programming, tutorial, beginner".to_string()),
            published: true,
            view_count: 1500,
        },
        BlogPostCreate {
            title: "Advanced Web Development with Axum Framework".to_string(),
            content: "Learn how to build high-performance web applications using Axum, a modern async web framework for Rust. We'll cover routing, middleware, state management, and database integration with Sea-ORM.".to_string(),
            author: "John Doe".to_string(),
            tags: Some("axum, web development, rust, async, framework".to_string()),
            published: true,
            view_count: 850,
        },
        BlogPostCreate {
            title: "Database Design Patterns in Modern Applications".to_string(),
            content: "Explore various database design patterns including CQRS, Event Sourcing, and Repository pattern. Learn how to implement these patterns effectively in your applications for better scalability and maintainability.".to_string(),
            author: "Alice Johnson".to_string(),
            tags: Some("database, design patterns, architecture, scalability".to_string()),
            published: true,
            view_count: 920,
        },
        BlogPostCreate {
            title: "Machine Learning Fundamentals".to_string(),
            content: "An introduction to machine learning concepts including supervised learning, unsupervised learning, and neural networks. Perfect for beginners looking to understand AI and ML basics.".to_string(),
            author: "Bob Wilson".to_string(),
            tags: Some("machine learning, AI, neural networks, python".to_string()),
            published: false,
            view_count: 340,
        },
        BlogPostCreate {
            title: "Microservices Architecture Best Practices".to_string(),
            content: "Learn how to design and implement microservices architecture. Covers service discovery, API gateways, distributed tracing, and communication patterns between services.".to_string(),
            author: "Sarah Chen".to_string(),
            tags: Some("microservices, architecture, distributed systems, api".to_string()),
            published: true,
            view_count: 1200,
        },
    ];
    
    // Insert sample data
    for post in sample_posts {
        BlogPost::create(&db, post).await?;
    }
    
    Ok(db)
}

// Set up the test API router
fn setup_fulltext_test_app(db: DatabaseConnection) -> Router {
    let api = Router::new()
        .route(
            "/blog_posts",
            axum::routing::get(get_all_handler).post(create_one_handler),
        )
        .route(
            "/blog_posts/{id}",
            axum::routing::get(get_one_handler)
                .put(update_one_handler)
                .delete(delete_one_handler),
        )
        .with_state(db);

    Router::new().nest("/api/v1", api)
}

#[tokio::test]
async fn test_basic_fulltext_search() {
    let db = setup_fulltext_test_db().await.expect("Failed to set up test database");
    let app = setup_fulltext_test_app(db);
    
    // Test basic fulltext search for "rust programming"
    let filter = r#"{"q": "rust programming"}"#;
    let encoded_filter = url_escape::encode_component(filter);
    let uri = format!("/api/v1/blog_posts?filter={}", encoded_filter);
    
    let request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let posts: Vec<BlogPost> = serde_json::from_slice(&body).unwrap();
    
    // Should find posts with "rust" and "programming" in title, content, or tags
    assert!(posts.len() >= 1, "Should find at least one post matching 'rust programming'");
    
    // Verify that found posts actually contain the search terms
    let found_rust_post = posts.iter().any(|post| {
        post.title.to_lowercase().contains("rust") || 
        post.content.to_lowercase().contains("rust") ||
        post.tags.as_ref().map_or(false, |tags| tags.to_lowercase().contains("rust"))
    });
    assert!(found_rust_post, "Should find posts containing 'rust'");
}

#[tokio::test]
async fn test_author_search() {
    let db = setup_fulltext_test_db().await.expect("Failed to set up test database");
    let app = setup_fulltext_test_app(db);
    
    // Test searching for author name
    let filter = r#"{"q": "Jane Smith"}"#;
    let encoded_filter = url_escape::encode_component(filter);
    let uri = format!("/api/v1/blog_posts?filter={}", encoded_filter);
    
    let request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let posts: Vec<BlogPost> = serde_json::from_slice(&body).unwrap();
    
    // Should find the post by Jane Smith
    assert!(posts.len() >= 1, "Should find at least one post by Jane Smith");
    
    let found_jane_post = posts.iter().any(|post| post.author == "Jane Smith");
    assert!(found_jane_post, "Should find post by Jane Smith");
}

#[tokio::test]
async fn test_numeric_field_search() {
    let db = setup_fulltext_test_db().await.expect("Failed to set up test database");
    let app = setup_fulltext_test_app(db);
    
    // Test searching for numeric content (view_count is included in fulltext)
    let filter = r#"{"q": "1500"}"#;
    let encoded_filter = url_escape::encode_component(filter);
    let uri = format!("/api/v1/blog_posts?filter={}", encoded_filter);
    
    let request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let posts: Vec<BlogPost> = serde_json::from_slice(&body).unwrap();
    
    // Should find the post with view_count 1500
    assert!(posts.len() >= 1, "Should find post with view count 1500");
    
    let found_1500_views = posts.iter().any(|post| post.view_count == 1500);
    assert!(found_1500_views, "Should find post with 1500 views");
}

#[tokio::test]
async fn test_combined_filter_and_fulltext() {
    let db = setup_fulltext_test_db().await.expect("Failed to set up test database");
    let app = setup_fulltext_test_app(db);
    
    // Test combining fulltext search with regular filters
    let filter = r#"{"q": "web development", "published": true}"#;
    let encoded_filter = url_escape::encode_component(filter);
    let uri = format!("/api/v1/blog_posts?filter={}", encoded_filter);
    
    let request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let posts: Vec<BlogPost> = serde_json::from_slice(&body).unwrap();
    
    // Should find published posts containing "web development"
    for post in &posts {
        assert!(post.published, "All returned posts should be published");
        
        let contains_search_term = post.title.to_lowercase().contains("web") ||
            post.title.to_lowercase().contains("development") ||
            post.content.to_lowercase().contains("web") ||
            post.content.to_lowercase().contains("development") ||
            post.tags.as_ref().map_or(false, |tags| 
                tags.to_lowercase().contains("web") || tags.to_lowercase().contains("development")
            );
        
        assert!(contains_search_term, "Post should contain search terms: {}", post.title);
    }
}

#[tokio::test]
async fn test_fulltext_with_pagination() {
    let db = setup_fulltext_test_db().await.expect("Failed to set up test database");
    let app = setup_fulltext_test_app(db);
    
    // Test fulltext search with pagination
    let filter = r#"{"q": "learning"}"#;
    let encoded_filter = url_escape::encode_component(filter);
    let uri = format!("/api/v1/blog_posts?filter={}&page=0&per_page=2", encoded_filter);
    
    let request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    
    // Check Content-Range header for pagination info
    let headers = response.headers();
    assert!(headers.contains_key("content-range"), "Should include Content-Range header for pagination");
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let posts: Vec<BlogPost> = serde_json::from_slice(&body).unwrap();
    
    // Should respect pagination limit
    assert!(posts.len() <= 2, "Should return at most 2 posts due to pagination");
}

#[tokio::test]
async fn test_fulltext_with_sorting() {
    let db = setup_fulltext_test_db().await.expect("Failed to set up test database");
    let app = setup_fulltext_test_app(db);
    
    // Test fulltext search with sorting by title
    let filter = r#"{"q": "rust web"}"#;
    let encoded_filter = url_escape::encode_component(filter);
    let uri = format!("/api/v1/blog_posts?filter={}&sort=title&order=ASC", encoded_filter);
    
    let request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let posts: Vec<BlogPost> = serde_json::from_slice(&body).unwrap();
    
    // Verify posts are sorted by title in ascending order
    if posts.len() > 1 {
        for i in 0..posts.len()-1 {
            assert!(
                posts[i].title <= posts[i+1].title,
                "Posts should be sorted by title in ascending order"
            );
        }
    }
}

#[tokio::test]
async fn test_empty_fulltext_search() {
    let db = setup_fulltext_test_db().await.expect("Failed to set up test database");
    let app = setup_fulltext_test_app(db);
    
    // Test empty search query
    let filter = r#"{"q": ""}"#;
    let encoded_filter = url_escape::encode_component(filter);
    let uri = format!("/api/v1/blog_posts?filter={}", encoded_filter);
    
    let request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let posts: Vec<BlogPost> = serde_json::from_slice(&body).unwrap();
    
    // Empty search should handle gracefully
    // Note: length is always >= 0, this just verifies no panic occurred
}

#[tokio::test]
async fn test_fulltext_search_no_results() {
    let db = setup_fulltext_test_db().await.expect("Failed to set up test database");
    let app = setup_fulltext_test_app(db);
    
    // Test search for something that doesn't exist
    let filter = r#"{"q": "nonexistent term xyz123"}"#;
    let encoded_filter = url_escape::encode_component(filter);
    let uri = format!("/api/v1/blog_posts?filter={}", encoded_filter);
    
    let request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let posts: Vec<BlogPost> = serde_json::from_slice(&body).unwrap();
    
    // Should return empty array for no matches
    assert_eq!(posts.len(), 0, "Should return no posts for nonexistent search term");
}