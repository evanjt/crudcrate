//! `join(relation = "...")` selects the child's `Relation` variant, so a child
//! with two foreign keys to the same parent can be joined through each.

use axum::http::StatusCode;
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr};
use serde_json::json;
use test_suite::http;
use uuid::Uuid;

pub mod person {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "jrl_people")]
    #[crudcrate(generate_router, api_struct = "Person", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[sea_orm(ignore)]
        #[crudcrate(
            non_db_attr = true,
            exclude(create, update),
            join(one, all, depth = 1, relation = "Author")
        )]
        pub authored: Vec<super::document::Document>,

        #[sea_orm(ignore)]
        #[crudcrate(
            non_db_attr = true,
            exclude(create, update),
            join(one, all, depth = 1, relation = "Reviewer")
        )]
        pub reviewed: Vec<super::document::Document>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::document::Entity")]
        Documents,
    }

    impl Related<super::document::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Documents.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod document {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "jrl_documents")]
    #[crudcrate(generate_router, api_struct = "Document", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable)]
        pub author_id: Uuid,

        #[crudcrate(filterable)]
        pub reviewer_id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub title: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::person::Entity",
            from = "Column::AuthorId",
            to = "super::person::Column::Id"
        )]
        Author,
        #[sea_orm(
            belongs_to = "super::person::Entity",
            from = "Column::ReviewerId",
            to = "super::person::Column::Id"
        )]
        Reviewer,
    }

    impl Related<super::person::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Author.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(person::Entity, document::Entity).await
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new()
        .nest("/people", person::Person::router(db).into())
        .nest("/documents", document::Document::router(db).into())
}

async fn create(app: &axum::Router, path: &str, payload: serde_json::Value) -> String {
    let (status, body) = http::post(app, path, &payload).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_str().unwrap().to_string()
}

fn titles(person: &serde_json::Value, field: &str) -> Vec<String> {
    let mut titles: Vec<String> = person[field]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["title"].as_str().unwrap().to_string())
        .collect();
    titles.sort();
    titles
}

async fn seed(app: &axum::Router) -> (String, String) {
    let ada = create(app, "/people", json!({"name": "Ada"})).await;
    let bob = create(app, "/people", json!({"name": "Bob"})).await;
    for (title, author, reviewer) in [
        ("Spec", &ada, &bob),
        ("Draft", &ada, &bob),
        ("Notes", &bob, &ada),
    ] {
        create(
            app,
            "/documents",
            json!({"title": title, "author_id": author, "reviewer_id": reviewer}),
        )
        .await;
    }
    (ada, bob)
}

#[tokio::test]
async fn get_one_loads_each_relation_through_its_own_foreign_key() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);
    let (ada, bob) = seed(&app).await;

    let (status, ada) = http::get(&app, &format!("/people/{ada}")).await;
    assert_eq!(status, StatusCode::OK, "{ada}");
    assert_eq!(titles(&ada, "authored"), ["Draft", "Spec"]);
    assert_eq!(titles(&ada, "reviewed"), ["Notes"]);

    let (_, bob) = http::get(&app, &format!("/people/{bob}")).await;
    assert_eq!(titles(&bob, "authored"), ["Notes"]);
    assert_eq!(titles(&bob, "reviewed"), ["Draft", "Spec"]);
}

#[tokio::test]
async fn get_all_loads_each_relation_through_its_own_foreign_key() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);
    seed(&app).await;

    let (status, people) = http::get(&app, "/people?sort=%5B%22name%22%2C%22ASC%22%5D").await;
    assert_eq!(status, StatusCode::OK, "{people}");
    let people = people.as_array().unwrap();
    assert_eq!(people.len(), 2);
    assert_eq!(titles(&people[0], "authored"), ["Draft", "Spec"]);
    assert_eq!(titles(&people[0], "reviewed"), ["Notes"]);
    assert_eq!(titles(&people[1], "authored"), ["Notes"]);
    assert_eq!(titles(&people[1], "reviewed"), ["Draft", "Spec"]);
}
