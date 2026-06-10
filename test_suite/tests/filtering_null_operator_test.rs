//! End-to-end coverage for `null`-valued filters on a nullable column, driven
//! through the generated HTTP router with `?filter={...}`.
//!
//! Exercises the `serde_json::Value::Null` arm of `apply_filters`
//! (`crudcrate/src/filtering/conditions.rs`):
//! - `{"col":null}`      -> `col IS NULL`
//! - `{"col_neq":null}`  -> `col IS NOT NULL`
//!
//! The `_neq null` -> `IS NOT NULL` mapping is what lets a list view filter to
//! "rows where an optional FK is set" (e.g. paired data streams).

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use tower::ServiceExt;
use uuid::Uuid;

pub mod nullable_record {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "nullable_records")]
    #[crudcrate(generate_router, api_struct = "NullableRecord")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        // The column under test: nullable + filterable.
        #[crudcrate(filterable)]
        pub score: Option<i32>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let db = Database::connect(&url).await?;
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    db.execute(
        backend.build(
            &Table::drop()
                .table(nullable_record::Entity)
                .if_exists()
                .to_owned(),
        ),
    )
    .await?;
    db.execute(backend.build(&schema.create_table_from_entity(nullable_record::Entity)))
        .await?;
    Ok(db)
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest(
        "/records",
        nullable_record::NullableRecord::router(db).into(),
    )
}

/// Two rows have a score, two are null.
///
/// | name  | score |
/// |-------|-------|
/// | alpha | 10    |
/// | beta  | NULL  |
/// | gamma | 30    |
/// | delta | NULL  |
async fn seed(db: &DatabaseConnection) {
    let rows = [
        ("alpha", Some(10)),
        ("beta", None),
        ("gamma", Some(30)),
        ("delta", None),
    ];
    for (name, score) in rows {
        let create = nullable_record::NullableRecordCreate {
            name: name.to_string(),
            score,
        };
        nullable_record::NullableRecord::create(db, create)
            .await
            .expect("seed row");
    }
}

fn filter_uri(filter_json: &str) -> String {
    format!(
        "/records?page=1&per_page=1000&filter={}",
        percent_encoding::utf8_percent_encode(filter_json, percent_encoding::NON_ALPHANUMERIC)
    )
}

async fn get_filtered(
    db: &DatabaseConnection,
    filter_json: &str,
) -> (StatusCode, serde_json::Value) {
    let resp = app(db)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(filter_uri(filter_json))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

fn names(list: &serde_json::Value) -> Vec<String> {
    let mut out: Vec<String> = list
        .as_array()
        .expect("list endpoint returns a JSON array")
        .iter()
        .map(|r| r["name"].as_str().expect("name field").to_string())
        .collect();
    out.sort();
    out
}

#[tokio::test]
async fn null_filter_selects_rows_where_column_is_null() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let (status, body) = get_filtered(&db, r#"{"score":null}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), vec!["beta".to_string(), "delta".to_string()]);
}

#[tokio::test]
async fn neq_null_filter_selects_rows_where_column_is_set() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // `_neq null` must mean IS NOT NULL — the inverse of `{"score":null}`.
    let (status, body) = get_filtered(&db, r#"{"score_neq":null}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names(&body), vec!["alpha".to_string(), "gamma".to_string()]);
}
