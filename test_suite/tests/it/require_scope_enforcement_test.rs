//! Covers the `REQUIRE_SCOPE` enforcement branch in the generated `get_one`/`get_all`
//! handlers.
//!
//! When a resource is declared with `#[crudcrate(require_scope)]`, the generated
//! handlers must refuse to serve requests that arrive without a `ScopeCondition`
//! extension (ie. the scope middleware is missing or misconfigured). The branch
//! under test is `if REQUIRE_SCOPE && scope.is_none() { return Err(internal(..)) }`,
//! which maps to HTTP 500.
//!
//! Scoped writes confine their rows inside the write transaction.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{Condition, DatabaseConnection, DbErr};
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
    #[crudcrate(generate_router, api_struct = "RseOther", create::one::post = record_creation, create::many::post = record_creations)]
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

    async fn record_creation<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(
        db: &C,
        row: &RseOther,
    ) -> Result<(), crudcrate::ApiError> {
        rse_item::Entity::insert(rse_item::ActiveModel {
            id: sea_orm::Set(Uuid::new_v4()),
            name: sea_orm::Set(row.name.clone()),
        })
        .exec(db)
        .await?;
        Ok(())
    }

    async fn record_creations<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(
        db: &C,
        rows: &[RseOther],
    ) -> Result<(), crudcrate::ApiError> {
        for row in rows {
            record_creation(db, row).await?;
        }
        Ok(())
    }
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(rse_item::Entity, rse_other::Entity).await
}

/// Router for the `require_scope` entity, mounted WITHOUT any scope layer.
fn items_app_unscoped(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/items", rse_item::RseItem::router(db).into())
}

/// Router for the `require_scope` entity, mounted WITH a `ScopeCondition` that matches
/// everything (`Condition::all()` with no predicates is an always-true AND).
fn items_app_scoped(db: &DatabaseConnection) -> axum::Router {
    items_app_unscoped(db).layer(axum::Extension(crudcrate::ScopeCondition {
        condition: Condition::all(),
    }))
}

/// Router for the control entity (no `require_scope`), mounted WITHOUT any scope layer.
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
//     With the extension, writes are confined to its condition.
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

fn items_app_confined(db: &DatabaseConnection) -> axum::Router {
    items_app_unscoped(db).layer(axum::Extension(crudcrate::ScopeCondition::new(
        Condition::all().add(rse_item::Column::Name.starts_with("allowed")),
    )))
}

