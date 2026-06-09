//! PK parity: a `String` primary key behaves exactly like a `Uuid` PK.
//!
//! The integer-PK suite (`integer_pk_test.rs`) proves parity for an
//! auto-increment `i32`; this file proves the same end-to-end CRUD surface for
//! a caller-supplied `String` PK (a slug). Because the resource value type is
//! generic via `crudcrate::PrimaryKeyType<R>`, the `crud_handlers!` macro emits
//! `Path<String>` for the `/{id}` routes, so a slug must round-trip in both the
//! path parameter and the JSON request/response bodies.
//!
//! Unlike auto-increment integer / UUID-on_create PKs, a `String` PK has no
//! generator: the caller supplies it on create. The id is therefore part of the
//! create model (no `exclude(create)`), but is still excluded from update — the
//! primary key is immutable, matching how every other PK model treats it.
//!
//! Each test notes which `integer_pk_test.rs` test it mirrors.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use serde_json::{Value, json};
use tower::ServiceExt;

/// Resource with a caller-supplied `String` primary key (slug "ppstr").
pub mod thing {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "ppstr_things")]
    #[crudcrate(generate_router, api_struct = "Thing", derive_partial_eq)]
    pub struct Model {
        // Caller supplies the slug on create; the DB does not generate it.
        // No on_create, no exclude(create) — but exclude(update): a PK is immutable.
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(update), filterable, sortable)]
        pub id: String,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[crudcrate(filterable)]
        pub color: Option<String>,
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

    // Persistent backends (Postgres/MySQL) keep tables across tests within a binary;
    // drop first so every test starts from a clean schema. On sqlite::memory: each
    // connection is a fresh database, so the drop is a harmless no-op.
    db.execute(backend.build(&Table::drop().table(thing::Entity).if_exists().to_owned()))
        .await?;
    db.execute(backend.build(&schema.create_table_from_entity(thing::Entity)))
        .await?;

    Ok(db)
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/things", thing::Thing::router(db).into())
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

async fn create_thing(app: &axum::Router, id: &str, name: &str, color: Option<&str>) -> Value {
    let body = json!({ "id": id, "name": name, "color": color });
    let req = Request::builder()
        .method("POST")
        .uri("/things")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let (status, value) = send(app, req).await;
    assert_eq!(status, StatusCode::CREATED, "create thing: {value:?}");
    value
}

// Mirrors `test_integer_pk_create`: 201 with the PK echoed in the body.
#[tokio::test]
async fn test_string_pk_create() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    let thing = create_thing(&app, "rust-lang", "Rust", Some("#DEA584")).await;

    assert!(
        thing["id"].is_string(),
        "id should be a string, got {:?}",
        thing["id"]
    );
    // The caller-supplied slug round-trips in the response body.
    assert_eq!(thing["id"], "rust-lang");
    assert_eq!(thing["name"], "Rust");
    assert_eq!(thing["color"], "#DEA584");
}

// Mirrors `test_integer_pk_get_one`: 200 by string path, 404 for a missing id.
#[tokio::test]
async fn test_string_pk_get_one_and_missing_404() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    create_thing(&app, "rust-lang", "Rust", Some("#DEA584")).await;

    // GET /things/{slug} — the slug round-trips through the Path<String> param.
    let req = Request::builder()
        .method("GET")
        .uri("/things/rust-lang")
        .body(Body::empty())
        .unwrap();
    let (status, value) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "get_one: {value:?}");
    assert_eq!(value["id"], "rust-lang");
    assert_eq!(value["name"], "Rust");

    // A non-existent string id returns 404 (no parse error — any string is valid).
    let req = Request::builder()
        .method("GET")
        .uri("/things/does-not-exist")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// Mirrors `test_integer_pk_get_all`: list returns every row with string ids.
#[tokio::test]
async fn test_string_pk_get_all() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    create_thing(&app, "rust-lang", "Rust", Some("#DEA584")).await;
    create_thing(&app, "python", "Python", Some("#3776AB")).await;
    create_thing(&app, "golang", "Go", None).await;

    let req = Request::builder()
        .method("GET")
        .uri("/things")
        .body(Body::empty())
        .unwrap();
    let (status, value) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "get_all: {value:?}");

    let things = value.as_array().expect("list response is an array");
    assert_eq!(things.len(), 3);

    let mut ids: Vec<&str> = things
        .iter()
        .map(|t| t["id"].as_str().expect("id is a string"))
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, vec!["golang", "python", "rust-lang"]);
}

// Mirrors `test_integer_pk_update`: PUT /things/{slug} keeps the PK, mutates fields.
#[tokio::test]
async fn test_string_pk_update() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    create_thing(&app, "rust-lang", "Rust", Some("#DEA584")).await;

    // The update body omits the immutable PK (exclude(update)); the slug comes
    // from the path only.
    let req = Request::builder()
        .method("PUT")
        .uri("/things/rust-lang")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "name": "Rust (lang)", "color": "#000000" }).to_string(),
        ))
        .unwrap();
    let (status, value) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "update: {value:?}");
    assert_eq!(value["id"], "rust-lang");
    assert_eq!(value["name"], "Rust (lang)");
    assert_eq!(value["color"], "#000000");

    // The change is persisted under the same slug.
    let req = Request::builder()
        .method("GET")
        .uri("/things/rust-lang")
        .body(Body::empty())
        .unwrap();
    let (status, value) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["name"], "Rust (lang)");
}

// Mirrors `test_integer_pk_delete`: DELETE by string path, then 404.
#[tokio::test]
async fn test_string_pk_delete() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    create_thing(&app, "rust-lang", "Rust", Some("#DEA584")).await;

    let req = Request::builder()
        .method("DELETE")
        .uri("/things/rust-lang")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // The row is gone — same 404 a UUID model would return after delete.
    let req = Request::builder()
        .method("GET")
        .uri("/things/rust-lang")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// Exercises filtering the list by the string PK column (`?filter={"id":"..."}`).
// The integer suite filters by integer FK in its join test; here the PK itself
// is the string filter target.
#[tokio::test]
async fn test_string_pk_filter_by_id() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    create_thing(&app, "rust-lang", "Rust", None).await;
    create_thing(&app, "python", "Python", None).await;
    create_thing(&app, "golang", "Go", None).await;

    let filter = json!({ "id": "python" }).to_string();
    let uri = format!("/things?filter={}", urlencode(&filter));
    let req = Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let (status, value) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "filter by id: {value:?}");

    let things = value.as_array().expect("filtered list is an array");
    assert_eq!(
        things.len(),
        1,
        "exactly one row matches the slug: {value:?}"
    );
    assert_eq!(things[0]["id"], "python");
    assert_eq!(things[0]["name"], "Python");
}

/// Minimal percent-encoding for the JSON characters that appear in a `filter`
/// query value (`{`, `}`, `"`, `:`, space). Avoids pulling in a URL-encoding dep.
fn urlencode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '{' => "%7B".to_string(),
            '}' => "%7D".to_string(),
            '"' => "%22".to_string(),
            ':' => "%3A".to_string(),
            ' ' => "%20".to_string(),
            other => other.to_string(),
        })
        .collect()
}
