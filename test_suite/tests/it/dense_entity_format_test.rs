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
use sea_orm::{ConnectionTrait, DatabaseConnection, DbErr, Schema};
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

        // crudcrate join field alongside the dense relation: this is how related
        // entities are exposed through the API.
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr, join(one, all, depth = 1))]
        pub book_list: Vec<super::dense_book::DenseBook>,
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
    #[crudcrate(generate_router, api_struct = "DenseBook", derive_partial_eq)]
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

// Dense entity with no relation fields: the ModelEx companion has no relation
// wrapper fields either, so the derives skip it purely by its generated name.
// The sea-orm macro generates async trait methods that do not await.
#[allow(clippy::unused_async_trait_impl)]
pub mod dense_plain {
    use crudcrate::{CRUDResource, EntityToModels};
    use sea_orm::entity::prelude::*;
    use uuid::Uuid;

    #[sea_orm::model]
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "dense_plain_items")]
    #[crudcrate(generate_router, api_struct = "DensePlain")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub label: String,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

// Classic compact entity opted into the transitional `#[sea_orm::compact_model]`
// macro, which generates the same ModelEx companion.
// The sea-orm macro generates async trait methods that do not await.
#[allow(clippy::unused_async_trait_impl)]
pub mod compact_item {
    use crudcrate::{CRUDResource, EntityToModels};
    use sea_orm::entity::prelude::*;
    use uuid::Uuid;

    #[sea_orm::compact_model]
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "compact_items")]
    #[crudcrate(generate_router, api_struct = "CompactItem")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub label: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(dense_author::Entity, dense_book::Entity).await
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
        .oneshot(
            Request::builder()
                .uri("/authors")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), usize::MAX).await.unwrap();
    let listed: Vec<Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(listed.len(), 1);
}

// Scenario: a crudcrate `join(...)` field coexists with dense relation fields
// on the same entity.
// Expected behaviour: the join field loads children exactly as it does on
// classic entities, resolving the FK through the Related impl that
// `#[sea_orm::model]` generates from the inline `belongs_to`.
#[tokio::test]
async fn dense_entity_crudcrate_join_loads_children() {
    let db = setup_test_db().await.expect("setup failed");

    let author = dense_author::DenseAuthor::create(
        &db,
        dense_author::DenseAuthorCreate {
            name: "Mary".to_string(),
        },
    )
    .await
    .expect("author create failed");

    for title in ["Frankenstein", "Mathilda"] {
        dense_book::DenseBook::create(
            &db,
            dense_book::DenseBookCreate {
                title: title.to_string(),
                author_id: author.id,
            },
        )
        .await
        .expect("book create failed");
    }

    let response = app(&db)
        .oneshot(
            Request::builder()
                .uri(format!("/authors/{}", author.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let fetched: Value = serde_json::from_slice(&body).unwrap();
    let titles: Vec<&str> = fetched["book_list"]
        .as_array()
        .expect("book_list must be a populated join field")
        .iter()
        .map(|b| b["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles.len(), 2);
    assert!(titles.contains(&"Frankenstein") && titles.contains(&"Mathilda"));
}

#[tokio::test]
async fn zero_relation_dense_entity_generates_working_api() {
    let db = setup_test_db().await.expect("setup failed");
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);
    db.execute(
        &Table::drop()
            .table(dense_plain::Entity)
            .if_exists()
            .to_owned(),
    )
    .await
    .unwrap();
    db.execute(&schema.create_table_from_entity(dense_plain::Entity))
        .await
        .unwrap();

    let created = dense_plain::DensePlain::create(
        &db,
        dense_plain::DensePlainCreate {
            label: "plain".to_string(),
        },
    )
    .await
    .expect("create failed");
    let fetched = dense_plain::DensePlain::get_one(&db, created.id)
        .await
        .expect("get_one failed");
    assert_eq!(fetched.label, "plain");
}

#[tokio::test]
async fn compact_model_entity_generates_working_api() {
    let db = setup_test_db().await.expect("setup failed");
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);
    db.execute(
        &Table::drop()
            .table(compact_item::Entity)
            .if_exists()
            .to_owned(),
    )
    .await
    .unwrap();
    db.execute(&schema.create_table_from_entity(compact_item::Entity))
        .await
        .unwrap();

    let created = compact_item::CompactItem::create(
        &db,
        compact_item::CompactItemCreate {
            label: "compact".to_string(),
        },
    )
    .await
    .expect("create failed");
    let fetched = compact_item::CompactItem::get_one(&db, created.id)
        .await
        .expect("get_one failed");
    assert_eq!(fetched.label, "compact");
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
