//! Non-UUID primary key support (integer i32 PKs) end-to-end.
//!
//! Exercises the full CRUD HTTP surface for an entity whose primary key is an
//! auto-increment `i32` rather than a `uuid::Uuid`: create, `get_one`, `get_all`,
//! update, delete, batch delete, and `get_all` batch join loading keyed by an
//! integer FK. The path parameter must parse as an integer, never a UUID.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use serde_json::{Value, json};
use tower::ServiceExt;

pub mod tag {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "ipk_tags")]
    #[crudcrate(generate_router, api_struct = "Tag", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key)]
        #[crudcrate(primary_key, exclude(create, update))]
        pub id: i32,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[crudcrate(filterable)]
        pub color: Option<String>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// Parent entity (integer PK) with a `has_many` to `ipk_labels`, loaded in
/// `get_all` via `join(all)` and keyed by the integer `book_id` FK on the child.
pub mod book {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "ipk_books")]
    #[crudcrate(generate_router, api_struct = "Book", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key)]
        #[crudcrate(primary_key, exclude(create, update))]
        pub id: i32,

        #[crudcrate(filterable, sortable)]
        pub title: String,

        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, exclude(create, update), join(all, depth = 1))]
        pub labels: Vec<super::label::Label>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::label::Entity")]
        Labels,
    }

    impl Related<super::label::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Labels.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod label {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "ipk_labels")]
    #[crudcrate(generate_router, api_struct = "Label", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key)]
        #[crudcrate(primary_key, exclude(create, update))]
        pub id: i32,

        #[crudcrate(filterable)]
        pub book_id: i32,

        #[crudcrate(filterable, sortable)]
        pub text: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::book::Entity",
            from = "Column::BookId",
            to = "super::book::Column::Id"
        )]
        Book,
    }

    impl Related<super::book::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Book.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let db = Database::connect(&url).await?;
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    // Persistent backends (Postgres/MySQL) keep tables across tests within a binary;
    // drop first so every test starts from a clean schema and empty data. On
    // sqlite::memory: each connection is a fresh database, so the drops are no-ops.
    // create_table_from_entity emits FK constraints from belongs_to relations, so
    // drop children before parents (label references book; tag is independent).
    for stmt in [
        Table::drop().table(label::Entity).if_exists().to_owned(),
        Table::drop().table(book::Entity).if_exists().to_owned(),
        Table::drop().table(tag::Entity).if_exists().to_owned(),
    ] {
        db.execute(&stmt).await?;
    }

    db.execute(&schema.create_table_from_entity(tag::Entity))
        .await?;
    db.execute(&schema.create_table_from_entity(book::Entity))
        .await?;
    db.execute(&schema.create_table_from_entity(label::Entity))
        .await?;

    Ok(db)
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new()
        .nest("/tags", tag::Tag::router(db).into())
        .nest("/books", book::Book::router(db).into())
        .nest("/labels", label::Label::router(db).into())
}

async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

async fn create_tag(app: &axum::Router, name: &str, color: Option<&str>) -> Value {
    let body = json!({ "name": name, "color": color });
    let req = Request::builder()
        .method("POST")
        .uri("/tags")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let (status, value) = send(app, req).await;
    assert_eq!(status, StatusCode::CREATED, "create tag: {value:?}");
    value
}

#[tokio::test]
async fn test_integer_pk_create() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    let tag = create_tag(&app, "rust", Some("#DEA584")).await;

    assert!(
        tag["id"].is_i64() || tag["id"].is_u64(),
        "id should be an integer, got {:?}",
        tag["id"]
    );
    assert_eq!(tag["id"], 1);
    assert_eq!(tag["name"], "rust");
    assert_eq!(tag["color"], "#DEA584");
}

