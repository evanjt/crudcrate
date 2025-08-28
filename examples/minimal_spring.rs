//! Minimal CRUD API Example with Spring-RS
//! 
//! This is a **complete CRUD API** implemented in just ~60 lines of code using crudcrate with Spring-RS.
//! 
//! ## What You Get
//! 
//! - ✅ Full CRUD operations (GET, POST, PUT, DELETE)
//! - ✅ Spring-RS framework integration
//! - ✅ Sortable and filterable endpoints
//! - ✅ Auto-generated primary keys and timestamps
//! - ✅ SQLite in-memory database (no setup required)
//! 
//! ## Run the Example
//! 
//! ```bash
//! cargo run --example minimal_spring
//! ```
//! 
//! Then visit:
//! - **API**: http://localhost:3000/todo

use chrono::{DateTime, Utc};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::{Database, DatabaseConnection, entity::prelude::*};
use std::env;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, EntityToModels)]
#[sea_orm(table_name = "todos")]
#[crudcrate(description = "Simple todo management", framework = "spring-rs", generate_router)]
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let db: DatabaseConnection = Database::connect(&database_url).await?;

    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        r#"CREATE TABLE IF NOT EXISTS todos (
            id TEXT PRIMARY KEY NOT NULL,
            title TEXT NOT NULL,
            completed BOOLEAN NOT NULL,
            updated_at TEXT NOT NULL
        );"#
        .to_owned(),
    ))
    .await?;

    // Initialize Spring-RS application with the generated router
    use spring_web::WebApp;
    
    let app = WebApp::new()
        .routes(router(&db));
    
    println!("🚀 Spring-RS API: http://0.0.0.0:3000/todo");
    app.listen("0.0.0.0:3000").await?;
    Ok(())
}