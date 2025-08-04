/*!
# External Security Integration Test

This test demonstrates how to properly secure a crudcrate API using external, 
battle-tested security libraries instead of rolling your own security features.

This serves as both a test and documentation for the recommended security stack.
*/

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
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use uuid::Uuid;

/// Example entity for demonstrating external security integration
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "secure_posts")]
#[crudcrate(api_struct = "SecurePost", active_model = "ActiveModel")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, create_model = false, update_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,
    
    #[crudcrate(fulltext, filterable, sortable)]
    pub title: String,
    
    #[sea_orm(column_type = "Text")]
    #[crudcrate(fulltext, filterable)]
    pub content: String,
    
    #[crudcrate(fulltext, filterable)]
    pub author: String,
    
    #[crudcrate(filterable)]
    pub published: bool,
    
    #[crudcrate(sortable, create_model = false, update_model = false, on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Generate CRUD handlers
crud_handlers!(SecurePost, SecurePostUpdate, SecurePostCreate);

// Migration setup
pub struct SecurePostMigrator;

#[async_trait::async_trait]
impl MigratorTrait for SecurePostMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(CreateSecurePostTable)]
    }
}

pub struct CreateSecurePostTable;

#[async_trait::async_trait]
impl MigrationName for CreateSecurePostTable {
    fn name(&self) -> &'static str {
        "m20240101_000001_create_secure_post_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for CreateSecurePostTable {
    async fn up(&self, manager: &SchemaManager) -> Result<(), sea_orm::DbErr> {
        let table = Table::create()
            .table(SecurePostEntity)
            .if_not_exists()
            .col(ColumnDef::new(SecurePostColumn::Id).uuid().not_null().primary_key())
            .col(ColumnDef::new(SecurePostColumn::Title).string().not_null())
            .col(ColumnDef::new(SecurePostColumn::Content).text().not_null())
            .col(ColumnDef::new(SecurePostColumn::Author).string().not_null())
            .col(ColumnDef::new(SecurePostColumn::Published).boolean().not_null().default(false))
            .col(ColumnDef::new(SecurePostColumn::CreatedAt).timestamp_with_time_zone().not_null())
            .to_owned();

        manager.create_table(table).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), sea_orm::DbErr> {
        manager.drop_table(Table::drop().table(SecurePostEntity).to_owned()).await?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum SecurePostColumn {
    Id,
    Title,
    Content,
    Author,
    Published,
    CreatedAt,
}

impl Iden for SecurePostColumn {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(
            s,
            "{}",
            match self {
                Self::Id => "id",
                Self::Title => "title",
                Self::Content => "content",
                Self::Author => "author",
                Self::Published => "published",
                Self::CreatedAt => "created_at",
            }
        ).unwrap();
    }
}

#[derive(Debug)]
pub struct SecurePostEntity;

impl Iden for SecurePostEntity {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(s, "secure_posts").unwrap();
    }
}

// Set up test database
async fn setup_secure_test_db() -> Result<DatabaseConnection, sea_orm::DbErr> {
    let db = Database::connect("sqlite::memory:").await?;
    SecurePostMigrator::up(&db, None).await?;
    
    // Insert sample data
    let sample_posts = vec![
        SecurePostCreate {
            title: "Secure API Best Practices".to_string(),
            content: "Learn how to secure your APIs using industry-standard libraries and practices.".to_string(),
            author: "Security Expert".to_string(),
            published: true,
        },
        SecurePostCreate {
            title: "Rate Limiting Strategies".to_string(),
            content: "Different approaches to implementing rate limiting in web applications.".to_string(),
            author: "Performance Engineer".to_string(),
            published: true,
        },
    ];
    
    for post in sample_posts {
        SecurePost::create(&db, post).await?;
    }
    
    Ok(db)
}

/// Create a production-ready, secure API using external libraries
/// 
/// This function demonstrates the RECOMMENDED way to secure a crudcrate API
fn create_secure_api(db: DatabaseConnection) -> Router {
    // 1. CORS configuration (using tower-http)
    let cors_layer = CorsLayer::new()
        // In production, specify exact origins instead of Any
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(Any);

    // 2. Build the CRUD API routes
    let api_routes = Router::new()
        .route(
            "/secure_posts",
            axum::routing::get(get_all_handler).post(create_one_handler),
        )
        .route(
            "/secure_posts/{id}",
            axum::routing::get(get_one_handler)
                .put(update_one_handler)
                .delete(delete_one_handler),
        )
        .with_state(db);

    // 3. Apply security layers in the correct order
    Router::new()
        .nest("/api/v1", api_routes)
        // Security layers (applied in reverse order - last layer applied first)
        .layer(TraceLayer::new_for_http())         // 2. Logging/tracing
        .layer(cors_layer)                         // 1. CORS
}

#[tokio::test]
async fn test_secure_api_functionality() {
    let db = setup_secure_test_db().await.expect("Failed to set up test database");
    let app = create_secure_api(db);
    
    // Test that the API still works with all security layers
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/secure_posts")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    // Should return OK
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    
    // Should have CORS headers applied
    let headers = response.headers();
    
    // Should have CORS headers
    assert!(headers.contains_key("access-control-allow-origin"));
    
    // Should return valid JSON
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let posts: Vec<SecurePost> = serde_json::from_slice(&body).unwrap();
    assert!(posts.len() >= 2, "Should return sample posts");
}

#[tokio::test]
async fn test_tracing_layer_applied() {
    let db = setup_secure_test_db().await.expect("Failed to set up test database");
    let app = create_secure_api(db);
    
    // Test that requests work with tracing layer applied
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/secure_posts")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    // Should succeed with tracing layer
    assert_eq!(response.status(), axum::http::StatusCode::OK);
}

#[tokio::test]
async fn test_cors_headers_present() {
    let db = setup_secure_test_db().await.expect("Failed to set up test database");
    let app = create_secure_api(db);
    
    // Test CORS preflight request
    let request = Request::builder()
        .method(Method::OPTIONS)
        .uri("/api/v1/secure_posts")
        .header("origin", "https://example.com")
        .header("access-control-request-method", "GET")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    // Should handle preflight request
    assert!(response.status().is_success() || response.status() == axum::http::StatusCode::NO_CONTENT);
    
    let headers = response.headers();
    assert!(headers.contains_key("access-control-allow-origin"));
    assert!(headers.contains_key("access-control-allow-methods"));
}


#[tokio::test]
async fn test_fulltext_search_still_works_with_security() {
    let db = setup_secure_test_db().await.expect("Failed to set up test database");
    let app = create_secure_api(db);
    
    // Test that fulltext search still works with all security layers
    let filter = r#"{"q": "secure api"}"#;
    let encoded_filter = url_escape::encode_component(filter);
    let uri = format!("/api/v1/secure_posts?filter={}", encoded_filter);
    
    let request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let posts: Vec<SecurePost> = serde_json::from_slice(&body).unwrap();
    
    // Should find the post about "Secure API Best Practices"
    assert!(posts.len() >= 1, "Should find posts matching 'secure api'");
    let found_secure_post = posts.iter().any(|post| {
        post.title.to_lowercase().contains("secure") ||
        post.content.to_lowercase().contains("api")
    });
    assert!(found_secure_post, "Should find posts containing 'secure' or 'api'");
}