mod common;

use axum::{body::Body, http::Request};
use chrono::{DateTime, Utc};
use common::Migrator;
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm_migration::{
    MigratorTrait,
    sea_query::{Alias, ColumnDef, Table},
};
use serde_json::json;
use tower::ServiceExt;

// Define an entity using EntityToModels macro with proper type detection
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "products")]
#[crudcrate(
    api_struct = "Products", 
    description = "Product management for type-based filtering tests",
    generate_router,
    enum_case_sensitive = true
    // Note: Using case-sensitive enum matching for this test
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

    // Enum field - should use case-sensitive exact matching
    #[crudcrate(filterable, enum_field)]
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
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());

    let db = common::setup_test_db()
        .await
        .expect("Failed to setup test database");

    // Run base migrations first
    Migrator::up(&db, None)
        .await
        .expect("Failed to run migrations");

    // For PostgreSQL, we need to handle enum types properly
    if database_url.starts_with("postgres") {
        // Drop and recreate the enum type to ensure it's clean
        let _ = db
            .execute_unprepared("DROP TYPE IF EXISTS product_category CASCADE")
            .await;
        db.execute_unprepared(
            "CREATE TYPE product_category AS ENUM ('Electronics', 'Books', 'Clothing')",
        )
        .await
        .expect("Failed to create PostgreSQL enum type");
    }

    // Drop existing table to ensure clean state
    let drop_table = Table::drop()
        .table(Alias::new("products"))
        .if_exists()
        .to_owned();
    let drop_statement = db.get_database_backend().build(&drop_table);
    let _ = db.execute(drop_statement).await;

    let mut create_table = Table::create();
    create_table
        .table(Alias::new("products"))
        .if_not_exists()
        .col(
            ColumnDef::new(Alias::new("id"))
                .uuid()
                .not_null()
                .primary_key(),
        )
        .col(ColumnDef::new(Alias::new("name")).text().not_null())
        .col(ColumnDef::new(Alias::new("description")).text())
        .col(ColumnDef::new(Alias::new("price")).integer().not_null())
        .col(ColumnDef::new(Alias::new("in_stock")).boolean().not_null());

    // Handle category column differently for each database
    if database_url.starts_with("postgres") {
        // For PostgreSQL, use custom enum type
        create_table.col(
            ColumnDef::new(Alias::new("category"))
                .custom(Alias::new("product_category"))
                .not_null(),
        );
    } else {
        // For other databases, use text (MySQL and SQLite handle enums as strings)
        create_table.col(ColumnDef::new(Alias::new("category")).text().not_null());
    }

    create_table.col(
        ColumnDef::new(Alias::new("created_at"))
            .timestamp_with_time_zone()
            .not_null(),
    );

    let create_table = create_table.clone();
    let statement = db.get_database_backend().build(&create_table);
    db.execute(statement)
        .await
        .expect("Failed to create products table");

    axum::Router::new().nest("/products", router(&db).into())
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

        let status = response.status();
        if status != axum::http::StatusCode::CREATED {
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let error_msg = String::from_utf8_lossy(&body);
            panic!(
                "Failed to create product '{}'. Status: {}, Body: {}",
                product_data["name"], status, error_msg
            );
        }
    }
}

#[tokio::test]
async fn test_string_field_uses_like_by_default() {
    let app = setup_test_app_with_products().await;
    create_test_products(&app).await;

    // Test default LIKE behavior for string fields
    // Should find "Wireless Mouse" when searching for "Mouse"
    let filter = url_escape::encode_component(r#"{"name":"Mouse"}"#);
    let request = Request::builder()
        .method("GET")
        .uri(format!("/products?filter={filter}"))
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
async fn test_string_field_description_uses_like_by_default() {
    let app = setup_test_app_with_products().await;
    create_test_products(&app).await;

    // Test default LIKE behavior for string fields
    // Should find products with "wireless" in description
    let filter = url_escape::encode_component(r#"{"description":"wireless"}"#);
    let request = Request::builder()
        .method("GET")
        .uri(format!("/products?filter={filter}"))
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
        .uri(format!("/products?filter={filter}"))
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
        .uri(format!("/products?filter={filter}"))
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
    assert!(!products[0].in_stock);
}

#[tokio::test]
async fn test_enum_field_exact_matching() {
    let app = setup_test_app_with_products().await;
    create_test_products(&app).await;

    // Test enum field filtering (now always case-insensitive)
    // Case sensitivity has been removed from crudcrate
    let filter = url_escape::encode_component(r#"{"category":"Electronics"}"#);
    let request = Request::builder()
        .method("GET")
        .uri(format!("/products?filter={filter}"))
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

    // Verify that lowercase DOES match (case-insensitive after removing case sensitivity)
    let filter = url_escape::encode_component(r#"{"category":"electronics"}"#);
    let request = Request::builder()
        .method("GET")
        .uri(format!("/products?filter={filter}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let products: Vec<Products> = serde_json::from_slice(&body).unwrap();

    // Should find 2 products because "electronics" matches "Electronics" (case-insensitive)
    assert_eq!(products.len(), 2);
}

#[tokio::test]
async fn test_string_exact_matching_with_eq_suffix() {
    let app = setup_test_app_with_products().await;
    create_test_products(&app).await;

    // Test that _eq suffix forces exact matching even for string fields
    let filter = url_escape::encode_component(r#"{"name_eq":"Mouse"}"#);
    let request = Request::builder()
        .method("GET")
        .uri(format!("/products?filter={filter}"))
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
