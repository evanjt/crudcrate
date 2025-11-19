#![allow(clippy::needless_for_each)]

//! # Lifecycle Hooks Example - Running API
//!
//! This example demonstrates the **three levels of customization** available through the
//! CRUDOperations trait lifecycle hooks system with a fully running API server.
//!
//! ## The Three Customization Levels
//!
//! 1. **Lifecycle Hooks** (`before_*`, `after_*`) - Validation, logging, enrichment
//! 2. **Core Logic** (`fetch_*`, `perform_*`) - Custom queries and business logic
//! 3. **Full Override** (main operations) - Complete control when needed
//!
//! ## Blog Post Management API
//!
//! Features:
//! - Content validation before creation (min 100 chars)
//! - View count tracking after fetch (enrichment)
//! - Published posts filtering (custom query)
//! - S3 image cleanup on delete (full override)
//!
//! Run with: `cargo run --example lifecycle_hooks`

use sea_orm::{
    ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, Set, entity::prelude::*,
};
use std::time::Duration;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};
use uuid::Uuid;

// Import local models
mod models;
use crudcrate::traits::CRUDResource;
use models::blog_post;
use sea_orm::{Condition, Order};

// OpenAPI documentation
#[derive(OpenApi)]
#[openapi(
    info(
        title = "CrudCrate Lifecycle Hooks API",
        description = "Demonstrates lifecycle hooks for CRUD operations: validation, enrichment, custom queries, and cleanup logic. The BlogPost API showcases before/after hooks, custom fetch logic, and full operation overrides.",
        version = "1.0.0",
        contact(
            name = "CrudCrate Documentation",
            url = "https://github.com/evanjt/crudcrate"
        )
    ),
    servers(
        (url = "http://localhost:3000", description = "Development server")
    ),
    tags(
        (name = "blog_posts", description = "Blog post management with lifecycle hooks")
    )
)]
struct ApiDoc;

// ============================================================================
// DATABASE SETUP
// ============================================================================

async fn setup_database() -> DatabaseConnection {
    let mut opt = ConnectOptions::new("sqlite::memory:".to_owned());
    // Force single connection to ensure in-memory DB stays alive
    opt.max_connections(1)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(30))
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(300))
        .max_lifetime(Duration::from_secs(3600))
        .sqlx_logging(false);

    let db = Database::connect(opt).await.unwrap();

    // Create tables and seed data
    create_tables(&db).await;
    seed_data(&db).await;

    db
}

async fn create_tables(db: &DatabaseConnection) {
    use sea_orm::Schema;

    let schema = Schema::new(sea_orm::DatabaseBackend::Sqlite);

    println!("📊 Creating blog_posts table...");
    let stmt = schema.create_table_from_entity(blog_post::Entity);
    match db.execute(db.get_database_backend().build(&stmt)).await {
        Ok(_) => println!("   ✅ blog_posts table created"),
        Err(e) => println!("   ❌ Failed to create blog_posts table: {e:?}"),
    }
}

async fn seed_data(db: &DatabaseConnection) {
    println!("\n🌱 Seeding sample blog posts...");

    let sample_posts = vec![
        (
            "Getting Started with Rust",
            "Rust is a systems programming language that runs blazingly fast, prevents segfaults, and guarantees thread safety. In this comprehensive guide, we'll explore the fundamentals of Rust programming and why it's becoming the language of choice for systems programming.",
            true,
            Some("blog-images/2024/rust-intro.jpg"),
        ),
        (
            "Building REST APIs with Axum",
            "Axum is a web application framework that focuses on ergonomics and modularity. Built on top of Tokio and Hyper, it provides an excellent foundation for building robust APIs. This tutorial will walk you through creating your first REST API with proper error handling, middleware, and OpenAPI documentation.",
            true,
            Some("blog-images/2024/axum-guide.jpg"),
        ),
        (
            "Database Migrations with SeaORM",
            "SeaORM is an async & dynamic ORM for Rust that makes database operations safe and productive. Learn how to set up migrations, define entities, and perform complex queries with this powerful ORM. We'll cover schema versioning, relationship handling, and best practices for production deployments.",
            true,
            None,
        ),
        (
            "Draft: Advanced Rust Patterns",
            "This is a draft post about advanced Rust patterns including the newtype pattern, type-state pattern, and builder pattern. This content will be expanded before publication with detailed code examples and real-world use cases.",
            false,
            None,
        ),
        (
            "Async Programming in Rust",
            "Asynchronous programming in Rust allows you to write concurrent code that's both safe and efficient. We'll dive deep into async/await syntax, futures, streams, and the Tokio runtime. By the end of this guide, you'll understand how to build high-performance async applications that can handle thousands of concurrent connections.",
            true,
            Some("blog-images/2024/async-rust.jpg"),
        ),
    ];

    for (title, content, published, image_s3_key) in sample_posts {
        blog_post::ActiveModel {
            id: Set(Uuid::new_v4()),
            title: Set(title.to_owned()),
            content: Set(content.to_owned()),
            published: Set(published),
            image_s3_key: Set(image_s3_key.map(String::from)),
            view_count: Set(0),
        }
        .insert(db)
        .await
        .unwrap();
    }

    println!("   ✅ Seeded 5 blog posts (4 published, 1 draft)");
}

