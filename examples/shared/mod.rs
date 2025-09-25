use chrono::{DateTime, Utc};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::{Database, DatabaseConnection, entity::prelude::*};
use uuid::Uuid;

/// Shared Todo model used by multiple examples
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

/// Common database setup for examples
pub async fn setup_todo_database(database_url: &str) -> Result<DatabaseConnection, Box<dyn std::error::Error>> {
    let db = Database::connect(database_url).await?;
    
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
    
    Ok(db)
}