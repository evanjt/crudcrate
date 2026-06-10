// Error / validation parity for a non-UUID (auto-increment i32) primary key.
//
// Proves the error paths a UUID-PK model produces are the same when the PK is
// an integer. Each test notes the UUID-based test it mirrors:
//
//   - duplicate unique column -> 409 CONFLICT   (mirrors duplicate_key_conflict_test.rs)
//   - Validatable create failure -> 422         (mirrors validatable_auto_test.rs)
//   - GET /things/{int} nonexistent -> 404 with the integer in the message
//                                               (mirrors integer_pk_test::test_integer_pk_get_one,
//                                                and the message shape of the UUID 404)
//   - GET /things/not-an-int (malformed path) -> 400/404, never 500/panic
//
// The PK is `#[sea_orm(primary_key)] pub id: i32` (auto-increment). The id is
// never given an on_create — the database assigns it — and is excluded from
// create/update. Unique prefix "ppe" keeps tables/structs from colliding with
// other self-contained suites.
//
// Run with:
//     DATABASE_URL="sqlite::memory:" cargo test -p test_suite --test pk_parity_errors_validation_test -- --test-threads=1

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use crudcrate::validation::{Validatable, ValidationError};
use crudcrate::{ApiError, CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use serde_json::{Value, json};
use tower::ServiceExt;

// =============================================================================
// Entity with a UNIQUE column (for the 409 path) AND a `Validatable` Create impl
// (for the 422 path). Integer auto-increment PK.
// =============================================================================

pub mod ppe_thing {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "ppe_things")]
    #[crudcrate(generate_router, api_struct = "PpeThing", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key)]
        #[crudcrate(primary_key, exclude(create, update))]
        pub id: i32,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[sea_orm(unique)]
        #[crudcrate(filterable, sortable)]
        pub email: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// name must be present and at least 3 characters — the same rule the UUID
// `validatable_auto_test` exercises, so a failure must surface as 422 here too.
fn name_is_valid(name: &str) -> Result<(), ValidationError> {
    if name.is_empty() {
        return Err(ValidationError::new("name", "Name is required"));
    }
    if name.chars().count() < 3 {
        return Err(ValidationError::new(
            "name",
            "Name must be at least 3 characters",
        ));
    }
    Ok(())
}

impl Validatable for ppe_thing::PpeThingCreate {
    fn validate(&self) -> Result<(), ValidationError> {
        name_is_valid(&self.name)
    }
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
            .table(ppe_thing::Entity)
            .if_exists()
            .to_owned(),
    )
    .await?;
    db.execute(&schema.create_table_from_entity(ppe_thing::Entity))
        .await?;

    // Belt-and-suspenders: ensure the unique index on `email` exists regardless
    // of whether `create_table_from_entity` emitted the `#[sea_orm(unique)]` one,
    // matching duplicate_key_conflict_test's setup.
    db.execute_unprepared("CREATE UNIQUE INDEX ppe_things_email_unique ON ppe_things (email)")
        .await?;

    Ok(db)
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/things", ppe_thing::PpeThing::router(db).into())
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

async fn post_thing(app: &axum::Router, name: &str, email: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri("/things")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "name": name, "email": email }).to_string(),
        ))
        .unwrap();
    send(app, req).await
}

// =============================================================================
// Guard: the unique constraint is actually live. Mirrors
// duplicate_key_conflict_test::unique_constraint_is_enforced so a silently
// missing index can't make the 409 tests pass for the wrong reason.
// =============================================================================

#[tokio::test]
async fn unique_constraint_is_enforced() {
    let db = setup_test_db().await.expect("setup failed");

    let first = ppe_thing::PpeThing::create(
        &db,
        ppe_thing::PpeThingCreate {
            name: "first".to_string(),
            email: "guard@test.com".to_string(),
        },
    )
    .await;
    assert!(first.is_ok(), "first insert should succeed: {first:?}");

    let second = ppe_thing::PpeThing::create(
        &db,
        ppe_thing::PpeThingCreate {
            name: "second".to_string(),
            email: "guard@test.com".to_string(),
        },
    )
    .await;
    assert!(
        second.is_err(),
        "second insert with duplicate email must fail — unique index is missing if this succeeds"
    );
}

// =============================================================================
// 409 CONFLICT parity. Mirrors
// duplicate_key_conflict_test::http_duplicate_email_returns_409_conflict.
// =============================================================================

#[tokio::test]
async fn http_duplicate_email_returns_409_conflict() {
    let db = setup_test_db().await.expect("setup failed");
    let app = app(&db);

    let (created_status, created) = post_thing(&app, "alice", "dup@test.com").await;
    assert_eq!(
        created_status,
        StatusCode::CREATED,
        "first POST should create the thing: {created:?}"
    );
    // The integer PK round-trips in the create response body, same as a UUID would.
    assert!(
        created["id"].is_i64() || created["id"].is_u64(),
        "id should be an integer, got {:?}",
        created["id"]
    );
    assert_eq!(created["id"], 1);

    let (conflict_status, _) = post_thing(&app, "bob", "dup@test.com").await;
    assert_eq!(
        conflict_status,
        StatusCode::CONFLICT,
        "second POST with a duplicate email must be 409 CONFLICT, not 500 — same as the UUID model"
    );
}

