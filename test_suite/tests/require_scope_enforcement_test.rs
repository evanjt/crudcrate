//! Covers the `REQUIRE_SCOPE` enforcement branch in the generated `get_one`/`get_all`
//! handlers.
//!
//! When a resource is declared with `#[crudcrate(require_scope)]`, the generated
//! handlers must refuse to serve requests that arrive without a `ScopeCondition`
//! extension (ie. the scope middleware is missing or misconfigured). The branch
//! under test is `if REQUIRE_SCOPE && scope.is_none() { return Err(internal(..)) }`,
//! which maps to HTTP 500.
//!
//! Writes are deliberately outside that branch: they are governed by scope presence
//! alone (403 when a `ScopeCondition` is present, allowed when absent), so the scope
//! can be mounted on safe methods only. Section 1b pins that contract.
//!
//! Self-contained: defines its own entities, its own `setup_test_db`, and uses no
//! shared `mod common`.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{Condition, Database, DatabaseConnection, DbErr, Schema};
use tower::ServiceExt;
use uuid::Uuid;

/// Entity that REQUIRES scope middleware to be present.
pub mod rse_item {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "rse_items")]
    #[crudcrate(generate_router, api_struct = "RseItem", require_scope)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// Control entity WITHOUT `require_scope`: must serve fine without any scope layer.
pub mod rse_other {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "rse_others")]
    #[crudcrate(generate_router, api_struct = "RseOther")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,
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
    // drop first so every test starts from a clean schema and empty data. On
    // sqlite::memory: each connection is a fresh database, so the drops are no-ops.
    // create_table_from_entity emits no FK constraints, so drop order is irrelevant.
    for stmt in [
        Table::drop().table(rse_item::Entity).if_exists().to_owned(),
        Table::drop()
            .table(rse_other::Entity)
            .if_exists()
            .to_owned(),
    ] {
        db.execute(&stmt).await?;
    }

    db.execute(&schema.create_table_from_entity(rse_item::Entity))
        .await?;
    db.execute(&schema.create_table_from_entity(rse_other::Entity))
        .await?;

    Ok(db)
}

/// Router for the require_scope entity, mounted WITHOUT any scope layer.
fn items_app_unscoped(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/items", rse_item::RseItem::router(db).into())
}

/// Router for the require_scope entity, mounted WITH a ScopeCondition that matches
/// everything (`Condition::all()` with no predicates is an always-true AND).
fn items_app_scoped(db: &DatabaseConnection) -> axum::Router {
    items_app_unscoped(db).layer(axum::Extension(crudcrate::ScopeCondition {
        condition: Condition::all(),
    }))
}

/// Router for the control entity (no require_scope), mounted WITHOUT any scope layer.
fn others_app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/others", rse_other::RseOther::router(db).into())
}

async fn get(app: axum::Router, uri: &str) -> StatusCode {
    app.oneshot(
        Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
    .status()
}

// =============================================================================
// 1. Without scope middleware: get_all + get_one on a require_scope resource 500.
// =============================================================================

#[tokio::test]
async fn require_scope_get_all_without_scope_returns_500() {
    let db = setup_test_db().await.unwrap();

    let status = get(items_app_unscoped(&db), "/items").await;
    assert_eq!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "GET /items on a require_scope resource without a ScopeCondition must return 500"
    );
}

#[tokio::test]
async fn require_scope_get_one_without_scope_returns_500() {
    let db = setup_test_db().await.unwrap();

    let some_uuid = Uuid::new_v4();
    let status = get(items_app_unscoped(&db), &format!("/items/{some_uuid}")).await;
    assert_eq!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "GET /items/{{id}} on a require_scope resource without a ScopeCondition must return 500"
    );
}

// =============================================================================
// 1b. Writes are governed by scope presence alone: `require_scope` gates reads.
//     Without the extension a write proceeds; with it, every write is 403.
// =============================================================================

