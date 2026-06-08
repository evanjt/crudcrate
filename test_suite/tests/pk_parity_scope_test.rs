//! Scope / security parity for an integer (i32) primary key.
//!
//! Proves that `ScopeCondition` + `exclude(scoped)` behaves IDENTICALLY for an
//! auto-increment `i32` PK as it does for a `Uuid` PK. This mirrors
//! `scope_security_test.rs` (the UUID-based suite) one assertion at a time, but
//! drives every `/things/{id}` route with an INTEGER path parameter.
//!
//! What is asserted, and the UUID test each case mirrors:
//! - scoped get_one returns the row when scope matches (mirrors
//!   `scope_get_one_ok_for_public`)
//! - scoped get_one returns 404 when scope excludes it, with an integer id
//!   (mirrors `scope_get_one_404_for_private`)
//! - scoped list filters private rows out (mirrors
//!   `scope_list_excludes_private_records`)
//! - `is_private` is stripped from scoped list + get_one responses (mirrors
//!   `scope_list_response_omits_is_private` / `scope_get_one_response_omits_is_private`)
//! - admin (unscoped) responses keep `is_private` (mirrors
//!   `admin_response_includes_is_private`)
//! - write verbs under scope -> 403: POST / PUT / DELETE / batch POST /
//!   batch DELETE / batch PATCH (mirrors `scope_*_blocked`)
//! - Content-Range reflects the scoped count (mirrors
//!   `scope_content_range_reflects_scoped_count`)
//! - filter / sort on the scoped-excluded column is ignored (mirrors
//!   `scope_filter_on_excluded_column_ignored` / `scope_sort_on_excluded_column_ignored`)
//! - nonexistent integer id and private id both 404 (mirrors
//!   `scope_nonexistent_and_private_both_404`)
//! - HEAD allowed under scope (mirrors `scope_head_request_allowed`)
//! - flip-to-private then 404 (mirrors `scope_get_one_atomic_single_query`)
//!
//! Real SQLite-in-memory database, no mocks. Path params are integers.

use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use crudcrate::{CRUDResource, EntityToModels, ScopeCondition};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{Condition, Database, DatabaseConnection, DbErr, Schema};
use serde_json::{Value, json};
use tower::ServiceExt;

// Unique slug prefix "ppsc" so nothing collides with other suites.
pub mod thing {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "ppsc_things")]
    #[crudcrate(generate_router, api_struct = "Thing", derive_partial_eq)]
    pub struct Model {
        // Integer PK assigned by the DB — NO on_create, excluded from create/update.
        #[sea_orm(primary_key)]
        #[crudcrate(primary_key, exclude(create, update))]
        pub id: i32,

        #[crudcrate(filterable, sortable, like_filterable)]
        pub name: String,

        // Scope-filtered column: excluded from scoped responses, stripped from
        // scoped filterable/sortable lists, and defaulted to false on create.
        #[crudcrate(filterable, exclude(scoped, create), on_create = false)]
        pub is_private: bool,
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

/// Unscoped (admin) app: plain router, full field visibility, writes allowed.
fn admin_app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/things", thing::Thing::router(db).into())
}

/// Scoped app: every request carries a `ScopeCondition` filtering is_private=false.
/// Mirrors `common::setup_scoped_app` from the UUID suite.
fn scoped_app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest(
        "/things",
        thing::Thing::router(db)
            .split_for_parts()
            .0
            .layer(axum::middleware::from_fn(scope_things)),
    )
}

async fn scope_things(mut req: Request, next: Next) -> Response {
    req.extensions_mut().insert(ScopeCondition::new(
        Condition::all().add(thing::Column::IsPrivate.eq(false)),
    ));
    next.run(req).await
}

fn encode_filter(filter: &Value) -> String {
    percent_encoding::utf8_percent_encode(&filter.to_string(), percent_encoding::NON_ALPHANUMERIC)
        .to_string()
}

async fn send(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value, axum::http::HeaderMap) {
    let b = body
        .map(|v| Body::from(v.to_string()))
        .unwrap_or_else(Body::empty);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(b)
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value, headers)
}

