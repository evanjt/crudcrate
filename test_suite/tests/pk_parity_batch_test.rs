//! Batch-operation parity for an entity with an auto-increment `i32` primary key.
//!
//! These tests prove that POST/PATCH/DELETE `/batch`, their `?partial=true`
//! variants, and `delete_many` de-duplication all behave identically to a
//! `Uuid`-keyed model, except that ids are JSON integers in request bodies,
//! response bodies, and the `succeeded`/`failed` arrays.
//!
//! UUID counterparts mirrored here all live in
//! `test_suite/tests/partial_success_batch_test.rs`:
//!   - `test_ppb_batch_create_all_valid`        ~ `test_batch_create_all_valid_succeeds`
//!   - `test_ppb_batch_update_all_valid`        ~ `test_batch_update_all_valid_succeeds`
//!   - `test_ppb_batch_delete_all_valid`        ~ `test_batch_delete_all_valid_succeeds`
//!   - `test_ppb_batch_create_partial_success`  ~ (no UUID analogue is un-ignored; same shape)
//!   - `test_ppb_batch_update_partial_success`  ~ `test_batch_update_partial_success`
//!   - `test_ppb_batch_delete_partial_success`  ~ `test_batch_delete_partial_success`
//!   - `test_ppb_batch_delete_dedups_input`     ~ `test_batch_delete_returns_only_existing_ids`

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels, SecurityProfile};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use serde_json::{Value, json};
use tower::ServiceExt;

/// Auto-increment `i32` PK. The id is DB-assigned (no `on_create`) and excluded
/// from the create/update models, exactly as a Uuid PK would exclude itself.
pub mod ppb_thing {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "ppb_things")]
    #[crudcrate(generate_router, api_struct = "Thing", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key)]
        #[crudcrate(primary_key, exclude(create, update))]
        pub id: i32,

        #[sea_orm(unique)]
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
    db.execute(
        &Table::drop()
            .table(ppb_thing::Entity)
            .if_exists()
            .to_owned(),
    )
    .await?;
    db.execute(&schema.create_table_from_entity(ppb_thing::Entity))
        .await?;
    // Ensure the unique index on `name` exists regardless of whether
    // `create_table_from_entity` emitted the `#[sea_orm(unique)]` one. A
    // duplicate-name insert is what induces a per-item failure in the partial
    // create test (parity for a DB-level conflict without custom hooks).
    db.execute_unprepared("CREATE UNIQUE INDEX ppb_things_name_unique ON ppb_things (name)")
        .await?;
    Ok(db)
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/things", ppb_thing::Thing::router(db).into())
}

