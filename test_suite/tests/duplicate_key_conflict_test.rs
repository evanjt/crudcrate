// Regression for A8: duplicate-key inserts must surface as 409 CONFLICT, not an
// opaque 500. A unique-constraint violation is a client error.
//
// Two angles are covered:
//   - the HTTP layer (POST /users with a duplicate email -> 409 CONFLICT)
//   - the trait layer (`CRUDResource::create` returns `ApiError::Conflict`, which
//     renders as a 409 via `IntoResponse`)
//
// The model carries a UNIQUE `email` column. We mark it `#[sea_orm(unique)]` and
// additionally create the unique index by hand so enforcement does not depend on
// `create_table_from_entity` emitting the constraint. A guard test asserts the
// constraint is actually live (a second insert fails) so a silently-missing index
// can't make the conflict tests pass for the wrong reason.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use crudcrate::{ApiError, CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use tower::ServiceExt;
use uuid::Uuid;

pub mod dkc_user {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "dkc_users")]
    #[crudcrate(generate_router, api_struct = "DkcUser")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

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

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let db = Database::connect(&url).await?;
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    // Persistent backends (Postgres/MySQL) keep tables across tests within a binary;
    // drop first so every test starts from a clean schema. On sqlite::memory: each
    // connection is a fresh database, so the drop is a harmless no-op. Dropping the
    // table also removes its unique index, so the index recreate below is clean.
    db.execute(backend.build(&Table::drop().table(dkc_user::Entity).if_exists().to_owned()))
        .await?;
    db.execute(backend.build(&schema.create_table_from_entity(dkc_user::Entity)))
        .await?;

    // Belt-and-suspenders: ensure a unique index on `email` exists regardless of
    // whether `create_table_from_entity` emitted the `#[sea_orm(unique)]` one. The
    // table is freshly dropped+recreated above, so the index never pre-exists and we
    // omit `IF NOT EXISTS` (MySQL rejects it on CREATE INDEX).
    db.execute_unprepared("CREATE UNIQUE INDEX dkc_users_email_unique ON dkc_users (email)")
        .await?;

    Ok(db)
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/users", dkc_user::DkcUser::router(db).into())
}

async fn post_user(db: &DatabaseConnection, name: &str, email: &str) -> axum::http::Response<Body> {
    app(db)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/users")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "name": name, "email": email }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

// Guard: prove the unique constraint is actually enforced. If a second raw insert
// of the same email succeeds, the index was never created and the conflict tests
// below would be meaningless.
#[tokio::test]
async fn unique_constraint_is_enforced() {
    let db = setup_test_db().await.expect("setup failed");

    let first = dkc_user::DkcUser::create(
        &db,
        dkc_user::DkcUserCreate {
            name: "first".to_string(),
            email: "guard@test.com".to_string(),
        },
    )
    .await;
    assert!(first.is_ok(), "first insert should succeed: {first:?}");

    let second = dkc_user::DkcUser::create(
        &db,
        dkc_user::DkcUserCreate {
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

#[tokio::test]
async fn http_duplicate_email_returns_409_conflict() {
    let db = setup_test_db().await.expect("setup failed");

    let created = post_user(&db, "a", "dup@test.com").await;
    assert_eq!(
        created.status(),
        StatusCode::CREATED,
        "first POST should create the user"
    );

    let conflict = post_user(&db, "a", "dup@test.com").await;
    assert_eq!(
        conflict.status(),
        StatusCode::CONFLICT,
        "second POST with a duplicate email must be 409 CONFLICT, not 500"
    );
}

#[tokio::test]
async fn trait_create_duplicate_maps_to_conflict() {
    let db = setup_test_db().await.expect("setup failed");

    let first = dkc_user::DkcUser::create(
        &db,
        dkc_user::DkcUserCreate {
            name: "a".to_string(),
            email: "trait-dup@test.com".to_string(),
        },
    )
    .await;
    assert!(first.is_ok(), "first create should succeed: {first:?}");

    let second = dkc_user::DkcUser::create(
        &db,
        dkc_user::DkcUserCreate {
            name: "b".to_string(),
            email: "trait-dup@test.com".to_string(),
        },
    )
    .await;

    let err = second.expect_err("second create with duplicate email must error");

    // The ApiError enum is public; a unique-constraint violation maps to Conflict.
    assert!(
        matches!(err, ApiError::Conflict { .. }),
        "expected ApiError::Conflict, got {err:?}"
    );

    // And rendered as an HTTP response it is a 409, confirming the user-facing status.
    let status = err.into_response().status();
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "ApiError from a duplicate key must render as 409 CONFLICT"
    );
}
