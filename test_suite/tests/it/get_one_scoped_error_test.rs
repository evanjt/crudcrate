//! Scoped `get_one` must surface real hook errors instead of masking them as 404.
//!
//! The scoped `get_one` handler runs the resource's read hooks inside
//! `get_one_scoped`, then propagates the result with `?`. The handler used to
//! wrap that call in `.map_err(|_| ApiError::not_found(..))`, which collapsed
//! EVERY error from the scoped fetch (including genuine 500-class faults raised
//! by a `read::one::transform` / `read::one::post` hook) down to a 404. That hid
//! real failures behind a "missing row" response.
//!
//! These tests pin both halves of the contract for a Uuid-PK scoped resource:
//!
//! - Scope HIT + failing hook: the row matches the scope condition, so
//!   `get_one_scoped` fetches it and runs the `read::one::transform` hook, which
//!   returns `ApiError::internal("boom", None)`. The handler must return 500, not
//!   a masked 404.
//! - Scope MISS: the row is excluded by the scope condition, so `get_one_scoped`
//!   returns `NotFound` before the hook ever runs. That still surfaces as 404, so
//!   excluded rows stay indistinguishable from missing ones.
//!
//! Real SQLite-in-memory database, no mocks. Mirrors the `ScopeCondition` layering
//! pattern from `scope_security_test.rs` / `pk_parity_scope_test.rs`.

use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use crudcrate::{ApiError, CRUDResource, EntityToModels, ScopeCondition};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{ActiveValue::Set, Condition, Database, DatabaseConnection, DbErr, Schema};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

/// `read::one::transform` hook that always fails. Reached only when the scoped
/// fetch returns a row, ie. when the scope condition INCLUDES it. Returning a
/// 500-class error here is the exact fault the handler must no longer mask.
#[allow(clippy::unused_async)]
async fn boom_on_read(
    _db: &DatabaseConnection,
    _entity: thing::Thing,
) -> Result<thing::Thing, ApiError> {
    Err(ApiError::internal("boom", None))
}

// Unique slug prefix "gose" so nothing collides with other suites.
pub mod thing {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "gose_things")]
    #[crudcrate(
        generate_router,
        api_struct = "Thing",
        read::one::transform = super::boom_on_read
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        // Scope-filtered column: defaulted to false on create, excluded from
        // scoped responses. The scope middleware filters on is_private = false.
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
    // drop first so every test starts from a clean schema and empty data. On
    // sqlite::memory: each connection is a fresh database, so the drop is a no-op.
    db.execute(&Table::drop().table(thing::Entity).if_exists().to_owned())
        .await?;
    db.execute(&schema.create_table_from_entity(thing::Entity))
        .await?;
    Ok(db)
}

/// Scoped app: every request carries a `ScopeCondition` filtering `is_private=false`,
/// so public rows are INCLUDED by the scope and private rows are EXCLUDED.
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

/// GET against the scoped app, returning the status and parsed JSON body.
async fn get(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

/// Seed a row by inserting its `ActiveModel` directly, bypassing the `CRUDResource`
/// `create` path. That path re-fetches via `get_one`, which would run the failing
/// `read::one::transform` hook during setup, so seeding through it is impossible.
/// Inserting straight into the table isolates the test to the `get_one_scoped` path.
async fn seed(db: &DatabaseConnection, name: &str, is_private: bool) -> Uuid {
    let id = Uuid::new_v4();
    thing::ActiveModel {
        id: Set(id),
        name: Set(name.to_string()),
        is_private: Set(is_private),
    }
    .insert(db)
    .await
    .expect("seed insert");
    id
}

// =============================================================================
// Scope HIT + failing read hook: the row matches the scope, so get_one_scoped
// fetches it and runs read::one::transform, which fails. The handler must
// propagate that as 500, NOT mask it as 404.
// =============================================================================

#[tokio::test]
async fn scoped_get_one_surfaces_hook_error_as_500_for_in_scope_row() {
    let db = setup_test_db().await.unwrap();
    let scoped = scoped_app(&db);

    // Public row: INCLUDED by the scope condition (is_private = false).
    let id = seed(&db, "Visible", false).await;

    let (status, body) = get(&scoped, &format!("/things/{id}")).await;
    assert_eq!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "in-scope row whose read hook fails must surface 500, not a masked 404; body: {body:?}"
    );
    // And the 500 must not be the row payload leaking through.
    assert!(
        body.get("name").is_none(),
        "500 response must not leak the row, got: {body}"
    );
}

// =============================================================================
// Scope MISS: the row is excluded by the scope condition, so get_one_scoped
// returns NotFound before the hook runs: still a clean 404, no leak.
// =============================================================================

#[tokio::test]
async fn scoped_get_one_returns_404_for_out_of_scope_row_without_running_hook() {
    let db = setup_test_db().await.unwrap();
    let scoped = scoped_app(&db);

    // Private row: EXCLUDED by the scope condition. The failing transform hook is
    // never reached, so this is a genuine 404, not a hook-induced 500.
    let id = seed(&db, "Hidden", true).await;

    let (status, body) = get(&scoped, &format!("/things/{id}")).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "out-of-scope row must 404 before the read hook runs; body: {body:?}"
    );
    assert!(
        body.get("name").is_none(),
        "404 response must not leak the row, got: {body}"
    );
}

// =============================================================================
// A nonexistent id behaves the same as an out-of-scope row: 404, hook never
// runs. Confirms the 500 in the first test is hook-driven, not fetch-driven.
// =============================================================================

#[tokio::test]
async fn scoped_get_one_returns_404_for_nonexistent_id() {
    let db = setup_test_db().await.unwrap();
    let scoped = scoped_app(&db);

    let fake_id = "00000000-0000-0000-0000-ffffffffffff";
    let (status, _) = get(&scoped, &format!("/things/{fake_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
