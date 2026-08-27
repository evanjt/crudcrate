//! Join loading at depth 2 when no entity names its primary key field `id`.

use axum::http::StatusCode;
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr};
use serde_json::json;
use test_suite::http;
use uuid::Uuid;

pub mod library {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "jpk_libraries")]
    #[crudcrate(generate_router, api_struct = "Library", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub library_id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, exclude(create, update), join(one, all, depth = 2))]
        pub shelves: Vec<super::shelf::Shelf>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::shelf::Entity")]
        Shelves,
    }

    impl Related<super::shelf::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Shelves.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod shelf {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "jpk_shelves")]
    #[crudcrate(generate_router, api_struct = "Shelf", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub shelf_id: Uuid,

        #[crudcrate(filterable)]
        pub library_id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub label: String,

        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, exclude(create, update), join(one, all, depth = 1))]
        pub books: Vec<super::book::Book>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::library::Entity",
            from = "Column::LibraryId",
            to = "super::library::Column::LibraryId"
        )]
        Library,
        #[sea_orm(has_many = "super::book::Entity")]
        Books,
    }

    impl Related<super::library::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Library.def()
        }
    }

    impl Related<super::book::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Books.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod book {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "jpk_books")]
    #[crudcrate(generate_router, api_struct = "Book", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub book_id: Uuid,

        #[crudcrate(filterable)]
        pub shelf_id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub title: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::shelf::Entity",
            from = "Column::ShelfId",
            to = "super::shelf::Column::ShelfId"
        )]
        Shelf,
    }

    impl Related<super::shelf::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Shelf.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(library::Entity, shelf::Entity, book::Entity).await
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new()
        .nest("/libraries", library::Library::router(db).into())
        .nest("/shelves", shelf::Shelf::router(db).into())
        .nest("/books", book::Book::router(db).into())
}

async fn create(app: &axum::Router, path: &str, payload: serde_json::Value) -> serde_json::Value {
    let (status, body) = http::post(app, path, &payload).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
}

async fn seed(app: &axum::Router) -> String {
    let library = create(app, "/libraries", json!({"name": "Central"})).await;
    let library_id = library["library_id"].as_str().unwrap().to_string();
    for label in ["A", "B"] {
        let shelf = create(
            app,
            "/shelves",
            json!({"library_id": library_id, "label": label}),
        )
        .await;
        let shelf_id = shelf["shelf_id"].as_str().unwrap();
        for n in 1..=2 {
            create(
                app,
                "/books",
                json!({"shelf_id": shelf_id, "title": format!("{label}{n}")}),
            )
            .await;
        }
    }
    library_id
}

fn book_titles(library: &serde_json::Value) -> Vec<String> {
    let mut titles: Vec<String> = library["shelves"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|shelf| shelf["books"].as_array().unwrap().iter())
        .map(|book| book["title"].as_str().unwrap().to_string())
        .collect();
    titles.sort();
    titles
}

#[tokio::test]
async fn get_one_loads_grandchildren_through_non_id_primary_keys() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);
    let library_id = seed(&app).await;

    let (status, library) = http::get(&app, &format!("/libraries/{library_id}")).await;
    assert_eq!(status, StatusCode::OK, "{library}");
    assert_eq!(library["shelves"].as_array().unwrap().len(), 2);
    assert_eq!(book_titles(&library), ["A1", "A2", "B1", "B2"]);
}

#[tokio::test]
async fn get_all_loads_grandchildren_through_non_id_primary_keys() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);
    seed(&app).await;

    let (status, libraries) = http::get(&app, "/libraries").await;
    assert_eq!(status, StatusCode::OK, "{libraries}");
    let libraries = libraries.as_array().unwrap();
    assert_eq!(libraries.len(), 1);
    assert_eq!(book_titles(&libraries[0]), ["A1", "A2", "B1", "B2"]);
}
