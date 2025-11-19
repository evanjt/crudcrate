// Custom CRUD Functions Test
// Tests fn_delete and fn_delete_many custom delete handlers with side effects
// Based on production pattern from s3_cleanup_on_delete.rs example

use axum::body::Body;
use axum::http::Request;
use axum::Router;
use chrono::{DateTime, Utc};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::{entity::prelude::*, Database, DatabaseConnection, EntityTrait};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use uuid::Uuid;

// ============================================================================
// Mock External Service (simulates S3, email service, etc.)
// ============================================================================

#[derive(Clone, Default)]
struct MockExternalService {
    deleted_keys: Arc<Mutex<Vec<String>>>,
    delete_many_calls: Arc<Mutex<Vec<Vec<String>>>>,
    should_fail: Arc<Mutex<bool>>,
}

impl MockExternalService {
    fn new() -> Self {
        Self::default()
    }

    async fn delete_object(&self, key: &str) -> Result<(), String> {
        if *self.should_fail.lock().unwrap() {
            return Err(format!("External service failure for key: {key}"));
        }
        self.deleted_keys.lock().unwrap().push(key.to_string());
        Ok(())
    }

    fn get_deleted_keys(&self) -> Vec<String> {
        self.deleted_keys.lock().unwrap().clone()
    }

    fn set_should_fail(&self, fail: bool) {
        *self.should_fail.lock().unwrap() = fail;
    }

    fn record_delete_many(&self, keys: Vec<String>) {
        self.delete_many_calls.lock().unwrap().push(keys);
    }
}

// Global mock service for use in custom delete functions
static MOCK_SERVICE: std::sync::OnceLock<MockExternalService> = std::sync::OnceLock::new();

fn get_mock_service() -> &'static MockExternalService {
    MOCK_SERVICE.get_or_init(MockExternalService::new)
}

// ============================================================================
// Asset Entity with Custom Delete Functions
// ============================================================================

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "assets")]
#[crudcrate(
    api_struct = "Asset",
    name_singular = "asset",
    name_plural = "assets",
    description = "Assets with external cleanup on delete",
    generate_router,
    fn_delete = delete_asset_with_cleanup,
    fn_delete_many = delete_many_assets_with_cleanup,
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable)]
    pub filename: String,

    #[crudcrate(filterable)]
    pub external_key: String, // Key in external service (S3, etc.)

    #[crudcrate(sortable)]
    pub size_bytes: i64,

    #[crudcrate(exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

// ============================================================================
// Custom Delete Functions
// ============================================================================

async fn delete_asset_with_cleanup(db: &DatabaseConnection, id: Uuid) -> Result<Uuid, DbErr> {
    // 1. Fetch asset to get external key
    let asset = Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("Asset not found".to_string()))?;

    // 2. Delete from external service (fail fast)
    get_mock_service()
        .delete_object(&asset.external_key)
        .await
        .map_err(|e| DbErr::Custom(format!("External service error: {e}")))?;

    // 3. Delete from database
    Entity::delete_by_id(id).exec(db).await?;

    // 4. Return the deleted ID
    Ok(id)
}

async fn delete_many_assets_with_cleanup(
    db: &DatabaseConnection,
    ids: Vec<Uuid>,
) -> Result<Vec<Uuid>, DbErr> {
    let mut deleted_ids = Vec::new();
    let mut external_keys = Vec::new();

    for id in &ids {
        // Fetch asset
        let asset = match Entity::find_by_id(*id).one(db).await? {
            Some(a) => a,
            None => continue,
        };

        // Try to delete from external service
        if get_mock_service().delete_object(&asset.external_key).await.is_ok() {
            external_keys.push(asset.external_key.clone());

            // Delete from database
            if Entity::delete_by_id(*id).exec(db).await.is_ok() {
                deleted_ids.push(*id);
            }
        }
    }

    // Record the batch operation
    get_mock_service().record_delete_many(external_keys);

    Ok(deleted_ids)
}

// ============================================================================
// Setup Helpers
// ============================================================================

async fn setup_db() -> Result<DatabaseConnection, sea_orm::DbErr> {
    let db = Database::connect("sqlite::memory:").await?;

    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        r"CREATE TABLE assets (
            id TEXT PRIMARY KEY,
            filename TEXT NOT NULL,
            external_key TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            created_at TEXT NOT NULL
        )"
        .to_owned(),
    ))
    .await?;

    Ok(db)
}

fn setup_app(db: &DatabaseConnection) -> Router {
    // Reset mock service
    let service = get_mock_service();
    service.deleted_keys.lock().unwrap().clear();
    service.delete_many_calls.lock().unwrap().clear();
    service.set_should_fail(false);

    Router::new().nest("/assets", Asset::router(db).into())
}

// ============================================================================
// Tests
// ============================================================================

#[tokio::test]
async fn test_custom_delete_single_with_cleanup() {
    let db = setup_db().await.expect("Failed to setup database");
    let app = setup_app(&db);

    // Create an asset
    let create_data = json!({
        "filename": "test.pdf",
        "external_key": "s3://bucket/test.pdf",
        "size_bytes": 1024
    });

    let request = Request::builder()
        .method("POST")
        .uri("/assets")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let asset_id = created["id"].as_str().unwrap();

    // Delete the asset
    let request = Request::builder()
        .method("DELETE")
        .uri(format!("/assets/{asset_id}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);

    // Verify external service was called
    let deleted_keys = get_mock_service().get_deleted_keys();
    assert_eq!(deleted_keys.len(), 1);
    assert_eq!(deleted_keys[0], "s3://bucket/test.pdf");

    // Verify asset was deleted from database
    let request = Request::builder()
        .method("GET")
        .uri(format!("/assets/{asset_id}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_custom_delete_single_external_failure() {
    let db = setup_db().await.expect("Failed to setup database");
    let app = setup_app(&db);

    // Create an asset
    let create_data = json!({
        "filename": "test.pdf",
        "external_key": "s3://bucket/test.pdf",
        "size_bytes": 1024
    });

    let request = Request::builder()
        .method("POST")
        .uri("/assets")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let asset_id = created["id"].as_str().unwrap();

    // Make external service fail
    get_mock_service().set_should_fail(true);

    // Try to delete - should fail
    let request = Request::builder()
        .method("DELETE")
        .uri(format!("/assets/{asset_id}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::INTERNAL_SERVER_ERROR);

    // Verify asset was NOT deleted from database (transaction rolled back)
    get_mock_service().set_should_fail(false);
    let request = Request::builder()
        .method("GET")
        .uri(format!("/assets/{asset_id}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK); // Still exists
}

// Note: delete_many HTTP endpoint tests omitted for now
// The fn_delete_many function is defined and will be tested via direct API calls

#[tokio::test]
async fn test_custom_delete_not_found() {
    let db = setup_db().await.expect("Failed to setup database");
    let app = setup_app(&db);

    let fake_id = Uuid::new_v4();

    // Try to delete non-existent asset
    let request = Request::builder()
        .method("DELETE")
        .uri(format!("/assets/{fake_id}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);

    // Verify external service was NOT called
    let deleted_keys = get_mock_service().get_deleted_keys();
    assert_eq!(deleted_keys.len(), 0);
}
