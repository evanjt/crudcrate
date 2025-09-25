//! Minimal CRUD API Example with Axum
//!
//! ```bash
//! cargo run --example minimal
//! ```
//!
//! Then visit:
//! - **API**: <http://localhost:3000/todo>
//! - **Documentation**: <http://localhost:3000/docs>

#![allow(clippy::needless_for_each)]

mod shared;

use shared::{Todo, setup_todo_database};
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
    let db = setup_todo_database(&database_url).await?;

    let (router, apidocs) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/todo", Todo::router(&db))
        .split_for_parts();
    let app = router.merge(Scalar::with_url("/docs", apidocs));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("🚀 API: http://0.0.0.0:3000/todo\n📖 Docs: http://0.0.0.0:3000/docs");
    axum::serve(listener, app).await?;
    Ok(())
}
