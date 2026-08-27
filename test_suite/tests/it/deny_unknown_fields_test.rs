//! Excluded fields in request bodies are ignored by default and rejected with `deny_unknown_fields`.

// deny_unknown_fields test
// Without the opt-in, a payload key the input model does not accept is ignored.
// With `#[crudcrate(deny_unknown_fields)]`, it is rejected instead of dropped.

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use sea_orm::{DatabaseConnection, entity::prelude::*};
use serde_json::json;
use tower::ServiceExt;

mod strict {
    use crudcrate::{CRUDResource, EntityToModels};
    use sea_orm::entity::prelude::*;
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "strict_articles")]
    #[crudcrate(
        api_struct = "StrictArticle",
        name_singular = "strict_article",
        name_plural = "strict_articles",
        generate_router,
        deny_unknown_fields
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(sortable, filterable)]
        pub title: String,

        #[crudcrate(exclude(create))]
        pub published: Option<bool>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

mod lenient {
    use crudcrate::{CRUDResource, EntityToModels};
    use sea_orm::entity::prelude::*;
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "lenient_articles")]
    #[crudcrate(
        api_struct = "LenientArticle",
        name_singular = "lenient_article",
        name_plural = "lenient_articles",
        generate_router
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(sortable, filterable)]
        pub title: String,

        #[crudcrate(exclude(create))]
        pub published: Option<bool>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

use lenient::LenientArticle;
use strict::StrictArticle;

async fn setup_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(strict::Entity, lenient::Entity).await
}

async fn post(app: &Router, uri: &str, payload: serde_json::Value) -> (u16, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, body.to_vec())
}

#[tokio::test]
async fn test_excluded_field_is_rejected_when_opted_in() {
    let db = setup_db().await.expect("database setup");
    let app = Router::new().nest("/strict", StrictArticle::router(&db).into());

    let (status, _) = post(
        &app,
        "/strict",
        json!({ "title": "Ada", "published": true }),
    )
    .await;
    assert_eq!(status, 422, "a field the create model excludes is rejected");

    let (status, _) = post(&app, "/strict", json!({ "title": "Ada" })).await;
    assert_eq!(status, 201);
}

#[tokio::test]
async fn test_excluded_field_is_ignored_by_default() {
    let db = setup_db().await.expect("database setup");
    let app = Router::new().nest("/lenient", LenientArticle::router(&db).into());

    let (status, body) = post(
        &app,
        "/lenient",
        json!({ "title": "Ada", "published": true }),
    )
    .await;
    assert_eq!(status, 201);
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(created["published"], serde_json::Value::Null);
}

#[tokio::test]
async fn test_update_rejects_excluded_field_when_opted_in() {
    let db = setup_db().await.expect("database setup");
    let app = Router::new().nest("/strict", StrictArticle::router(&db).into());

    let (_, body) = post(&app, "/strict", json!({ "title": "Ada" })).await;
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = created["id"].as_str().unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/strict/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "id": id, "title": "Ada Lovelace" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 422);
}
