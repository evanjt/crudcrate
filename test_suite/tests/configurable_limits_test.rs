// Tests for configurable limits feature
// Verifies that batch_limit and max_page_size can be configured via struct attributes

use axum::body::Body;
use axum::http::Request;
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use tower::ServiceExt;
use uuid::Uuid;

// Define a model with custom batch_limit
pub mod limited_item {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "limited_items")]
    #[crudcrate(
        generate_router,
        api_struct = "LimitedItem",
        batch_limit = 5,
        max_page_size = 50
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[crudcrate(exclude(create, update), on_create = chrono::Utc::now())]
        pub created_at: chrono::DateTime<chrono::Utc>,

        #[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
        pub updated_at: chrono::DateTime<chrono::Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// Define a model with default limits (for comparison)
pub mod default_item {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "default_items")]
    #[crudcrate(generate_router, api_struct = "DefaultItem")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[crudcrate(exclude(create, update), on_create = chrono::Utc::now())]
        pub created_at: chrono::DateTime<chrono::Utc>,

        #[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
        pub updated_at: chrono::DateTime<chrono::Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let db = Database::connect("sqlite::memory:").await?;

    // Create tables
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    db.execute(backend.build(&schema.create_table_from_entity(limited_item::Entity)))
        .await?;
    db.execute(backend.build(&schema.create_table_from_entity(default_item::Entity)))
        .await?;

    Ok(db)
}

#[tokio::test]
async fn test_custom_batch_limit_is_set() {
    // Verify that batch_limit() is correctly set to 5
    assert_eq!(limited_item::LimitedItem::batch_limit(), 5);
    assert_eq!(limited_item::LimitedItem::max_page_size(), 50);
}

#[tokio::test]
async fn test_default_batch_limit() {
    // Verify that the default batch_limit() is 100
    assert_eq!(default_item::DefaultItem::batch_limit(), 100);
    assert_eq!(default_item::DefaultItem::max_page_size(), 1000);
}

#[tokio::test]
async fn test_batch_create_within_limit_succeeds() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");

    // Create 5 items (at the limit)
    let items: Vec<limited_item::LimitedItemCreate> = (0..5)
        .map(|i| limited_item::LimitedItemCreate {
            name: format!("Item {}", i),
        })
        .collect();

    let result = limited_item::LimitedItem::create_many(&db, items).await;
    assert!(result.is_ok(), "Creating 5 items should succeed");
    assert_eq!(result.unwrap().len(), 5);
}

#[tokio::test]
async fn test_batch_create_exceeds_limit_fails() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");

    // Create 6 items (exceeds limit of 5)
    let items: Vec<limited_item::LimitedItemCreate> = (0..6)
        .map(|i| limited_item::LimitedItemCreate {
            name: format!("Item {}", i),
        })
        .collect();

    let result = limited_item::LimitedItem::create_many(&db, items).await;
    assert!(result.is_err(), "Creating 6 items should fail");

    let error = result.unwrap_err();
    let error_message = format!("{:?}", error);
    assert!(
        error_message.contains("Batch create limited to 5 items"),
        "Error should mention the batch limit: {}",
        error_message
    );
}

#[tokio::test]
async fn test_default_model_allows_more_items() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");

    // Create 50 items (well under default limit of 100)
    let items: Vec<default_item::DefaultItemCreate> = (0..50)
        .map(|i| default_item::DefaultItemCreate {
            name: format!("Default Item {}", i),
        })
        .collect();

    let result = default_item::DefaultItem::create_many(&db, items).await;
    assert!(
        result.is_ok(),
        "Creating 50 items should succeed with default limit"
    );
    assert_eq!(result.unwrap().len(), 50);
}

#[tokio::test]
async fn test_batch_update_within_limit_succeeds() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");

    // First create 5 items
    let items: Vec<limited_item::LimitedItemCreate> = (0..5)
        .map(|i| limited_item::LimitedItemCreate {
            name: format!("Item {}", i),
        })
        .collect();

    let created = limited_item::LimitedItem::create_many(&db, items)
        .await
        .expect("Failed to create items");

    // Update all 5 items (at the limit)
    let updates: Vec<(Uuid, limited_item::LimitedItemUpdate)> = created
        .iter()
        .map(|item| {
            (
                item.id,
                limited_item::LimitedItemUpdate {
                    name: Some(Some(format!("Updated {}", item.name))),
                },
            )
        })
        .collect();

    let result = limited_item::LimitedItem::update_many(&db, updates).await;
    assert!(result.is_ok(), "Updating 5 items should succeed");
}

#[tokio::test]
async fn test_batch_update_exceeds_limit_fails() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");

    // Create fake UUIDs for the update (we don't need real items for this test)
    let updates: Vec<(Uuid, limited_item::LimitedItemUpdate)> = (0..6)
        .map(|_| {
            (
                Uuid::new_v4(),
                limited_item::LimitedItemUpdate {
                    name: Some(Some("Updated".to_string())),
                },
            )
        })
        .collect();

    let result = limited_item::LimitedItem::update_many(&db, updates).await;
    assert!(result.is_err(), "Updating 6 items should fail");

    let error = result.unwrap_err();
    let error_message = format!("{:?}", error);
    assert!(
        error_message.contains("Batch update limited to 5 items"),
        "Error should mention the batch limit: {}",
        error_message
    );
}

#[tokio::test]
async fn test_max_page_size_enforced_at_handler_level() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");

    // Create 60 items (more than max_page_size=50 for LimitedItem)
    let items: Vec<limited_item::LimitedItemCreate> = (0..60)
        .map(|i| limited_item::LimitedItemCreate {
            name: format!("Page Size Test Item {}", i),
        })
        .collect();

    // Insert in batches of 5 (batch_limit=5)
    for chunk in items.chunks(5) {
        limited_item::LimitedItem::create_many(&db, chunk.to_vec())
            .await
            .expect("Failed to create items");
    }

    // Set up router
    let app: axum::Router = axum::Router::new().nest(
        "/limited_items",
        limited_item::LimitedItem::router(&db).into(),
    );

    // Request with per_page=1000 (exceeds max_page_size=50)
    let request = Request::builder()
        .method("GET")
        .uri("/limited_items?page=1&per_page=1000")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(
        response.status(),
        axum::http::StatusCode::OK,
        "GET request should succeed"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let items: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    assert!(
        items.len() <= 50,
        "max_page_size=50 should be enforced at handler level, but got {} items",
        items.len()
    );
}