#[tokio::test]
async fn test_scoped_create_confines_single_and_batch() {
    let db = setup_test_db().await.unwrap();
    for (uri, body, expected) in [
        ("/items", r#"{"name":"allowed one"}"#, StatusCode::CREATED),
        ("/items", r#"{"name":"outside"}"#, StatusCode::FORBIDDEN),
        (
            "/items/batch",
            r#"[{"name":"allowed two"},{"name":"allowed three"}]"#,
            StatusCode::CREATED,
        ),
        (
            "/items/batch",
            r#"[{"name":"allowed rollback"},{"name":"outside"}]"#,
            StatusCode::FORBIDDEN,
        ),
        (
            "/items/batch?partial=true",
            r#"[{"name":"allowed partial"},{"name":"outside"}]"#,
            StatusCode::MULTI_STATUS,
        ),
    ] {
        assert_eq!(
            send(items_app_confined(&db), "POST", uri, body).await,
            expected,
            "{uri}: {body}"
        );
    }
    let rows = rse_item::Entity::find().all(&db).await.unwrap();
    assert_eq!(rows.len(), 4);
    assert!(rows.iter().all(|row| row.name.starts_with("allowed")));
    assert!(!rows.iter().any(|row| row.name == "allowed rollback"));
}

#[tokio::test]
async fn test_scoped_update_confines_existing_and_resulting_rows() {
    let db = setup_test_db().await.unwrap();
    let allowed = seed_item(&db, "allowed original").await;
    let outside = seed_item(&db, "outside").await;
    for (id, name, expected) in [
        (outside, "allowed takeover", StatusCode::NOT_FOUND),
        (allowed, "outside", StatusCode::FORBIDDEN),
        (allowed, "allowed changed", StatusCode::OK),
    ] {
        assert_eq!(
            send(
                items_app_confined(&db),
                "PUT",
                &format!("/items/{id}"),
                &format!(r#"{{"name":"{name}"}}"#)
            )
            .await,
            expected
        );
    }
    assert_eq!(
        rse_item::Entity::find_by_id(outside)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .name,
        "outside"
    );
    assert_eq!(
        rse_item::Entity::find_by_id(allowed)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .name,
        "allowed changed"
    );
}

#[tokio::test]
async fn test_scoped_batch_update_and_delete_roll_back_excluded_rows() {
    let db = setup_test_db().await.unwrap();
    let allowed = seed_item(&db, "allowed original").await;
    let outside = seed_item(&db, "outside").await;
    for suffix in ["", "?partial=true"] {
        let expected = if suffix.is_empty() {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::MULTI_STATUS
        };
        let body = format!(
            r#"[{{"id":"{allowed}","name":"allowed changed"}},{{"id":"{outside}","name":"allowed takeover"}}]"#
        );
        assert_eq!(
            send(
                items_app_confined(&db),
                "PATCH",
                &format!("/items/batch{suffix}"),
                &body
            )
            .await,
            expected
        );
        let row = rse_item::Entity::find_by_id(allowed)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            row.name,
            if suffix.is_empty() {
                "allowed original"
            } else {
                "allowed changed"
            }
        );
    }
    let body = format!(r#"[{{"id":"{allowed}","name":"outside"}}]"#);
    assert_eq!(
        send(items_app_confined(&db), "PATCH", "/items/batch", &body).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            items_app_confined(&db),
            "DELETE",
            &format!("/items/{outside}"),
            ""
        )
        .await,
        StatusCode::NOT_FOUND
    );
    for suffix in ["", "?partial=true"] {
        let expected = if suffix.is_empty() {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::MULTI_STATUS
        };
        assert_eq!(
            send(
                items_app_confined(&db),
                "DELETE",
                &format!("/items/batch{suffix}"),
                &format!(r#"["{allowed}","{outside}"]"#)
            )
            .await,
            expected
        );
        assert_eq!(
            rse_item::Entity::find_by_id(allowed)
                .one(&db)
                .await
                .unwrap()
                .is_some(),
            suffix.is_empty()
        );
    }
    assert!(
        rse_item::Entity::find_by_id(outside)
            .one(&db)
            .await
            .unwrap()
            .is_some()
    );
    let allowed = seed_item(&db, "allowed delete").await;
    assert_eq!(
        send(
            items_app_confined(&db),
            "DELETE",
            &format!("/items/{allowed}"),
            ""
        )
        .await,
        StatusCode::NO_CONTENT
    );
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
                .uri(format!("/items/{}", created.id))
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

// REQUIRE_SCOPE reflects the attribute.
const _: () = {
    assert!(<rse_item::RseItem as CRUDResource>::REQUIRE_SCOPE);
    assert!(!<rse_other::RseOther as CRUDResource>::REQUIRE_SCOPE);
};

#[tokio::test]
async fn test_scoped_create_hooks_share_the_scope_transaction() {
    let db = setup_test_db().await.unwrap();
    let app = others_app(&db).layer(axum::Extension(crudcrate::ScopeCondition::new(
        Condition::all().add(rse_other::Column::Name.starts_with("allowed")),
    )));
    assert_eq!(
        send(app.clone(), "POST", "/others", r#"{"name":"allowed"}"#).await,
        StatusCode::CREATED
    );
    assert_eq!(
        send(app.clone(), "POST", "/others", r#"{"name":"outside"}"#).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            app.clone(),
            "POST",
            "/others/batch",
            r#"[{"name":"allowed rollback"},{"name":"outside"}]"#
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            app,
            "POST",
            "/others/batch?partial=true",
            r#"[{"name":"allowed partial"},{"name":"outside"}]"#
        )
        .await,
        StatusCode::MULTI_STATUS
    );
    for names in [
        rse_item::Entity::find()
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.name)
            .collect::<Vec<_>>(),
        rse_other::Entity::find()
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.name)
            .collect::<Vec<_>>(),
    ] {
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"allowed".to_string()));
        assert!(names.contains(&"allowed partial".to_string()));
    }
}
