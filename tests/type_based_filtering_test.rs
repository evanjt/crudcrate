mod common;

use axum::body::Body;
use axum::http::Request;
use chrono::{DateTime, Utc};
use common::{Migrator, setup_test_db};
use crudcrate::{EntityToModels, CRUDResource};
use sea_orm::entity::prelude::*;
use sea_orm_migration::MigratorTrait;
use serde_json::json;
use tower::ServiceExt;

// Define an entity using EntityToModels macro with proper type detection
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "products")]
#[crudcrate(
    api_struct = "Products",
    description = "Product management for type-based filtering tests",
    generate_router
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, sortable, create_model = false, update_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,

    // Text field - should use substring matching
    #[crudcrate(sortable, filterable)]
    pub name: String,

    // Another text field - should use substring matching
    #[crudcrate(filterable)]
    pub description: String,

    // Numeric field - should use exact matching
    #[crudcrate(filterable)]
    pub price: i32,

    // Boolean field - should use exact matching
    #[crudcrate(filterable)]
    pub in_stock: bool,

    // Enum field - should use case-insensitive exact matching
    #[crudcrate(filterable)]
    pub category: ProductCategory,

    #[crudcrate(sortable, create_model = false, update_model = false, on_create = chrono::Utc::now())]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    utoipa::ToSchema,
    EnumIter,
    DeriveActiveEnum,
)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "product_category")]
pub enum ProductCategory {
    #[sea_orm(string_value = "Electronics")]
    Electronics,
    #[sea_orm(string_value = "Books")]
    Books,
    #[sea_orm(string_value = "Clothing")]
    Clothing,
}

async fn setup_test_app_with_products() -> axum::Router {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");

    Migrator::up(&db, None)
        .await
        .expect("Failed to run migrations");

    // Create the products table
    let create_table_stmt = sea_orm::Statement::from_string(
        db.get_database_backend(),
        r#"
        CREATE TABLE IF NOT EXISTS products (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            price INTEGER NOT NULL,
            in_stock BOOLEAN NOT NULL,
            category TEXT NOT NULL,
            created_at TEXT NOT NULL
        )
        "#
        .to_owned(),
    );

    db.execute(create_table_stmt)
        .await
        .expect("Failed to create products table");

    router(&db).into()
}

async fn create_test_products(app: &axum::Router) {
    let products = vec![
        json!({
            "name": "Wireless Mouse",
            "description": "High-quality wireless computer mouse",
            "price": 2500,
            "in_stock": true,
            "category": "Electronics"
        }),
        json!({
            "name": "Programming Book",
            "description": "Learn Rust programming language",
            "price": 4500,
            "in_stock": true,
            "category": "Books"
        }),
        json!({
            "name": "T-Shirt",
            "description": "Comfortable cotton t-shirt",
            "price": 1500,
            "in_stock": false,
            "category": "Clothing"
        }),
        json!({
            "name": "Bluetooth Headphones",
            "description": "Noise-cancelling wireless headphones",
            "price": 15000,
            "in_stock": true,
            "category": "Electronics"
        }),
    ];

    for product_data in products {
        let request = Request::builder()
            .method("POST")
            .uri("/products")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&product_data).unwrap()))
            .unwrap();

        let app_clone = app.clone();
        let response = app_clone.oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            axum::http::StatusCode::CREATED,
            "Failed to create product"
        );
    }
}

#[tokio::test]
async fn test_string_field_uses_substring_matching() {
    let app = setup_test_app_with_products().await;
    create_test_products(&app).await;

    // Test substring matching for 'name' field (String type)
    // Should find "Wireless Mouse" when searching for "Mouse"
    let filter = url_escape::encode_component(r#"{"name":"Mouse"}"#);
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/products?filter={}", filter))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let products: Vec<Products> = serde_json::from_slice(&body).unwrap();

    assert_eq!(products.len(), 1);
    assert_eq!(products[0].name, "Wireless Mouse");
}

#[tokio::test]
async fn test_string_field_description_uses_substring_matching() {
    let app = setup_test_app_with_products().await;
    create_test_products(&app).await;

    // Test substring matching for 'description' field (String type)
    // Should find products with "wireless" in description
    let filter = url_escape::encode_component(r#"{"description":"wireless"}"#);
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/products?filter={}", filter))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let products: Vec<Products> = serde_json::from_slice(&body).unwrap();

    assert_eq!(products.len(), 2); // "Wireless Mouse" and "Bluetooth Headphones"
}

#[tokio::test]
async fn test_numeric_field_uses_exact_matching() {
    let app = setup_test_app_with_products().await;
    create_test_products(&app).await;

    // Test exact matching for 'price' field (i32 type)
    let filter = url_escape::encode_component(r#"{"price":2500}"#);
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/products?filter={}", filter))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let products: Vec<Products> = serde_json::from_slice(&body).unwrap();

    assert_eq!(products.len(), 1);
    assert_eq!(products[0].name, "Wireless Mouse");
    assert_eq!(products[0].price, 2500);
}

#[tokio::test]
async fn test_boolean_field_uses_exact_matching() {
    let app = setup_test_app_with_products().await;
    create_test_products(&app).await;

    // Test exact matching for 'in_stock' field (bool type)
    let filter = url_escape::encode_component(r#"{"in_stock":false}"#);
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/products?filter={}", filter))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let products: Vec<Products> = serde_json::from_slice(&body).unwrap();

    assert_eq!(products.len(), 1);
    assert_eq!(products[0].name, "T-Shirt");
    assert_eq!(products[0].in_stock, false);
}

#[tokio::test]
async fn test_enum_field_uses_case_insensitive_exact_matching() {
    let app = setup_test_app_with_products().await;
    create_test_products(&app).await;

    // Test case-insensitive exact matching for 'category' field (enum type)
    // Should match "Electronics" when searching for "electronics"
    let filter = url_escape::encode_component(r#"{"category":"electronics"}"#);
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/products?filter={}", filter))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let products: Vec<Products> = serde_json::from_slice(&body).unwrap();

    assert_eq!(products.len(), 2); // "Wireless Mouse" and "Bluetooth Headphones"
    assert!(
        products
            .iter()
            .all(|p| matches!(p.category, ProductCategory::Electronics))
    );
}

#[tokio::test]
async fn test_string_exact_matching_with_eq_suffix() {
    let app = setup_test_app_with_products().await;
    create_test_products(&app).await;

    // Test that _eq suffix forces exact matching even for string fields
    let filter = url_escape::encode_component(r#"{"name_eq":"Mouse"}"#);
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/products?filter={}", filter))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let products: Vec<Products> = serde_json::from_slice(&body).unwrap();

    // Should find no matches because no product has name exactly equal to "Mouse"
    assert_eq!(products.len(), 0);
}
