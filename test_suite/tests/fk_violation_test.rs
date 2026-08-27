//! A foreign-key violation surfaces as HTTP 409 Conflict, not 500, on every
//! backend.
//!
//! `create_table_from_entity` emits the FK constraint from the child's
//! `belongs_to` relation. Postgres and `MySQL` enforce it natively; `SQLite` only
//! enforces foreign keys when `PRAGMA foreign_keys = ON`, which this test enables
//! so the behaviour is uniform across all three engines. Posting a child whose
//! `parent_id` references a non-existent parent must return 409 (the documented
//! constraint-violation response), while a child with a valid parent returns 201.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use serde_json::{Value, json};
use tower::ServiceExt;

pub mod fkv_parent {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "fkv_parents")]
    #[crudcrate(generate_router, api_struct = "FkvParent", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key)]
        #[crudcrate(primary_key, exclude(create, update))]
        pub id: i32,

        #[crudcrate(filterable)]
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod fkv_child {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "fkv_children")]
    #[crudcrate(generate_router, api_struct = "FkvChild", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key)]
        #[crudcrate(primary_key, exclude(create, update))]
        pub id: i32,

        #[crudcrate(filterable)]
        pub parent_id: i32,

        #[crudcrate(filterable)]
        pub label: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::fkv_parent::Entity",
            from = "Column::ParentId",
            to = "super::fkv_parent::Column::Id"
        )]
        Parent,
    }

    impl Related<super::fkv_parent::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Parent.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let db = Database::connect(&url).await?;

    // SQLite does not enforce foreign keys unless explicitly enabled; Postgres and
    // MySQL enforce them natively. Enable it so all three reject a dangling FK.
    if url.starts_with("sqlite") {
        db.execute_unprepared("PRAGMA foreign_keys = ON").await?;
    }

    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    // Drop children before parents (the FK depends on the parent table).
    db.execute(
        &Table::drop()
            .table(fkv_child::Entity)
            .if_exists()
            .to_owned(),
    )
    .await?;
    db.execute(
        &Table::drop()
            .table(fkv_parent::Entity)
            .if_exists()
            .to_owned(),
    )
    .await?;
    db.execute(&schema.create_table_from_entity(fkv_parent::Entity))
        .await?;
    db.execute(&schema.create_table_from_entity(fkv_child::Entity))
        .await?;

    Ok(db)
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new()
        .nest("/parents", fkv_parent::FkvParent::router(db).into())
        .nest("/children", fkv_child::FkvChild::router(db).into())
}

async fn post(app: &axum::Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

#[tokio::test]
async fn child_with_dangling_fk_returns_409() {
    let db = setup_test_db().await.expect("setup db");
    let app = app(&db);

    // No parent with id 9999 exists -> the insert violates the FK constraint.
    let (status, _) = post(
        &app,
        "/children",
        json!({ "parent_id": 9999, "label": "orphan" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "a dangling foreign key must surface as 409, not 500"
    );
}

#[tokio::test]
async fn child_with_valid_fk_returns_201() {
    let db = setup_test_db().await.expect("setup db");
    let app = app(&db);

    let (status, parent) = post(&app, "/parents", json!({ "name": "root" })).await;
    assert_eq!(status, StatusCode::CREATED);
    let parent_id = parent["id"].as_i64().expect("parent id");

    let (status, _) = post(
        &app,
        "/children",
        json!({ "parent_id": parent_id, "label": "ok" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "a child referencing an existing parent is accepted"
    );
}