/// Legacy profile keeps the `succeeded: [id]` / `failed` array shape and the
/// `[id, ...]` body on non-partial delete, so a test can inspect the integer
/// ids themselves rather than the secure-default count summary.
fn legacy_app(db: &DatabaseConnection) -> axum::Router {
    app(db).layer(axum::Extension(SecurityProfile::legacy()))
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

fn post(uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn patch(uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("PATCH")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn delete(uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

/// Asserts a JSON value is an integer (i64/u64), never a string. This is the
/// core parity claim: where a Uuid model would emit `"id": "<uuid>"`, the i32
/// model emits `"id": <int>`.
fn assert_is_int(v: &Value, ctx: &str) {
    assert!(
        v.is_i64() || v.is_u64(),
        "{ctx}: expected an integer id, got {v:?}"
    );
}

/// Seed N things named "thing-0".."thing-{N-1}" via single creates so the test
/// knows the exact auto-increment ids (1..=N on a fresh in-memory DB).
async fn seed(app: &axum::Router, n: usize) -> Vec<i64> {
    let mut ids = Vec::new();
    for i in 0..n {
        let (status, value) = send(
            app,
            post("/things", &json!({ "name": format!("thing-{i}") })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "seed create: {value:?}");
        assert_is_int(&value["id"], "seed id");
        ids.push(value["id"].as_i64().unwrap());
    }
    ids
}

// ============================================================================
// POST /things/batch: create_many
// ============================================================================

/// 201 + N rows, each with an integer id assigned by the DB.
#[tokio::test]
async fn test_ppb_batch_create_all_valid() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    let body = json!([
        { "name": "a", "color": "#aaa" },
        { "name": "b", "color": null },
        { "name": "c", "color": "#ccc" }
    ]);
    let (status, value) = send(&app, post("/things/batch", &body)).await;
    assert_eq!(status, StatusCode::CREATED, "batch create: {value:?}");

    let created = value.as_array().expect("batch create returns an array");
    assert_eq!(created.len(), 3);

    let mut ids: Vec<i64> = created
        .iter()
        .map(|t| {
            assert_is_int(&t["id"], "created id");
            t["id"].as_i64().unwrap()
        })
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, vec![1, 2, 3], "DB-assigned auto-increment ids");

    assert_eq!(created[0]["name"], "a");
    assert_eq!(created[1]["color"], Value::Null);
}

/// Empty batch create returns 201 with an empty array (parity edge case).
#[tokio::test]
async fn test_ppb_batch_create_empty() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    let (status, value) = send(&app, post("/things/batch", &json!([]))).await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(value.as_array().unwrap().is_empty());
}

/// Mixed valid/invalid create under `?partial=true` → 207 with succeeded items
/// carrying integer ids and a failed entry at the offending index.
///
/// The DB-level failure is a unique-constraint conflict on `name` (the middle
/// item reuses an already-existing name), which fails that one per-item insert
/// without needing custom validation hooks. This is the i32-PK analogue of the
/// UUID `test_batch_create_validation_partial_success` (ignored upstream because
/// the customer model has no validation; here a unique index supplies the error).
#[tokio::test]
async fn test_ppb_batch_create_partial_success() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    // Pre-existing row whose name the batch will collide with.
    let (status, _) = send(&app, post("/things", &json!({ "name": "dup" }))).await;
    assert_eq!(status, StatusCode::CREATED);

    let body = json!([
        { "name": "ok-0", "color": null },
        { "name": "dup",  "color": "#bad" }, // duplicate name -> unique conflict
        { "name": "ok-2", "color": null }
    ]);
    let (status, value) = send(&app, post("/things/batch?partial=true", &body)).await;
    assert_eq!(
        status,
        StatusCode::MULTI_STATUS,
        "partial create: {value:?}"
    );

    let succeeded = value["succeeded"].as_array().expect("succeeded array");
    assert_eq!(succeeded.len(), 2, "two valid creates: {value:?}");
    for item in succeeded {
        assert_is_int(&item["id"], "partial-create succeeded id");
    }
    let names: Vec<&str> = succeeded
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"ok-0") && names.contains(&"ok-2"));

    let failed = value["failed"].as_array().expect("failed array");
    assert_eq!(failed.len(), 1, "one invalid create");
    assert_eq!(
        failed[0]["index"].as_u64().unwrap(),
        1,
        "failure at index 1"
    );
}

// ============================================================================
// PATCH /things/batch: update_many
// ============================================================================

/// 200 + all rows updated; ids in the request body and response are integers.
#[tokio::test]
async fn test_ppb_batch_update_all_valid() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    let ids = seed(&app, 3).await;

    let updates: Vec<Value> = ids
        .iter()
        .map(|id| json!({ "id": id, "name": format!("renamed-{id}") }))
        .collect();
    let (status, value) = send(&app, patch("/things/batch", &json!(updates))).await;
    assert_eq!(status, StatusCode::OK, "batch update: {value:?}");

    let rows = value.as_array().expect("batch update returns an array");
    assert_eq!(rows.len(), 3);
    for item in rows {
        assert_is_int(&item["id"], "updated id");
    }

    // The new names are persisted and reachable by integer id path param.
    for id in &ids {
        let (status, value) = send(&app, get(&format!("/things/{id}"))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(value["id"], json!(id));
        assert_eq!(value["name"], format!("renamed-{id}"));
    }
}

