//! Scenario: an entity whose only `exclude(scoped)` field is a non-db attribute,
//! so no database column is scoped.
//! Expected behaviour: the router must still wire the generated scoped structs,
//! otherwise the excluded field serialises to scoped callers. The predicate that
//! decides wiring and the one that decides struct generation must agree.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{Condition, DatabaseConnection, DbErr};
use tower::ServiceExt;
use uuid::Uuid;

pub mod snd_item {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "snd_items")]
    #[crudcrate(generate_router, api_struct = "SndItem")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, exclude(scoped, create, update))]
        pub internal_note: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

use snd_item::{SndItem, SndItemCreate};

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(snd_item::Entity).await
}

fn app_unscoped(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/items", SndItem::router(db).into())
}

fn app_scoped(db: &DatabaseConnection) -> axum::Router {
    app_unscoped(db).layer(axum::Extension(crudcrate::ScopeCondition {
        condition: Condition::all(),
    }))
}

async fn get(app: axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let resp = app
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
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    (status, json)
}

async fn create_item(db: &DatabaseConnection, name: &str) -> SndItem {
    SndItem::create(
        db,
        SndItemCreate {
            name: name.to_string(),
        },
    )
    .await
    .expect("direct trait create should succeed")
}

#[tokio::test]
async fn scoped_list_omits_non_db_excluded_field() {
    let db = setup_test_db().await.unwrap();
    create_item(&db, "row").await;

    let (status, json) = get(app_scoped(&db), "/items").await;
    assert_eq!(status, StatusCode::OK);
    let items = json.as_array().expect("list response should be an array");
    assert_eq!(items.len(), 1);
    let keys: Vec<&String> = items[0].as_object().unwrap().keys().collect();
    assert!(
        !items[0].as_object().unwrap().contains_key("internal_note"),
        "exclude(scoped) on a non-db field must strip it from the scoped list, got keys: {keys:?}"
    );
}

#[tokio::test]
async fn scoped_get_one_omits_non_db_excluded_field() {
    let db = setup_test_db().await.unwrap();
    let created = create_item(&db, "row").await;

    let (status, json) = get(app_scoped(&db), &format!("/items/{}", created.id)).await;
    assert_eq!(status, StatusCode::OK);
    let keys: Vec<&String> = json.as_object().unwrap().keys().collect();
    assert!(
        !json.as_object().unwrap().contains_key("internal_note"),
        "exclude(scoped) on a non-db field must strip it from the scoped get_one, got keys: {keys:?}"
    );
}

#[tokio::test]
async fn unscoped_responses_keep_non_db_field() {
    let db = setup_test_db().await.unwrap();
    let created = create_item(&db, "row").await;

    let (status, json) = get(app_unscoped(&db), &format!("/items/{}", created.id)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        json.as_object().unwrap().contains_key("internal_note"),
        "the unscoped response must still carry the non-db field"
    );

    let (status, json) = get(app_unscoped(&db), "/items").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        json.as_array().unwrap()[0]
            .as_object()
            .unwrap()
            .contains_key("internal_note"),
        "the unscoped list must still carry the non-db field"
    );
}
