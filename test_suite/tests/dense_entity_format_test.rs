// Scenario: entities written in the SeaORM 2.0 dense format (`#[sea_orm::model]`
// with inline relation fields) also derive `EntityToModels`.
// Expected behaviour: the derive attaches to the scalar `Model` that
// `#[sea_orm::model]` re-emits and produces a working CRUD API. The relation
// wrapper fields live on the generated `ModelEx` companion and must not leak
// into the Create/Update/List models or break compilation.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::CRUDResource;
use sea_orm::sea_query::Table;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbErr, Schema};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

pub mod dense_author {
    use crudcrate::{CRUDResource, EntityToModels};
    use sea_orm::entity::prelude::*;
    use uuid::Uuid;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "dense_authors")]
    #[crudcrate(generate_router, api_struct = "DenseAuthor")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable, fulltext)]
        pub name: String,

        #[sea_orm(has_many)]
        pub books: HasMany<super::dense_book::Entity>,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod dense_book {
    use crudcrate::{CRUDResource, EntityToModels};
    use sea_orm::entity::prelude::*;
    use uuid::Uuid;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "dense_books")]
    #[crudcrate(generate_router, api_struct = "DenseBook")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub title: String,

        #[crudcrate(filterable)]
        pub author_id: Uuid,

        #[sea_orm(belongs_to, from = "author_id", to = "id")]
        pub author: BelongsTo<super::dense_author::Entity>,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let db = Database::connect(&url).await?;
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    db.execute(&Table::drop().table(dense_book::Entity).if_exists().to_owned())
        .await?;
    db.execute(
        &Table::drop()
            .table(dense_author::Entity)
            .if_exists()
            .to_owned(),
    )
    .await?;
    db.execute(&schema.create_table_from_entity(dense_author::Entity))
        .await?;
    db.execute(&schema.create_table_from_entity(dense_book::Entity))
        .await?;

    Ok(db)
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/authors", dense_author::DenseAuthor::router(db).into())
}

#[tokio::test]
async fn dense_entity_crud_roundtrip() {
    let db = setup_test_db().await.expect("setup failed");

    let create = app(&db)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/authors")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "name": "Ada" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);

    let body = to_bytes(create.into_body(), usize::MAX).await.unwrap();
    let created: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(created["name"], "Ada");
    let id = Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
    assert!(
        created.get("books").is_none(),
        "relation wrapper field must not appear in the API response"
    );

    let get = app(&db)
        .oneshot(
            Request::builder()
                .uri(format!("/authors/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::OK);

    let list = app(&db)
        .oneshot(Request::builder().uri("/authors").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), usize::MAX).await.unwrap();
    let listed: Vec<Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(listed.len(), 1);
}

#[tokio::test]
async fn dense_entity_create_model_omits_relation_fields() {
    let db = setup_test_db().await.expect("setup failed");

    let created = dense_author::DenseAuthor::create(
        &db,
        dense_author::DenseAuthorCreate {
            name: "Grace".to_string(),
        },
    )
    .await
    .expect("create failed");
    assert_eq!(created.name, "Grace");
}