/// Mixed valid/invalid update under `?partial=true` → 207. The non-existent
/// integer id (`9999`) is the failure, mirroring the UUID nil-id failure in
/// `test_batch_update_partial_success`.
#[tokio::test]
async fn test_ppb_batch_update_partial_success() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    let ids = seed(&app, 2).await;

    let updates = json!([
        { "id": ids[0], "name": "updated-0" },
        { "id": 9999,    "name": "ghost" }, // nonexistent integer id -> NOT_FOUND
        { "id": ids[1], "name": "updated-1" }
    ]);
    let (status, value) = send(&app, patch("/things/batch?partial=true", &updates)).await;
    assert_eq!(
        status,
        StatusCode::MULTI_STATUS,
        "partial update: {value:?}"
    );

    let succeeded = value["succeeded"].as_array().expect("succeeded array");
    assert_eq!(succeeded.len(), 2, "two valid updates");
    for item in succeeded {
        assert_is_int(&item["id"], "partial-update succeeded id");
    }

    let failed = value["failed"].as_array().expect("failed array");
    assert_eq!(failed.len(), 1, "one failed update");
    assert_eq!(
        failed[0]["index"].as_u64().unwrap(),
        1,
        "failure at index 1"
    );

    // All-or-nothing was NOT applied: the two valid updates committed.
    let (_, v0) = send(&app, get(&format!("/things/{}", ids[0]))).await;
    assert_eq!(v0["name"], "updated-0");
    // The ghost id never created a row.
    let (status, _) = send(&app, get("/things/9999")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// All-valid update under `?partial=true` → 200 with `BatchResult` shape and an
/// empty `failed` array.
#[tokio::test]
async fn test_ppb_batch_update_partial_all_succeed() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    let ids = seed(&app, 3).await;
    let updates: Vec<Value> = ids
        .iter()
        .map(|id| json!({ "id": id, "name": format!("p-{id}") }))
        .collect();

    let (status, value) = send(&app, patch("/things/batch?partial=true", &json!(updates))).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "all-succeed partial update: {value:?}"
    );
    assert_eq!(value["succeeded"].as_array().unwrap().len(), 3);
    assert!(value["failed"].as_array().unwrap().is_empty());
}

// ============================================================================
// DELETE /things/batch: delete_many
// ============================================================================

/// DELETE with a JSON array of INTEGERS `[1,2,3]` removes those rows.
/// Secure default profile reports a count; legacy profile echoes the integer ids.
#[tokio::test]
async fn test_ppb_batch_delete_all_valid() {
    let db = setup_test_db().await.unwrap();

    // Secure default: { "deleted": <count> } (mirrors integer_pk_test::test_integer_pk_batch_delete).
    let secure = app(&db);
    let ids = seed(&secure, 3).await;
    assert_eq!(ids, vec![1, 2, 3]);

    let (status, value) = send(&secure, delete("/things/batch", &json!([1, 2, 3]))).await;
    assert_eq!(status, StatusCode::OK, "batch delete: {value:?}");
    assert_eq!(value["deleted"], 3, "secure default reports a count");

    let (_, list) = send(&secure, get("/things")).await;
    assert!(list.as_array().unwrap().is_empty());
}

/// Legacy profile returns the array of deleted INTEGER ids verbatim.
#[tokio::test]
async fn test_ppb_batch_delete_returns_integer_ids() {
    let db = setup_test_db().await.unwrap();
    let app = legacy_app(&db);

    let ids = seed(&app, 3).await;

    let (status, value) = send(&app, delete("/things/batch", &json!([1, 2, 3]))).await;
    assert_eq!(status, StatusCode::OK, "legacy batch delete: {value:?}");

    let returned = value.as_array().expect("legacy delete returns an id array");
    let mut returned_ids: Vec<i64> = returned
        .iter()
        .map(|v| {
            assert_is_int(v, "deleted id");
            v.as_i64().unwrap()
        })
        .collect();
    returned_ids.sort_unstable();
    assert_eq!(returned_ids, ids);
}