/// Create a public thing via the admin app, returning its integer id.
async fn admin_create(app: &axum::Router, name: &str) -> i64 {
    let (status, value, _) = send(app, "POST", "/things", Some(json!({ "name": name }))).await;
    assert_eq!(status, StatusCode::CREATED, "create thing: {value:?}");
    assert!(
        value["id"].is_i64() || value["id"].is_u64(),
        "id should be an integer, got {:?}",
        value["id"]
    );
    value["id"].as_i64().unwrap()
}

/// Create a thing then flip it private via admin update. Returns its integer id.
async fn admin_create_private(app: &axum::Router, name: &str) -> i64 {
    let id = admin_create(app, name).await;
    let (status, _, _) = send(
        app,
        "PUT",
        &format!("/things/{id}"),
        Some(json!({ "name": name, "is_private": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "flip private");
    id
}

// =============================================================================
// Mirrors scope_get_one_ok_for_public + scope_get_one_404_for_private:
// integer path param round-trips, get_one_scoped honours the condition.
// =============================================================================

#[tokio::test]
async fn scope_get_one_matches_returns_row_with_integer_id() {
    let db = setup_test_db().await.unwrap();
    let admin = admin_app(&db);
    let scoped = scoped_app(&db);

    let id = admin_create(&admin, "Visible").await;

    let (status, body, _) = send(&scoped, "GET", &format!("/things/{id}"), None).await;
    assert_eq!(status, StatusCode::OK, "scoped get_one public: {body:?}");
    // Integer id round-trips through the JSON response body.
    assert_eq!(body["id"], json!(id));
    assert_eq!(body["name"], "Visible");
}

#[tokio::test]
async fn scope_get_one_excluded_returns_404_for_integer_id() {
    let db = setup_test_db().await.unwrap();
    let admin = admin_app(&db);
    let scoped = scoped_app(&db);

    let id = admin_create_private(&admin, "Secret").await;

    // Same 404 a private UUID row would yield — driven by an integer path param,
    // and never a UUID parse error.
    let (status, _, _) = send(&scoped, "GET", &format!("/things/{id}"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// =============================================================================
// Mirrors scope_list_excludes_private_records.
// =============================================================================

#[tokio::test]
async fn scope_list_excludes_private_rows() {
    let db = setup_test_db().await.unwrap();
    let admin = admin_app(&db);
    let scoped = scoped_app(&db);

    admin_create(&admin, "Public Alice").await;
    admin_create_private(&admin, "Private Bob").await;

    let (status, body, _) = send(&scoped, "GET", "/things", None).await;
    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 1, "only the public thing should be visible");
    assert_eq!(items[0]["name"], "Public Alice");
}

// =============================================================================
// Mirrors scope_list_response_omits_is_private + scope_get_one_response_omits_is_private.
// =============================================================================

#[tokio::test]
async fn scope_responses_omit_is_private() {
    let db = setup_test_db().await.unwrap();
    let admin = admin_app(&db);
    let scoped = scoped_app(&db);

    let id = admin_create(&admin, "Test").await;

    let (_, list, _) = send(&scoped, "GET", "/things", None).await;
    let items = list.as_array().unwrap();
    assert!(!items.is_empty());
    assert!(
        items[0].get("is_private").is_none(),
        "is_private must not appear in scoped list response, got keys: {:?}",
        items[0].as_object().unwrap().keys().collect::<Vec<_>>()
    );

    let (_, one, _) = send(&scoped, "GET", &format!("/things/{id}"), None).await;
    assert!(
        one.get("is_private").is_none(),
        "is_private must not appear in scoped get_one response"
    );
}

// =============================================================================
// Mirrors admin_response_includes_is_private.
// =============================================================================

#[tokio::test]
async fn admin_responses_include_is_private() {
    let db = setup_test_db().await.unwrap();
    let admin = admin_app(&db);

    let id = admin_create_private(&admin, "Admin View").await;

    let (_, list, _) = send(&admin, "GET", "/things", None).await;
    let items = list.as_array().unwrap();
    assert!(
        items[0].get("is_private").is_some(),
        "admin list should include is_private"
    );

    let (_, one, _) = send(&admin, "GET", &format!("/things/{id}"), None).await;
    assert!(
        one.get("is_private").is_some(),
        "admin get_one should include is_private"
    );
    assert_eq!(one["is_private"], json!(true));
}

// =============================================================================
// Mirrors scope_create_blocked: POST under scope -> 403.
// =============================================================================

#[tokio::test]
async fn scope_create_blocked() {
    let db = setup_test_db().await.unwrap();
    let scoped = scoped_app(&db);

    let (status, _, _) = send(&scoped, "POST", "/things", Some(json!({ "name": "Hack" }))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// =============================================================================
// Mirrors scope_update_blocked: PUT /things/{int} under scope -> 403.
// =============================================================================

#[tokio::test]
async fn scope_update_blocked_with_integer_id() {
    let db = setup_test_db().await.unwrap();
    let admin = admin_app(&db);
    let scoped = scoped_app(&db);

    let id = admin_create(&admin, "Existing").await;

    let (status, _, _) = send(
        &scoped,
        "PUT",
        &format!("/things/{id}"),
        Some(json!({ "name": "Hacked" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// =============================================================================
// Mirrors scope_delete_blocked: DELETE /things/{int} under scope -> 403.
// =============================================================================

#[tokio::test]
async fn scope_delete_blocked_with_integer_id() {
    let db = setup_test_db().await.unwrap();
    let admin = admin_app(&db);
    let scoped = scoped_app(&db);

    let id = admin_create(&admin, "Existing").await;

    let (status, _, _) = send(&scoped, "DELETE", &format!("/things/{id}"), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // And the row is untouched: still visible via the admin app.
    let (status, _, _) = send(&admin, "GET", &format!("/things/{id}"), None).await;
    assert_eq!(status, StatusCode::OK, "delete must have been blocked");
}

// =============================================================================
// Mirrors scope_batch_create_blocked / scope_batch_delete_blocked /
// scope_batch_update_blocked, but with integer ids in the batch payloads.
// =============================================================================

#[tokio::test]
async fn scope_batch_create_blocked() {
    let db = setup_test_db().await.unwrap();
    let scoped = scoped_app(&db);

    let (status, _, _) = send(
        &scoped,
        "POST",
        "/things/batch",
        Some(json!([{ "name": "A" }])),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn scope_batch_delete_blocked_with_integer_ids() {
    let db = setup_test_db().await.unwrap();
    let admin = admin_app(&db);
    let scoped = scoped_app(&db);

    let id = admin_create(&admin, "Existing").await;

    let (status, _, _) = send(&scoped, "DELETE", "/things/batch", Some(json!([id]))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn scope_batch_update_blocked_with_integer_ids() {
    let db = setup_test_db().await.unwrap();
    let admin = admin_app(&db);
    let scoped = scoped_app(&db);

    let id = admin_create(&admin, "Existing").await;

    let (status, _, _) = send(
        &scoped,
        "PATCH",
        "/things/batch",
        Some(json!([{ "id": id, "name": "Hacked" }])),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// =============================================================================
// Mirrors scope_content_range_reflects_scoped_count.
// =============================================================================

#[tokio::test]
async fn scope_content_range_reflects_scoped_count() {
    let db = setup_test_db().await.unwrap();
    let admin = admin_app(&db);
    let scoped = scoped_app(&db);

    for i in 0..2 {
        admin_create(&admin, &format!("Pub{i}")).await;
    }
    for i in 0..3 {
        admin_create_private(&admin, &format!("Priv{i}")).await;
    }

    let (_, _, admin_headers) = send(&admin, "GET", "/things", None).await;
    let admin_range = admin_headers
        .get("content-range")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        admin_range.contains("/5"),
        "admin Content-Range should show total 5, got: {admin_range}"
    );

    let (_, _, scoped_headers) = send(&scoped, "GET", "/things", None).await;
    let scoped_range = scoped_headers
        .get("content-range")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        scoped_range.contains("/2"),
        "scoped Content-Range should show total 2, got: {scoped_range}"
    );
}

// =============================================================================
// Mirrors scope_filter_on_excluded_column_ignored.
// =============================================================================

#[tokio::test]
async fn scope_filter_on_excluded_column_ignored() {
    let db = setup_test_db().await.unwrap();
    let admin = admin_app(&db);
    let scoped = scoped_app(&db);

    admin_create(&admin, "Public").await;
    admin_create_private(&admin, "Private").await;

    let filter = encode_filter(&json!({ "is_private": true }));
    let (status, body, _) = send(&scoped, "GET", &format!("/things?filter={filter}"), None).await;
    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().unwrap();
    assert_eq!(
        items.len(),
        1,
        "filtering on is_private should be ignored in scoped context, only public shown"
    );
    assert_eq!(items[0]["name"], "Public");
}

// =============================================================================
// Mirrors scope_sort_on_excluded_column_ignored.
// =============================================================================

#[tokio::test]
async fn scope_sort_on_excluded_column_ignored() {
    let db = setup_test_db().await.unwrap();
    let admin = admin_app(&db);
    let scoped = scoped_app(&db);

    admin_create(&admin, "Zulu").await;
    admin_create(&admin, "Alpha").await;

    let sort = percent_encoding::utf8_percent_encode(
        r#"["is_private","DESC"]"#,
        percent_encoding::NON_ALPHANUMERIC,
    )
    .to_string();
    let (status, body, _) = send(&scoped, "GET", &format!("/things?sort={sort}"), None).await;
    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 2, "both public things should be returned");
    assert!(
        items[0].get("is_private").is_none(),
        "is_private must not appear even when used as a sort column"
    );
}

// =============================================================================
// Mirrors scope_nonexistent_and_private_both_404: both a missing integer id
// and a private integer id return the same 404 (no existence side channel).
// =============================================================================

#[tokio::test]
async fn scope_nonexistent_and_private_both_404() {
    let db = setup_test_db().await.unwrap();
    let admin = admin_app(&db);
    let scoped = scoped_app(&db);

    let private_id = admin_create_private(&admin, "Hidden").await;
    let fake_id: i64 = 999_999;

    let (s1, _, _) = send(&scoped, "GET", &format!("/things/{private_id}"), None).await;
    let (s2, _, _) = send(&scoped, "GET", &format!("/things/{fake_id}"), None).await;
    assert_eq!(s1, StatusCode::NOT_FOUND);
    assert_eq!(s2, StatusCode::NOT_FOUND);
}

// =============================================================================
// Mirrors scope_head_request_allowed.
// =============================================================================

#[tokio::test]
async fn scope_head_request_allowed() {
    let db = setup_test_db().await.unwrap();
    let admin = admin_app(&db);
    let scoped = scoped_app(&db);

    admin_create(&admin, "HeadTest").await;

    let resp = scoped
        .clone()
        .oneshot(
            Request::builder()
                .method("HEAD")
                .uri("/things")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// =============================================================================
// Mirrors scope_get_one_atomic_single_query: visible, flip private, then 404 —
// the scope condition is part of the fetch, keyed by the integer id.
// =============================================================================

#[tokio::test]
async fn scope_get_one_atomic_after_flip_to_private() {
    let db = setup_test_db().await.unwrap();
    let admin = admin_app(&db);
    let scoped = scoped_app(&db);

    let id = admin_create(&admin, "WillGoPrivate").await;

    let (s, _, _) = send(&scoped, "GET", &format!("/things/{id}"), None).await;
    assert_eq!(s, StatusCode::OK, "public thing should be visible");

    let (s, _, _) = send(
        &admin,
        "PUT",
        &format!("/things/{id}"),
        Some(json!({ "name": "WillGoPrivate", "is_private": true })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    let (s, body, _) = send(&scoped, "GET", &format!("/things/{id}"), None).await;
    assert_eq!(
        s,
        StatusCode::NOT_FOUND,
        "after flipping to private, scoped get_one must return 404"
    );
    assert!(
        body.get("name").is_none(),
        "404 response must not leak the row, got: {body}"
    );
}
