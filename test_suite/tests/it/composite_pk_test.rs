//! Composite primary keys end to end.
//!
//! An entity whose key is more than one column gets the same generated models, list, filtering,
//! sorting and pagination as a single-key one, and the same `CRUDResource` methods for a single
//! row. The single-row HTTP routes are not mounted: a composite key has no settled path spelling
//! (Q153), and nothing here depends on one.

use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr};
use test_suite::http;

/// Two columns, both in the key, neither of them a UUID: the shape `notification_state`
/// (`kind+subject_key`) and `user_project_grants` (`user_sub+project_id`) have.
pub mod grant {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "cpk_grants")]
    #[crudcrate(generate_router, api_struct = "Grant", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(update))]
        pub user_sub: String,

        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(update))]
        pub project_id: i32,

        #[crudcrate(filterable, sortable)]
        pub role: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(grant::Entity).await
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/grants", grant::Grant::router(db).into())
}

async fn seed(db: &DatabaseConnection, rows: &[(&str, i32, &str)]) {
    for (user_sub, project_id, role) in rows {
        let model = grant::ActiveModel {
            user_sub: sea_orm::ActiveValue::Set((*user_sub).to_string()),
            project_id: sea_orm::ActiveValue::Set(*project_id),
            role: sea_orm::ActiveValue::Set((*role).to_string()),
        };
        <grant::Entity as EntityTrait>::insert(model)
            .exec(db)
            .await
            .expect("seed a grant");
    }
}

/// The gate this item removes: more than one `primary_key` field was a compile error, so the
/// derive could not be put on a composite-key model at all.
#[tokio::test]
async fn a_composite_key_entity_derives_and_lists() {
    let db = setup_test_db().await.expect("db");
    seed(
        &db,
        &[
            ("alice", 1, "admin"),
            ("alice", 2, "reader"),
            ("bob", 1, "reader"),
        ],
    )
    .await;
    let app = app(&db);

    let (status, body, _) = http::get_with_headers(&app, "/grants").await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body:?}");
    assert_eq!(body.as_array().expect("a list").len(), 3, "{body:?}");
}

/// The key is what orders a page, so every key column has to be in the tie-break or two pages can
/// repeat or skip a row.
#[tokio::test]
async fn a_composite_key_paginates_without_repeating_a_row() {
    let db = setup_test_db().await.expect("db");
    seed(
        &db,
        &[
            ("alice", 1, "admin"),
            ("alice", 2, "reader"),
            ("bob", 1, "reader"),
            ("bob", 2, "admin"),
        ],
    )
    .await;
    let app = app(&db);

    let mut seen: Vec<String> = Vec::new();
    for range in ["[0,1]", "[2,3]"] {
        let (status, body, _) =
            http::get_with_headers(&app, &format!("/grants?range={range}")).await;
        assert_eq!(status, axum::http::StatusCode::OK, "{body:?}");
        for row in body.as_array().expect("a list") {
            seen.push(format!(
                "{}:{}",
                row["user_sub"].as_str().expect("user_sub"),
                row["project_id"].as_i64().expect("project_id")
            ));
        }
    }
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        4,
        "two pages covered every row exactly once: {seen:?}"
    );
}

/// `get_one`, `update` and `delete` take the key as a tuple. They are the trait's methods, not
/// routes, so a hand-written handler can reach one row without the path question being settled.
#[tokio::test]
async fn the_trait_reaches_one_row_by_its_whole_key() {
    let db = setup_test_db().await.expect("db");
    seed(&db, &[("alice", 1, "admin"), ("bob", 1, "reader")]).await;

    let found = grant::Grant::get_one(&db, ("alice".to_string(), 1))
        .await
        .expect("the row is found by its whole key");
    assert_eq!(found.role, "admin");

    let missing = grant::Grant::get_one(&db, ("alice".to_string(), 99)).await;
    assert!(
        missing.is_err(),
        "a key that names no row is not found, and the message renders the whole key"
    );

    let deleted = grant::Grant::delete(&db, ("bob".to_string(), 1))
        .await
        .expect("delete by whole key");
    assert_eq!(deleted, ("bob".to_string(), 1));
}

/// A batch delete cannot be `ID_COLUMN.is_in(ids)` when the key is several columns.
#[tokio::test]
async fn a_batch_delete_takes_whole_keys() {
    let db = setup_test_db().await.expect("db");
    seed(
        &db,
        &[
            ("alice", 1, "admin"),
            ("alice", 2, "reader"),
            ("bob", 1, "reader"),
        ],
    )
    .await;

    let removed = grant::Grant::delete_many(
        &db,
        vec![
            ("alice".to_string(), 1),
            ("bob".to_string(), 1),
            ("nobody".to_string(), 7),
        ],
    )
    .await
    .expect("batch delete");
    assert_eq!(
        removed,
        vec![("alice".to_string(), 1), ("bob".to_string(), 1)],
        "only the keys that existed are echoed back"
    );

    let left = grant::Grant::get_all(
        &db,
        &sea_orm::Condition::all(),
        grant::Column::Role,
        sea_orm::Order::Asc,
        0,
        100,
    )
    .await
    .expect("list what is left");
    assert_eq!(left.len(), 1, "the rows named were the rows removed");
}

/// What the single-row routes do today if they are mounted: `Path<(A, B)>` against a one-segment
/// `/{id}` cannot deserialize, so the route exists and can never succeed. Not mounting it is why.
#[tokio::test]
async fn a_single_row_route_is_not_mounted_for_a_composite_key() {
    let db = setup_test_db().await.expect("db");
    seed(&db, &[("alice", 1, "admin")]).await;
    let app = app(&db);

    let (status, _, _) = http::get_with_headers(&app, "/grants/alice").await;
    assert_eq!(
        status,
        axum::http::StatusCode::NOT_FOUND,
        "no single-row route is mounted, so the path does not resolve at all"
    );
}
