//! Minimal CRUD API Example with Spring-RS
//!
//! This demonstrates using crudcrate's Axum router within a Spring-RS application.
//! No Spring-RS plugins needed - you manage your own SeaORM database!
//!
//! ## Run the Example
//!
//! ```bash
//! cargo run --example minimal_spring --features "derive,spring-rs"
//! ```
//!
//! Then visit:
//! - **API**: <http://localhost:8080/todo>      (todos endpoint)
//! - **GET**: <http://localhost:8080/todo/{id}> (get by ID)

use chrono::{DateTime, Utc};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::{Database, DatabaseConnection, entity::prelude::*};
use spring::{App, auto_config};
use spring_web::{WebConfigurator, WebPlugin};
use std::env;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, EntityToModels)]
#[sea_orm(table_name = "todos")]
#[crudcrate(
    description = "Simple todo management",
    generate_router
)]
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

#[auto_config(WebConfigurator)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up your own database (no Spring-RS plugins needed!)
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let db: DatabaseConnection = Database::connect(&database_url).await?;

    // Create the table
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

    println!("Spring-RS + crudcrate API: http://localhost:8080/");

    // Use crudcrate's generated Axum router in Spring-RS!
    App::new()
        .add_plugin(WebPlugin)
        .add_router(router_with_path(&db, "/todo").into()) // Mount router at /todo
        .run()
        .await;

    Ok(())
}
