//! Scoped Access Example — Public vs Admin endpoints
//!
//! Demonstrates using `ScopeCondition` and `read_only_router()` to serve
//! different data based on authentication context.
//!
//! ```bash
//! cargo run --example scoped_access
//! ```
//!
//! Then try:
//! - **Public API** (read-only, non-private only): <http://localhost:3000/public/articles>
//! - **Admin API** (full CRUD, all records): <http://localhost:3000/admin/articles>
//! - **Docs**: <http://localhost:3000/docs>

mod article;

use article::{Article, setup_article_database};
use axum::{Extension, Router};
use crudcrate::ScopeCondition;
use sea_orm::{ColumnTrait, Condition};
use std::env;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};

#[derive(OpenApi)]
#[openapi()]
struct ApiDoc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let db = setup_article_database(&database_url).await?;

    // Seed some test data
    seed_articles(&db).await;

    // Admin router: full CRUD, all records
    let admin_router = Article::router(&db);

    // Public router: read-only, only non-private articles
    let public_router = Article::read_only_router(&db).layer(Extension(ScopeCondition::new(
        Condition::all().add(article::Column::IsPrivate.eq(false)),
    )));

    let (router, apidocs) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/admin/articles", admin_router)
        .nest("/public/articles", public_router)
        .split_for_parts();

    let app: Router = router.merge(Scalar::with_url("/docs", apidocs));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Admin API:  http://0.0.0.0:3000/admin/articles  (full CRUD, all records)");
    println!("Public API: http://0.0.0.0:3000/public/articles (read-only, non-private)");
    println!("Docs:       http://0.0.0.0:3000/docs");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn seed_articles(db: &sea_orm::DatabaseConnection) {
    use article::ActiveModel;
    use sea_orm::{ActiveModelTrait, Set};

    let articles = vec![
        ("Public article 1", "This is visible to everyone", false),
        ("Public article 2", "Also visible to everyone", false),
        ("Private draft", "Only visible to admins", true),
        ("Internal notes", "Admin eyes only", true),
    ];

    for (title, body, is_private) in articles {
        let article = ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            title: Set(title.to_string()),
            body: Set(body.to_string()),
            is_private: Set(is_private),
        };
        article.insert(db).await.ok();
    }
    println!("Seeded {} articles (2 public, 2 private)", 4);
}