/// `delete_many` de-duplicates a repeated integer id: `[1,1,2]` deletes two
/// distinct rows and reports them once each, never over-counting.
/// Mirrors the phantom-id de-dup guarantee in
/// `test_batch_delete_returns_only_existing_ids` (UUID).
#[tokio::test]
async fn test_ppb_batch_delete_dedups_input() {
    let db = setup_test_db().await.unwrap();
    let app = legacy_app(&db);

    let ids = seed(&app, 2).await;
    assert_eq!(ids, vec![1, 2]);

    // [1, 1, 2]: id 1 repeated. Plus a nonexistent id to prove phantom drop.
    let (status, value) = send(&app, delete("/things/batch", &json!([1, 1, 2, 999]))).await;
    assert_eq!(status, StatusCode::OK, "dedup delete: {value:?}");

    let returned: Vec<i64> = value
        .as_array()
        .expect("id array")
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert_eq!(
        returned.len(),
        2,
        "duplicate [1,1] collapses to one id; 999 (phantom) dropped: {returned:?}"
    );
    let mut sorted = returned.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, vec![1, 2], "exactly the two distinct existing ids");

    // Both rows are actually gone.
    for id in [1, 2] {
        let (status, _) = send(&app, get(&format!("/things/{id}"))).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "id {id} should be deleted");
    }
}

/// `delete_many` count under the secure default also de-duplicates: `[1,1,2]`
/// reports `{ "deleted": 2 }`, not 3.
#[tokio::test]
async fn test_ppb_batch_delete_dedups_count_secure() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);

    seed(&app, 2).await;

    let (status, value) = send(&app, delete("/things/batch", &json!([1, 1, 2]))).await;
    assert_eq!(status, StatusCode::OK, "dedup delete (secure): {value:?}");
    assert_eq!(value["deleted"], 2, "duplicate id counted once");
}

/// Mixed existing/nonexistent delete under `?partial=true` → 207 with integer
/// ids in `succeeded` and the offending index in `failed` (legacy profile keeps
/// the array shape). Mirrors `test_batch_delete_partial_success` (UUID).
#[tokio::test]
async fn test_ppb_batch_delete_partial_success() {
    let db = setup_test_db().await.unwrap();
    let app = legacy_app(&db);

    let ids = seed(&app, 2).await;

    let body = json!([ids[0], 9999, ids[1]]); // middle id is nonexistent
    let (status, value) = send(&app, delete("/things/batch?partial=true", &body)).await;
    assert_eq!(
        status,
        StatusCode::MULTI_STATUS,
        "partial delete: {value:?}"
    );

    let succeeded = value["succeeded"].as_array().expect("succeeded array");
    assert_eq!(succeeded.len(), 2, "two valid deletes");
    let mut ok_ids: Vec<i64> = succeeded
        .iter()
        .map(|v| {
            assert_is_int(v, "partial-delete succeeded id");
            v.as_i64().unwrap()
        })
        .collect();
    ok_ids.sort_unstable();
    assert_eq!(ok_ids, ids);

    let failed = value["failed"].as_array().expect("failed array");
    assert_eq!(failed.len(), 1, "one failed delete");
    assert_eq!(
        failed[0]["index"].as_u64().unwrap(),
        1,
        "failure at index 1"
    );

    // Both existing rows are gone.
    for id in &ids {
        let (status, _) = send(&app, get(&format!("/things/{id}"))).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

/// Partial delete with ALL nonexistent integer ids → 400 with empty `succeeded`.
/// Mirrors `test_batch_delete_partial_all_fail` (UUID).
#[tokio::test]
async fn test_ppb_batch_delete_partial_all_fail() {
    let db = setup_test_db().await.unwrap();
    let app = legacy_app(&db);

    let (status, value) = send(
        &app,
        delete("/things/batch?partial=true", &json!([777, 888])),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "all-fail partial delete: {value:?}"
    );

    assert!(value["succeeded"].as_array().unwrap().is_empty());
    assert_eq!(value["failed"].as_array().unwrap().len(), 2);
}