#[tokio::test]
async fn test_integer_pk_get_one() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    create_tag(&app, "rust", Some("#DEA584")).await;

    let req = Request::builder()
        .method("GET")
        .uri("/tags/1")
        .body(Body::empty())
        .unwrap();
    let (status, value) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "get_one: {value:?}");
    assert_eq!(value["id"], 1);
    assert_eq!(value["name"], "rust");

    // A non-existent integer id returns 404, not a UUID parse error.
    let req = Request::builder()
        .method("GET")
        .uri("/tags/999")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_integer_pk_get_all() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    create_tag(&app, "rust", Some("#DEA584")).await;
    create_tag(&app, "python", Some("#3776AB")).await;
    create_tag(&app, "go", None).await;

    let req = Request::builder()
        .method("GET")
        .uri("/tags")
        .body(Body::empty())
        .unwrap();
    let (status, value) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "get_all: {value:?}");

    let tags = value.as_array().expect("list response is an array");
    assert_eq!(tags.len(), 3);
    let mut ids: Vec<i64> = tags.iter().map(|t| t["id"].as_i64().unwrap()).collect();
    ids.sort_unstable();
    assert_eq!(ids, vec![1, 2, 3]);
}

#[tokio::test]
async fn test_integer_pk_update() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    create_tag(&app, "rust", Some("#DEA584")).await;

    let req = Request::builder()
        .method("PUT")
        .uri("/tags/1")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "name": "rust-lang", "color": "#000000" }).to_string(),
        ))
        .unwrap();
    let (status, value) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "update: {value:?}");
    assert_eq!(value["id"], 1);
    assert_eq!(value["name"], "rust-lang");
    assert_eq!(value["color"], "#000000");
}

#[tokio::test]
async fn test_integer_pk_delete() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    create_tag(&app, "rust", Some("#DEA584")).await;

    let req = Request::builder()
        .method("DELETE")
        .uri("/tags/1")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let req = Request::builder()
        .method("GET")
        .uri("/tags/1")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_integer_pk_batch_delete() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    create_tag(&app, "a", None).await;
    create_tag(&app, "b", None).await;
    create_tag(&app, "c", None).await;

    let req = Request::builder()
        .method("DELETE")
        .uri("/tags/batch")
        .header("content-type", "application/json")
        .body(Body::from(json!([1, 2, 3]).to_string()))
        .unwrap();
    let (status, value) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "batch delete: {value:?}");
    // Secure default profile reports a count rather than the raw integer ids.
    assert_eq!(value["deleted"], 3);

    let req = Request::builder()
        .method("GET")
        .uri("/tags")
        .body(Body::empty())
        .unwrap();
    let (_, value) = send(&app, req).await;
    assert_eq!(value.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_integer_pk_batch_loading_joins() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    // Two books, each with labels keyed by an integer book_id FK.
    let book_one = book::Book::create(
        &db,
        book::BookCreate {
            title: "The Rust Programming Language".to_string(),
        },
    )
    .await
    .expect("create book one");
    let book_two = book::Book::create(
        &db,
        book::BookCreate {
            title: "Programming Rust".to_string(),
        },
    )
    .await
    .expect("create book two");

    for text in ["systems", "memory-safety"] {
        label::Label::create(
            &db,
            label::LabelCreate {
                book_id: book_one.id,
                text: text.to_string(),
            },
        )
        .await
        .expect("create label for book one");
    }
    label::Label::create(
        &db,
        label::LabelCreate {
            book_id: book_two.id,
            text: "ownership".to_string(),
        },
    )
    .await
    .expect("create label for book two");

    let req = Request::builder()
        .method("GET")
        .uri("/books")
        .body(Body::empty())
        .unwrap();
    let (status, value) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "books list: {value:?}");

    let books = value.as_array().expect("books list is an array");
    assert_eq!(books.len(), 2);

    let found_one = books
        .iter()
        .find(|b| b["id"] == json!(book_one.id))
        .expect("book one present");
    let found_two = books
        .iter()
        .find(|b| b["id"] == json!(book_two.id))
        .expect("book two present");

    assert_eq!(
        found_one["labels"].as_array().map(Vec::len),
        Some(2),
        "book one should batch-load 2 labels via integer FK join: {found_one:?}"
    );
    assert_eq!(
        found_two["labels"].as_array().map(Vec::len),
        Some(1),
        "book two should batch-load 1 label via integer FK join: {found_two:?}"
    );
}