// ============================================================================
// MAIN APPLICATION
// ============================================================================

#[tokio::main]
async fn main() {
    println!("🚀 CrudCrate Lifecycle Hooks Example\n");

    let db = setup_database().await;

    // Get sample data for help text
    let condition = Condition::all();
    let posts = blog_post::BlogPost::get_all(&db, &condition, blog_post::Column::Title, Order::Asc, 0, 100)
        .await
        .unwrap();

    if let Some(post) = posts.first() {
        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("🌐 Server running on http://localhost:3000");
        println!("📚 OpenAPI Documentation: http://localhost:3000/docs");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

        println!("📊 Dataset Overview:");
        println!("  • {} total blog posts in database", posts.len());
        println!("  • 4 published posts (visible in GET /blog_posts)");
        println!("  • 1 draft post (filtered out by custom fetch_all)");
        println!("  • Some posts have S3 images for cleanup demo\n");

        println!("🎯 Lifecycle Hooks Demonstrated:\n");

        println!("  📝 LEVEL 1: Lifecycle Hooks");
        println!("     • before_create  - Content validation (min 100 chars)");
        println!("     • after_create   - Notification logging");
        println!("     • after_get_one  - View count enrichment");
        println!("     • before_delete  - Permission checks\n");

        println!("  🔍 LEVEL 2: Core Logic Customization");
        println!("     • fetch_all      - Auto-filter to published posts only\n");

        println!("  🚀 LEVEL 3: Full Operation Override");
        println!("     • delete         - S3 cleanup before database delete\n");

        println!("🧪 Try these API calls:\n");

        println!("  # Get all posts (custom fetch_all filters to published only)");
        println!("  curl -s http://localhost:3000/blog_posts | jq .\n");

        println!("  # Get single post (triggers view count enrichment)");
        println!("  curl -s http://localhost:3000/blog_posts/{} | jq .", post.id);
        println!("  # Notice the view_count field is populated!\n");

        println!("  # Create new post (validation hook requires 100+ chars)");
        println!("  curl -X POST http://localhost:3000/blog_posts \\");
        println!("    -H 'Content-Type: application/json' \\");
        println!("    -d '{{");
        println!("      \"title\": \"Test Post\",");
        println!("      \"content\": \"Short\",");
        println!("      \"published\": true");
        println!("    }}' | jq .");
        println!("  # This will FAIL validation (content too short)\n");

        println!("  # Create valid post (100+ chars)");
        println!("  curl -X POST http://localhost:3000/blog_posts \\");
        println!("    -H 'Content-Type: application/json' \\");
        println!("    -d '{{");
        println!("      \"title\": \"Valid Post\",");
        println!("      \"content\": \"{}\"", "This is a valid blog post with enough content to pass validation. ".repeat(2));
        println!("      \"published\": true");
        println!("    }}' | jq .\n");

        println!("  # Delete post with S3 image (full override with cleanup)");
        println!("  curl -X DELETE http://localhost:3000/blog_posts/{}", post.id);
        println!("  # Watch the server logs for S3 cleanup messages!\n");

        println!("  # Update a post");
        println!("  curl -X PUT http://localhost:3000/blog_posts/{} \\", post.id);
        println!("    -H 'Content-Type: application/json' \\");
        println!("    -d '{{\"title\": \"Updated Title\"}}' | jq .\n");

        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    } else {
        println!("⚠️ No blog posts found in database");
    }

    // Build the router with OpenAPI documentation
    let (router, openapi) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/blog_posts", blog_post::BlogPost::router(&db))
        .split_for_parts();

    let app = router
        .merge(Scalar::with_url("/docs", openapi))
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
