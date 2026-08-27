// List model serde attribute test
// Field-level serde attributes shape the wire format, so the list response must
// serialize a field the same way the single-record response does.

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::sea_query::Table;
use sea_orm::{Database, DatabaseConnection, Schema, entity::prelude::*};
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

#[derive(
    Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize, EntityToModels,
)]
#[sea_orm(table_name = "contacts")]
#[crudcrate(
    api_struct = "Contact",
    name_singular = "contact",
    name_plural = "contacts",
    generate_router
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(sortable, filterable)]
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,

    #[serde(rename = "phoneNumber")]
    pub phone_number: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

async fn setup_db() -> Result<DatabaseConnection, sea_orm::DbErr> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let db = Database::connect(&url).await?;
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);
    db.execute(&Table::drop().table(Entity).if_exists().to_owned())
        .await?;
    db.execute(&schema.create_table_from_entity(Entity)).await?;
    Ok(db)
}

async fn create_contact(app: &Router) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/contacts")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "name": "Ada", "nickname": null, "phone_number": null }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    created["id"].as_str().unwrap().to_string()
}

async fn get_json(app: &Router, uri: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn test_skip_serializing_if_applies_to_list_entries() {
    let db = setup_db().await.expect("database setup");
    let app = Router::new().nest("/contacts", Contact::router(&db).into());
    let id = create_contact(&app).await;

    let one = get_json(&app, &format!("/contacts/{id}")).await;
    assert!(one.get("nickname").is_none());

    let list = get_json(&app, "/contacts").await;
    let entry = &list.as_array().unwrap()[0];
    assert!(
        entry.get("nickname").is_none(),
        "list entry emitted a null for a field skipped when none: {entry}"
    );
}

#[tokio::test]
async fn test_rename_applies_to_list_entries() {
    let db = setup_db().await.expect("database setup");
    let app = Router::new().nest("/contacts", Contact::router(&db).into());
    let id = create_contact(&app).await;

    let one = get_json(&app, &format!("/contacts/{id}")).await;
    assert!(one.get("phoneNumber").is_some());

    let list = get_json(&app, "/contacts").await;
    let entry = &list.as_array().unwrap()[0];
    assert!(entry.get("phoneNumber").is_some(), "renamed key missing");
    assert!(entry.get("phone_number").is_none());
}