// Trait-layer parity. Mirrors
// duplicate_key_conflict_test::trait_create_duplicate_maps_to_conflict.
#[tokio::test]
async fn trait_create_duplicate_maps_to_conflict() {
    let db = setup_test_db().await.expect("setup failed");

    let first = ppe_thing::PpeThing::create(
        &db,
        ppe_thing::PpeThingCreate {
            name: "alice".to_string(),
            email: "trait-dup@test.com".to_string(),
        },
    )
    .await;
    assert!(first.is_ok(), "first create should succeed: {first:?}");

    let second = ppe_thing::PpeThing::create(
        &db,
        ppe_thing::PpeThingCreate {
            name: "bob".to_string(),
            email: "trait-dup@test.com".to_string(),
        },
    )
    .await;

    let err = second.expect_err("second create with duplicate email must error");
    assert!(
        matches!(err, ApiError::Conflict { .. }),
        "expected ApiError::Conflict, got {err:?}"
    );

    let status = err.into_response().status();
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "ApiError from a duplicate key must render as 409 CONFLICT regardless of PK type"
    );
}

// =============================================================================
// 422 validation parity. Mirrors
// validatable_auto_test::post_empty_name_is_rejected_with_422 /
// post_too_short_name_is_rejected_with_422 / post_valid_name_is_created.
// =============================================================================

#[tokio::test]
async fn post_invalid_name_is_rejected_with_422() {
    let db = setup_test_db().await.expect("setup failed");
    let app = app(&db);

    // Empty name fails the Validatable rule -> 422.
    let (empty_status, _) = post_thing(&app, "", "empty@test.com").await;
    assert_eq!(
        empty_status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "empty name must fail validation with 422 — same as the UUID model"
    );

    // Too-short name fails the >=3-char rule -> 422.
    let (short_status, _) = post_thing(&app, "ab", "short@test.com").await;
    assert_eq!(
        short_status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a 2-char name is shorter than the 3-char minimum"
    );

    // Nothing was persisted by either rejected create.
    let count = ppe_thing::Entity::find().count(&db).await.expect("count");
    assert_eq!(count, 0, "no rows should persist when validation fails");
}

#[tokio::test]
async fn post_valid_name_is_created() {
    let db = setup_test_db().await.expect("setup failed");
    let app = app(&db);

    let (status, body) = post_thing(&app, "okay", "ok@test.com").await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "a >=3-char name must pass validation and be created: {body:?}"
    );
    assert_eq!(body["name"], "okay");
    assert_eq!(body["id"], 1);
}

// =============================================================================
// 404 parity for a nonexistent integer id, with the integer present in the
// message. Mirrors integer_pk_test::test_integer_pk_get_one (404 branch) plus
// the UUID 404 message shape ("<Resource> with ID '<id>' not found").
// =============================================================================

#[tokio::test]
async fn get_nonexistent_integer_id_returns_404_with_id_in_message() {
    let db = setup_test_db().await.expect("setup failed");
    let app = app(&db);

    // Seed one row so the table is non-empty; id 1 exists, 999 does not.
    let (created_status, _) = post_thing(&app, "alice", "exists@test.com").await;
    assert_eq!(created_status, StatusCode::CREATED);

    let req = Request::builder()
        .method("GET")
        .uri("/things/999")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(&app, req).await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a missing integer id must be 404, not a parse error or 500"
    );

    // The integer id is echoed into the user-facing message, exactly as a UUID
    // id would be (ApiError::not_found formats `with ID '<id>'`).
    let message = body["error"].as_str().unwrap_or_default();
    assert!(
        message.contains("999"),
        "the 404 message must contain the integer id, got {message:?}"
    );
    assert!(
        message.contains("not found"),
        "the 404 message shape must match the UUID model, got {message:?}"
    );
}

// =============================================================================
// Malformed path parity: a non-integer segment must be rejected cleanly. Axum's
// Path<i32> extraction fails with a 4xx (400/404) — it must never 500 or panic.
// A UUID model behaves the same when handed a non-UUID segment.
// =============================================================================

#[tokio::test]
async fn malformed_integer_path_does_not_500_or_panic() {
    let db = setup_test_db().await.expect("setup failed");
    let app = app(&db);

    for bad in ["not-an-int", "1.5", "abc", "0x10"] {
        let req = Request::builder()
            .method("GET")
            .uri(format!("/things/{bad}"))
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(&app, req).await;

        assert!(
            status == StatusCode::BAD_REQUEST || status == StatusCode::NOT_FOUND,
            "malformed integer path {bad:?} should be 400 or 404, got {status} ({body:?})"
        );
        assert_ne!(
            status,
            StatusCode::INTERNAL_SERVER_ERROR,
            "malformed integer path {bad:?} must not produce a 500"
        );
    }
}
