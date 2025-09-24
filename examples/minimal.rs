//! Minimal CRUD API Example with Axum
//!
//! ```bash
//! cargo run --example minimal
//! ```
//!
//! Then visit:
//! - **API**: <http://localhost:3000/todo>
//! - **Documentation**: <http://localhost:3000/docs>

use chrono::{DateTime, Utc};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::{Database, DatabaseConnection, entity::prelude::*};
use std::env;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, EntityToModels)]
#[sea_orm(table_name = "todos")]
#[crudcrate(api_struct = "Todo", description = "Simple todo management", generate_router)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, create_model = false, update_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,
    #[crudcrate(sortable, filterable)]
    pub title: String,
    #[crudcrate(filterable, on_create = false)]
    pub completed: bool,
    #[crudcrate(create_model = false, update_model = false, on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

#[derive(OpenApi)]
#[openapi()]
struct ApiDoc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let db: DatabaseConnection = Database::connect(&database_url).await?;

    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        r"CREATE TABLE IF NOT EXISTS todos (
            id TEXT PRIMARY KEY NOT NULL,
            title TEXT NOT NULL,
            completed BOOLEAN NOT NULL,
            updated_at TEXT NOT NULL
        );"
        .to_owned(),
    ))
    .await?;

    let (router, apidocs) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/todo", Todo::router(&db))
        .split_for_parts();
    let app = router.merge(Scalar::with_url("/docs", apidocs));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("🚀 API: http://0.0.0.0:3000/todo\n📖 Docs: http://0.0.0.0:3000/docs");
    axum::serve(listener, app).await?;
    Ok(())
}
