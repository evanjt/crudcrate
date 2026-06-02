//! Single-item CRUD parity for an integer (`i32`) primary key.
//!
//! Proves that an entity whose PK is an auto-increment `i32` exhibits exactly
//! the same HTTP surface a `uuid::Uuid`-keyed entity would: create returns 201
//! with a DB-assigned id, get_one returns 200, list returns 200 with ascending
//! ids, update returns 200 and persists, delete returns 204, and reads against
//! a deleted or never-existing id return 404. Throughout we assert the id is a
//! JSON number, never a string.
//!
//! The slug `ppc` keeps the table/api_struct names unique within the suite.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use serde_json::{Value, json};
use tower::ServiceExt;

pub mod thing {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "ppc_things")]
    #[crudcrate(generate_router, api_struct = "Thing", derive_partial_eq)]
    pub struct Model {
        // Auto-increment integer PK: no on_create, the DB assigns the value.
        #[sea_orm(primary_key)]
        #[crudcrate(primary_key, exclude(create, update))]
        pub id: i32,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[crudcrate(filterable)]
        pub note: Option<String>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let db = Database::connect("sqlite::memory:").await?;
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

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

async fn create_thing(app: &axum::Router, name: &str, note: Option<&str>) -> Value {
    let body = json!({ "name": name, "note": note });
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

/// Asserts the `id` field is a JSON number, never a string. This is the core
/// parity claim: a UUID model serialises its id as a string, an integer model
/// must serialise it as a number.
fn assert_id_is_json_number(value: &Value) {
    assert!(
        value["id"].is_i64() || value["id"].is_u64(),
        "id must be a JSON number, got {:?}",
        value["id"]
    );
    assert!(
        !value["id"].is_string(),
        "id must not be a JSON string, got {:?}",
        value["id"]
    );
}

// Mirrors the create half of the UUID single-item CRUD flow: 201 with a
// server-assigned id. With an integer PK the first row gets id 1.
#[tokio::test]
async fn test_pk_parity_create_returns_201_with_integer_id() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    let thing = create_thing(&app, "alpha", Some("first")).await;

    assert_id_is_json_number(&thing);
    assert_eq!(thing["id"], json!(1));
    assert_eq!(thing["name"], "alpha");
    assert_eq!(thing["note"], "first");

    // Consecutive creates get ascending integer ids 2, 3, ... just as a UUID
    // model would mint distinct ids per row.
    let second = create_thing(&app, "beta", None).await;
    let third = create_thing(&app, "gamma", None).await;
    assert_eq!(second["id"], json!(2));
    assert_eq!(third["id"], json!(3));
    assert_id_is_json_number(&second);
    assert_id_is_json_number(&third);
}

// Mirrors get_one for the UUID model: 200 for an existing id, with the integer
// id round-tripping through the `/things/{int}` path param into the response.
#[tokio::test]
async fn test_pk_parity_get_one_returns_200() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    create_thing(&app, "alpha", Some("first")).await;

    let req = Request::builder()
        .method("GET")
        .uri("/things/1")
        .body(Body::empty())
        .unwrap();
    let (status, value) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "get_one: {value:?}");
    assert_id_is_json_number(&value);
    assert_eq!(value["id"], json!(1));
    assert_eq!(value["name"], "alpha");
    assert_eq!(value["note"], "first");
}

// Mirrors get_all for the UUID model: 200 with a JSON array. Every id is a
// number and the ids come back in ascending order.
#[tokio::test]
async fn test_pk_parity_get_all_returns_200_ascending_integer_ids() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    create_thing(&app, "alpha", None).await;
    create_thing(&app, "beta", None).await;
    create_thing(&app, "gamma", None).await;

    let req = Request::builder()
        .method("GET")
        .uri("/things")
        .body(Body::empty())
        .unwrap();
    let (status, value) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "get_all: {value:?}");

    let things = value.as_array().expect("list response is an array");
    assert_eq!(things.len(), 3);

    for t in things {
        assert_id_is_json_number(t);
    }

    let ids: Vec<i64> = things.iter().map(|t| t["id"].as_i64().unwrap()).collect();
    assert_eq!(ids, vec![1, 2, 3], "ids should already be ascending");
}

// Mirrors update for the UUID model: PUT /{id} returns 200 and the change is
// readable on a subsequent get_one.
#[tokio::test]
async fn test_pk_parity_update_returns_200_and_persists() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    create_thing(&app, "alpha", Some("first")).await;

    let req = Request::builder()
        .method("PUT")
        .uri("/things/1")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "name": "alpha-renamed", "note": "edited" }).to_string(),
        ))
        .unwrap();
    let (status, value) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "update: {value:?}");
    assert_id_is_json_number(&value);
    assert_eq!(value["id"], json!(1));
    assert_eq!(value["name"], "alpha-renamed");
    assert_eq!(value["note"], "edited");

    // The change persists across a fresh read.
    let req = Request::builder()
        .method("GET")
        .uri("/things/1")
        .body(Body::empty())
        .unwrap();
    let (status, value) = send(&app, req).await;
    assert_eq!(status, StatusCode::OK, "get after update: {value:?}");
    assert_eq!(value["name"], "alpha-renamed");
    assert_eq!(value["note"], "edited");
}

// Mirrors delete for the UUID model: DELETE /{id} returns 204 No Content and a
// follow-up get_one on the deleted id returns 404.
#[tokio::test]
async fn test_pk_parity_delete_returns_204_then_404() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    create_thing(&app, "alpha", None).await;

    let req = Request::builder()
        .method("DELETE")
        .uri("/things/1")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let req = Request::builder()
        .method("GET")
        .uri("/things/1")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "get on deleted id");
}

// Mirrors the not-found path for the UUID model: a get_one against an id that
// was never created returns 404. The integer path param parses cleanly; there
// is no UUID parse error masquerading as a different status.
#[tokio::test]
async fn test_pk_parity_get_nonexistent_returns_404() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    create_thing(&app, "alpha", None).await;

    let req = Request::builder()
        .method("GET")
        .uri("/things/9999")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "get on never-created id");
}
