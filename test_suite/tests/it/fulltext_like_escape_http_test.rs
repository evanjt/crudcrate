//! End-to-end regression for the LIKE ESCAPE handling on the fulltext LIKE-fallback path.
//!
//! The `q` search first tries `build_fulltext_condition`. For an entity with no
//! `fulltext` attribute that returns `None`, so `handle_fulltext_search` falls back
//! to a LIKE search over the filterable columns. That fallback escapes the SQL LIKE
//! wildcards (`%`, `_`) so a literal `%` or `_` in the query does not act as a wildcard.
//!
//! Pre-fix, `GET /things?filter={"q":"a%b"}` matched both `a%b` and `axxb` on `SQLite`:
//! `handle_fulltext_search` escaped wildcards with a backslash but emitted no `ESCAPE`
//! clause, so `SQLite` (whose default LIKE has no escape char) still treated `%`/`_` as
//! wildcards. The fix routes the fallback through `LikeExpr::...escape('!')`, declaring
//! an explicit `ESCAPE '!'`, so a literal `%`/`_` matches only the literal row and never
//! the wildcard sibling.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

pub mod fle_thing {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "fle_things")]
    #[crudcrate(generate_router, api_struct = "FleThing")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        // Filterable + sortable, and crucially NO `fulltext` attribute, so the `q`
        // search exercises the LIKE fallback path rather than a native fulltext index.
        #[crudcrate(filterable, sortable)]
        pub body: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(fle_thing::Entity).await
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/things", fle_thing::FleThing::router(db).into())
}

fn encode_filter(filter: &Value) -> String {
    percent_encoding::utf8_percent_encode(&filter.to_string(), percent_encoding::NON_ALPHANUMERIC)
        .to_string()
}

async fn insert_thing(app: &axum::Router, body: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/things")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::json!({ "body": body }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "POST /things {body:?} should succeed"
    );
}

/// GET /things with a `q` filter, returning the parsed JSON array of results.
async fn search_q(app: &axum::Router, q: &str) -> Vec<Value> {
    let filter = serde_json::json!({ "q": q });
    let uri = format!("/things?filter={}", encode_filter(&filter));
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "GET {uri} should return 200"
    );
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    json.as_array()
        .expect("list endpoint should return a JSON array")
        .clone()
}

fn bodies(rows: &[Value]) -> Vec<String> {
    rows.iter()
        .map(|row| row["body"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn test_literal_percent_is_not_treated_as_wildcard() {
    // Scenario: a `%` in the search term must NOT act as a SQL LIKE wildcard.
    //
    // Before the fix, "a%b" matched BOTH "a%b" and "axxb" on SQLite (the `%` expanded
    // as a wildcard) because the LIKE-fallback escaped with a backslash but emitted no
    // ESCAPE clause. The fix routes the fallback through `LikeExpr::...escape('!')`, so
    // the `%` is escaped AND declared: searching "a%b" now matches only the literal row.
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    insert_thing(&app, "a%b").await;
    insert_thing(&app, "axxb").await;

    let rows = search_q(&app, "a%b").await;
    assert_eq!(
        bodies(&rows),
        vec!["a%b".to_string()],
        "literal '%' must be escaped: only the literal 'a%b' row matches, not the \
         wildcard sibling 'axxb', got: {:?}",
        bodies(&rows)
    );
}

#[tokio::test]
async fn test_literal_underscore_is_not_treated_as_wildcard() {
    // `_` is a single-character LIKE wildcard; the fallback escaping + ESCAPE '!' clause
    // stops it from expanding, so searching "a_b" matches only the literal "a_b" row and
    // not the wildcard sibling "azb".
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    insert_thing(&app, "a_b").await;
    insert_thing(&app, "azb").await;

    let rows = search_q(&app, "a_b").await;
    assert_eq!(
        bodies(&rows),
        vec!["a_b".to_string()],
        "literal '_' must be escaped: only the literal 'a_b' row matches, not the \
         wildcard sibling 'azb', got: {:?}",
        bodies(&rows)
    );
}

#[tokio::test]
async fn test_plain_term_still_matches() {
    // A wildcard-free term must still match its substring as before.
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    insert_thing(&app, "a%b").await;
    insert_thing(&app, "axxb").await;

    let rows = search_q(&app, "axxb").await;
    assert_eq!(
        rows.len(),
        1,
        "plain term 'axxb' must match only the 'axxb' row, got: {:?}",
        bodies(&rows)
    );
    assert_eq!(rows[0]["body"].as_str().unwrap(), "axxb");
}
