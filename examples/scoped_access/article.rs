use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::{Database, DatabaseConnection, entity::prelude::*};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, EntityToModels)]
#[sea_orm(table_name = "articles")]
#[crudcrate(
    api_struct = "Article",
    description = "Articles with public/private visibility",
    generate_router
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,
    #[crudcrate(sortable, filterable, fulltext)]
    pub title: String,
    #[crudcrate(filterable, fulltext)]
    pub body: String,
    #[crudcrate(filterable, on_create = false)]
    pub is_private: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub async fn setup_article_database(
    database_url: &str,
) -> Result<DatabaseConnection, Box<dyn std::error::Error>> {
    let db = Database::connect(database_url).await?;

    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        r"CREATE TABLE IF NOT EXISTS articles (
            id TEXT PRIMARY KEY NOT NULL,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            is_private BOOLEAN NOT NULL DEFAULT 0
        );"
        .to_owned(),
    ))
    .await?;

    Ok(db)
}