async fn send(app: axum::Router, method: &str, uri: &str, body: &str) -> StatusCode {
    app.oneshot(
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
    .unwrap()
    .status()
}

async fn seed_item(db: &DatabaseConnection, name: &str) -> Uuid {
    rse_item::RseItem::create(
        db,
        rse_item::RseItemCreate {
            name: name.to_string(),
        },
    )
    .await
    .expect("direct trait create succeeds")
    .id
}

#[tokio::test]
async fn require_scope_create_one_without_scope_is_allowed() {
    let db = setup_test_db().await.unwrap();

    let status = send(items_app_unscoped(&db), "POST", "/items", r#"{"name":"x"}"#).await;
    assert_eq!(status, StatusCode::CREATED);

    let count = rse_item::Entity::find().all(&db).await.unwrap().len();
    assert_eq!(count, 1, "the accepted create must have written the row");
}

#[tokio::test]
async fn require_scope_create_many_without_scope_is_allowed() {
    let db = setup_test_db().await.unwrap();

    let status = send(
        items_app_unscoped(&db),
        "POST",
        "/items/batch",
        r#"[{"name":"x"},{"name":"y"}]"#,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn require_scope_update_one_without_scope_is_allowed() {
    let db = setup_test_db().await.unwrap();
    let id = seed_item(&db, "before").await;

    let status = send(
        items_app_unscoped(&db),
        "PUT",
        &format!("/items/{id}"),
        r#"{"name":"after"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let row = rse_item::Entity::find_by_id(id).one(&db).await.unwrap();
    assert_eq!(row.unwrap().name, "after");
}

#[tokio::test]
async fn require_scope_update_many_without_scope_is_allowed() {
    let db = setup_test_db().await.unwrap();
    let id = seed_item(&db, "before").await;

    let status = send(
        items_app_unscoped(&db),
        "PATCH",
        "/items/batch",
        &format!(r#"[{{"id":"{id}","name":"after"}}]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn require_scope_delete_one_without_scope_is_allowed() {
    let db = setup_test_db().await.unwrap();
    let id = seed_item(&db, "doomed").await;

    let status = send(
        items_app_unscoped(&db),
        "DELETE",
        &format!("/items/{id}"),
        "",
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let count = rse_item::Entity::find().all(&db).await.unwrap().len();
    assert_eq!(count, 0, "the delete must have removed the row");
}

#[tokio::test]
async fn require_scope_delete_many_without_scope_is_allowed() {
    let db = setup_test_db().await.unwrap();
    let id = seed_item(&db, "doomed").await;

    let status = send(
        items_app_unscoped(&db),
        "DELETE",
        "/items/batch",
        &format!(r#"["{id}"]"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn scoped_create_one_returns_403_and_writes_no_row() {
    let db = setup_test_db().await.unwrap();

    let status = send(items_app_scoped(&db), "POST", "/items", r#"{"name":"x"}"#).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let count = rse_item::Entity::find().all(&db).await.unwrap().len();
    assert_eq!(count, 0, "the refused create must not have written a row");
}

#[tokio::test]
async fn scoped_writes_return_403_on_every_verb() {
    let db = setup_test_db().await.unwrap();
    let id = seed_item(&db, "kept").await;

    for (method, uri, body) in [
        ("POST", "/items".to_string(), r#"{"name":"x"}"#.to_string()),
        (
            "POST",
            "/items/batch".to_string(),
            r#"[{"name":"x"}]"#.to_string(),
        ),
        ("PUT", format!("/items/{id}"), r#"{"name":"x"}"#.to_string()),
        (
            "PATCH",
            "/items/batch".to_string(),
            format!(r#"[{{"id":"{id}","name":"x"}}]"#),
        ),
        ("DELETE", format!("/items/{id}"), String::new()),
        ("DELETE", "/items/batch".to_string(), format!(r#"["{id}"]"#)),
    ] {
        let status = send(items_app_scoped(&db), method, &uri, &body).await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "{method} {uri} was not refused"
        );
    }

    let row = rse_item::Entity::find_by_id(id).one(&db).await.unwrap();
    assert_eq!(row.unwrap().name, "kept", "a scoped write reached the row");
}

// =============================================================================
// 2. With a ScopeCondition layer: get_all returns 200 (even with no rows), and
//    once a row exists, get_one resolves it.
// =============================================================================

#[tokio::test]
async fn require_scope_get_all_with_scope_returns_200_empty() {
    let db = setup_test_db().await.unwrap();

    let resp = items_app_scoped(&db)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/items")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json.as_array().map(Vec::len),
        Some(0),
        "Empty require_scope list with scope present should be an empty array"
    );
}

#[tokio::test]
async fn require_scope_get_all_with_scope_returns_created_row() {
    let db = setup_test_db().await.unwrap();

    let created = rse_item::RseItem::create(
        &db,
        rse_item::RseItemCreate {
            name: "scoped-row".to_string(),
        },
    )
    .await
    .expect("direct trait create should succeed");

    let resp = items_app_scoped(&db)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/items")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let items = json.as_array().expect("list response should be an array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "scoped-row");
    assert_eq!(items[0]["id"], created.id.to_string());
}

#[tokio::test]
async fn require_scope_get_one_with_scope_resolves_existing_row() {
    let db = setup_test_db().await.unwrap();

    let created = rse_item::RseItem::create(
        &db,
        rse_item::RseItemCreate {
            name: "fetch-me".to_string(),
        },
    )
    .await
    .expect("direct trait create should succeed");

    let resp = items_app_scoped(&db)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/items/{}", created.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "get_one with scope present should resolve the existing row"
    );

    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["name"], "fetch-me");
}

// =============================================================================
// 3. Control: a sibling entity WITHOUT require_scope serves fine with no scope layer.
// =============================================================================

#[tokio::test]
async fn non_require_scope_get_all_without_scope_returns_200() {
    let db = setup_test_db().await.unwrap();

    let status = get(others_app(&db), "/others").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "A resource WITHOUT require_scope must serve GET /others without any scope layer"
    );
}

#[tokio::test]
async fn non_require_scope_get_one_without_scope_returns_404_not_500() {
    let db = setup_test_db().await.unwrap();

    // No row exists, so this is a 404, crucially NOT a 500. This confirms the
    // require_scope branch is not taken for the control entity.
    let some_uuid = Uuid::new_v4();
    let status = get(others_app(&db), &format!("/others/{some_uuid}")).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "Control resource get_one for a missing row should be 404, not the require_scope 500"
    );
}

// =============================================================================
// 4. Trait-level constant sanity: REQUIRE_SCOPE reflects the attribute.
// =============================================================================

#[tokio::test]
async fn require_scope_constant_is_set_correctly() {
    assert!(
        <rse_item::RseItem as CRUDResource>::REQUIRE_SCOPE,
        "RseItem declares require_scope, so REQUIRE_SCOPE must be true"
    );
    assert!(
        !<rse_other::RseOther as CRUDResource>::REQUIRE_SCOPE,
        "RseOther does not declare require_scope, so REQUIRE_SCOPE must default to false"
    );
}
