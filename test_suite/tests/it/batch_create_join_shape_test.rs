//! Response-shape parity between `POST /{resource}/batch` (all-or-nothing) and
//! `POST /{resource}/batch?partial=true` (partial success) for a resource that
//! has BOTH a `Vec<Child>` join field AND a `read::one::transform` hook.
//!
//! Both batch-create modes go through `CRUDResource::create_many`, which neither
//! join-loads children nor runs `read::one::transform`. The partial path
//! previously called the single `create` (which re-fetches via `get_one`, join
//! loads, and applies `read::one::transform`), so its per-item shape diverged
//! from the all-or-nothing path. The earlier shape-parity test used a join-less,
//! transform-less entity and so could not observe that divergence.
//!
//! This test pins the agreed-upon contract: in BOTH modes each created item is
//! the flat `create_many` shape: the join field is empty/default and the
//! `read::one::transform` hook is NOT applied (the marker stays at its raw,
//! posted value). If the partial path regressed to per-item `create`, the two
//! shapes would differ (transformed name and/or populated join field), failing
//! the key-set and value assertions below.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{ApiError, CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

/// Child entity owned by the parent via a `parent_id` foreign key. Plain CRUD
/// resource; it only exists so the parent can declare a `Vec<Child>` join.
pub mod bcjs_child {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "bcjs_children")]
    #[crudcrate(
        generate_router,
        api_struct = "BcjsChild",
        name_singular = "bcjs_child",
        name_plural = "bcjs_children",
        derive_partial_eq
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub parent_id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub label: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::bcjs_parent::Entity",
            from = "Column::ParentId",
            to = "super::bcjs_parent::Column::Id"
        )]
        Parent,
    }

    impl Related<super::bcjs_parent::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Parent.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// Parent entity with a `Vec<BcjsChild>` join field (`join(one, all)`, so a
/// `get_one`/`get_all` WOULD populate it) AND a `read::one::transform` hook that
/// rewrites `name`. Neither effect should appear on the batch-create response.
pub mod bcjs_parent {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "bcjs_parents")]
    #[crudcrate(
        generate_router,
        api_struct = "BcjsParent",
        name_singular = "bcjs_parent",
        name_plural = "bcjs_parents",
        read::one::transform = super::transform_parent_after_read_one,
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, exclude(create, update), join(one, all, depth = 1))]
        pub children: Vec<super::bcjs_child::BcjsChild>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::bcjs_child::Entity")]
        Children,
    }

    impl Related<super::bcjs_child::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Children.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

use bcjs_parent::BcjsParent;

/// `read::one::transform` hook: rewrites `name` so that anywhere this hook runs
/// the response carries a `_read_transformed` suffix. The batch-create paths
/// must NOT run it, so a created parent's `name` stays exactly as posted.
///
/// Must be `async` to match the hook signature the derive macro calls (`.await`).
#[allow(clippy::unused_async)]
async fn transform_parent_after_read_one(
    _db: &DatabaseConnection,
    mut entity: BcjsParent,
) -> Result<BcjsParent, ApiError> {
    entity.name = format!("{}_read_transformed", entity.name);
    Ok(entity)
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(bcjs_parent::Entity, bcjs_child::Entity).await
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new()
        .nest("/parents", bcjs_parent::BcjsParent::router(db).into())
        .nest("/children", bcjs_child::BcjsChild::router(db).into())
}

async fn post_batch(db: &DatabaseConnection, uri: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app(db).oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    (status, value)
}

/// Sorted JSON object key set for an item, used to compare response shapes.
fn keys(item: &Value) -> Vec<String> {
    let mut ks: Vec<String> = item
        .as_object()
        .expect("item is a JSON object")
        .keys()
        .cloned()
        .collect();
    ks.sort();
    ks
}

/// Asserts a single created-parent item is the flat `create_many` shape:
/// raw (un-transformed) name and an empty join field.
fn assert_flat_create_shape(item: &Value, expected_name: &str) {
    assert_eq!(
        item["name"].as_str().unwrap(),
        expected_name,
        "name must be the raw posted value, NOT the read::one::transform output \
         ('{expected_name}_read_transformed'); transform must not run on batch create"
    );
    let children = item["children"]
        .as_array()
        .expect("join field `children` present as an array");
    assert!(
        children.is_empty(),
        "create_many must not join-load children: {children:?}"
    );
    assert_eq!(
        keys(item),
        vec!["children".to_string(), "id".to_string(), "name".to_string()],
        "created item should expose exactly id, name, and the (empty) join field"
    );
}

const BODY: fn() -> Value = || json!([{ "name": "alpha" }, { "name": "beta" }]);

/// All-or-nothing batch create returns a bare array of flat-shaped items.
#[tokio::test]
async fn test_all_or_nothing_create_is_flat_shape() {
    let db = setup_test_db().await.expect("db setup");
    let (status, body) = post_batch(&db, "/parents/batch", BODY()).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "all-or-nothing create: {body:?}"
    );

    let items = body
        .as_array()
        .expect("all-or-nothing returns a bare array");
    assert_eq!(items.len(), 2);
    assert_flat_create_shape(&items[0], "alpha");
    assert_flat_create_shape(&items[1], "beta");
}

/// Partial batch create wraps the SAME flat-shaped items under `succeeded`.
#[tokio::test]
async fn test_partial_create_is_flat_shape() {
    let db = setup_test_db().await.expect("db setup");
    let (status, body) = post_batch(&db, "/parents/batch?partial=true", BODY()).await;
    assert_eq!(status, StatusCode::CREATED, "partial create: {body:?}");

    let items = body["succeeded"]
        .as_array()
        .expect("partial returns a succeeded array");
    assert_eq!(items.len(), 2);
    assert_flat_create_shape(&items[0], "alpha");
    assert_flat_create_shape(&items[1], "beta");
    assert!(
        body["failed"].as_array().expect("failed array").is_empty(),
        "no items should fail"
    );
}

/// The core parity claim: per-item key sets, the join-field representation, and
/// the transform-applied state are IDENTICAL between the two batch modes.
#[tokio::test]
async fn test_batch_create_shape_parity_between_modes() {
    let plain_db = setup_test_db().await.expect("db setup");
    let (plain_status, plain_body) = post_batch(&plain_db, "/parents/batch", BODY()).await;
    assert_eq!(plain_status, StatusCode::CREATED);
    let plain_items = plain_body.as_array().expect("all-or-nothing array");

    let partial_db = setup_test_db().await.expect("db setup");
    let (partial_status, partial_body) =
        post_batch(&partial_db, "/parents/batch?partial=true", BODY()).await;
    assert_eq!(partial_status, StatusCode::CREATED);
    let partial_items = partial_body["succeeded"]
        .as_array()
        .expect("partial succeeded array");

    assert_eq!(plain_items.len(), partial_items.len(), "same item count");

    for (i, (plain, partial)) in plain_items.iter().zip(partial_items.iter()).enumerate() {
        assert_eq!(
            keys(plain),
            keys(partial),
            "item {i}: key sets must match across batch modes"
        );

        // Join-field representation matches (both empty arrays).
        assert_eq!(
            plain["children"], partial["children"],
            "item {i}: join-field representation must match"
        );

        // Transform-applied state matches: neither mode applied the
        // read::one::transform suffix.
        assert_eq!(
            plain["name"], partial["name"],
            "item {i}: transform-applied state must match"
        );
        assert!(
            !plain["name"]
                .as_str()
                .unwrap()
                .ends_with("_read_transformed"),
            "item {i}: read::one::transform must NOT be applied in either batch mode"
        );
    }
}
