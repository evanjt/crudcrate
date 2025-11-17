// List Model Optimization Test
// Tests list_model=false attribute for selective field visibility in list vs detail views

use axum::body::Body;
use axum::http::Request;
use axum::Router;
use chrono::{DateTime, Utc};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::{entity::prelude::*, Database, DatabaseConnection};
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

// ============================================================================
// Test Entity: Product with list_model=false on expensive fields
// ============================================================================

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "products")]
#[crudcrate(
    api_struct = "Product",
    name_singular = "product",
    name_plural = "products",
    description = "Products with optimized list view",
    generate_router
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    // ✅ Show in list view (essential browsing info)
    #[crudcrate(sortable, filterable, fulltext)]
    pub name: String,

    #[crudcrate(sortable, filterable)]
    pub price: f64,

    // ❌ Hide from list view - only show in detail view
    #[sea_orm(column_type = "Text")]
    #[crudcrate(filterable, fulltext, exclude(list))]
    pub description: Option<String>,

    #[sea_orm(column_type = "Text")]
    #[crudcrate(exclude(list))]
    pub specifications: Option<String>,

    #[crudcrate(sortable, filterable, exclude(list))]
    pub weight_kg: Option<f64>,

    #[crudcrate(sortable, filterable, exclude(list))]
    pub dimensions: Option<String>,

    // ✅ Show timestamps in list (useful for sorting)
    #[crudcrate(exclude(create, update), on_create = Utc::now(), sortable)]
    pub created_at: DateTime<Utc>,

    // ❌ Hide updated_at from list (detail-only)
    #[crudcrate(
        exclude(create, update, list),
        on_create = Utc::now(),
        on_update = Utc::now(),
        sortable
    )]
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

// ============================================================================
// Setup Helpers
// ============================================================================

async fn setup_products_db() -> Result<DatabaseConnection, sea_orm::DbErr> {
    let db = Database::connect("sqlite::memory:").await?;

    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        r#"CREATE TABLE products (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            price REAL NOT NULL,
            description TEXT,
            specifications TEXT,
            weight_kg REAL,
            dimensions TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )"#
        .to_owned(),
    ))
    .await?;

    Ok(db)
}

fn setup_products_app(db: &DatabaseConnection) -> Router {
    Router::new().nest("/products", Product::router(db).into())
}

// ============================================================================
// Tests
// ============================================================================

#[tokio::test]
async fn test_list_model_excludes_expensive_fields() {
    // Verify that fields marked list_model=false are NOT in ProductList
    let db = setup_products_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_products_app(&db);

    // Create a product with full data
    let create_data = json!({
        "name": "Premium Widget",
        "price": 99.99,
        "description": "This is a very long detailed description that would make list responses huge".repeat(10),
        "specifications": "Detailed technical specs...".repeat(20),
        "weight_kg": 2.5,
        "dimensions": "10x20x30cm"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/products")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // Get list view
    let request = Request::builder()
        .method("GET")
        .uri("/products")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let items: Vec<serde_json::Value> =
        serde_json::from_slice(&body).expect("Failed to parse list response");

    assert_eq!(items.len(), 1);

    let product_in_list = &items[0];

    // ✅ List model SHOULD include: id, name, price, created_at
    assert!(product_in_list.get("id").is_some());
    assert_eq!(product_in_list["name"], "Premium Widget");
    assert_eq!(product_in_list["price"], 99.99);
    assert!(product_in_list.get("created_at").is_some());

    // ❌ List model SHOULD NOT include: description, specifications, weight_kg, dimensions, updated_at
    assert!(product_in_list.get("description").is_none());
    assert!(product_in_list.get("specifications").is_none());
    assert!(product_in_list.get("weight_kg").is_none());
    assert!(product_in_list.get("dimensions").is_none());
    assert!(product_in_list.get("updated_at").is_none());
}

#[tokio::test]
async fn test_detail_view_includes_all_fields() {
    // Verify that get_one returns ALL fields, including list_model=false ones
    let db = setup_products_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_products_app(&db);

    // Create a product
    let create_data = json!({
        "name": "Deluxe Gadget",
        "price": 199.99,
        "description": "Comprehensive product description",
        "specifications": "Technical specifications",
        "weight_kg": 1.5,
        "dimensions": "15x25x35cm"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/products")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let product_id = created["id"].as_str().unwrap();

    // Get detail view
    let request = Request::builder()
        .method("GET")
        .uri(format!("/products/{product_id}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let detail: serde_json::Value =
        serde_json::from_slice(&body).expect("Failed to parse detail response");

    // ✅ Detail view SHOULD include ALL fields
    assert!(detail.get("id").is_some());
    assert_eq!(detail["name"], "Deluxe Gadget");
    assert_eq!(detail["price"], 199.99);
    assert_eq!(detail["description"], "Comprehensive product description");
    assert_eq!(detail["specifications"], "Technical specifications");
    assert_eq!(detail["weight_kg"], 1.5);
    assert_eq!(detail["dimensions"], "15x25x35cm");
    assert!(detail.get("created_at").is_some());
    assert!(detail.get("updated_at").is_some());
}

#[tokio::test]
async fn test_list_optimization_reduces_payload_size() {
    // Demonstrate payload size reduction from hiding expensive fields
    let db = setup_products_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_products_app(&db);

    // Create product with large fields
    let large_description = "Long description ".repeat(100); // ~1.7KB
    let large_specs = "Technical details ".repeat(150); // ~2.5KB

    let create_data = json!({
        "name": "Heavy Data Product",
        "price": 49.99,
        "description": large_description,
        "specifications": large_specs,
        "weight_kg": 10.0,
        "dimensions": "100x200x300cm"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/products")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let detail_size = body_bytes.len();

    // Get list view
    let request = Request::builder()
        .method("GET")
        .uri("/products")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let list_size = body_bytes.len();

    // List payload should be significantly smaller than detail payload
    // Since we're hiding ~4KB of data (description + specifications)
    println!("Detail view size: {} bytes", detail_size);
    println!("List view size: {} bytes", list_size);
    println!(
        "Size reduction: {:.1}%",
        (1.0 - (list_size as f64 / detail_size as f64)) * 100.0
    );

    // List should be at least 50% smaller (conservative check)
    assert!(
        list_size < detail_size / 2,
        "List payload should be significantly smaller than detail payload"
    );
}

#[tokio::test]
async fn test_list_model_with_optional_fields() {
    // Verify list_model=false works correctly with Option<T> fields
    let db = setup_products_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_products_app(&db);

    // Create product with minimal data (no optional fields)
    let create_data = json!({
        "name": "Basic Product",
        "price": 9.99
    });

    let request = Request::builder()
        .method("POST")
        .uri("/products")
        .header("content-type", "application/json")
        .body(Body::from(create_data.to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // Get list view
    let request = Request::builder()
        .method("GET")
        .uri("/products")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let items: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    let product = &items[0];

    // Should have basic fields
    assert_eq!(product["name"], "Basic Product");
    assert_eq!(product["price"], 9.99);

    // Should NOT have optional fields marked list_model=false
    assert!(product.get("description").is_none());
    assert!(product.get("specifications").is_none());
}

#[tokio::test]
async fn test_list_model_multiple_items() {
    // Verify list_model=false works correctly when returning multiple items
    let db = setup_products_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_products_app(&db);

    // Create multiple products
    for i in 1..=5 {
        let create_data = json!({
            "name": format!("Product {}", i),
            "price": i as f64 * 10.0,
            "description": format!("Description for product {}", i).repeat(20),
            "specifications": format!("Specs for product {}", i).repeat(15),
            "weight_kg": i as f64,
            "dimensions": format!("{}x{}x{}cm", i, i*2, i*3)
        });

        let request = Request::builder()
            .method("POST")
            .uri("/products")
            .header("content-type", "application/json")
            .body(Body::from(create_data.to_string()))
            .unwrap();

        app.clone().oneshot(request).await.unwrap();
    }

    // Get all products
    let request = Request::builder()
        .method("GET")
        .uri("/products")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let items: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(items.len(), 5);

    // Verify all items exclude list_model=false fields
    for product in &items {
        // All should have basic fields
        assert!(product.get("name").is_some());
        assert!(product.get("price").is_some());

        // All should exclude expensive fields
        assert!(product.get("description").is_none());
        assert!(product.get("specifications").is_none());
        assert!(product.get("weight_kg").is_none());
        assert!(product.get("dimensions").is_none());
        assert!(product.get("updated_at").is_none());
    }

    // Verify all 5 products exist (order doesn't matter)
    let names: Vec<String> = items
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_string())
        .collect();
    for i in 1..=5 {
        assert!(names.contains(&format!("Product {}", i)));
    }
}

#[tokio::test]
async fn test_list_model_with_sorting_on_hidden_field() {
    // Verify that fields with list_model=false can still be used for sorting
    let db = setup_products_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_products_app(&db);

    // Create products with different weights
    for i in 1..=3 {
        let create_data = json!({
            "name": format!("Product {}", i),
            "price": 10.0,
            "weight_kg": (4 - i) as f64  // 3.0, 2.0, 1.0
        });

        let request = Request::builder()
            .method("POST")
            .uri("/products")
            .header("content-type", "application/json")
            .body(Body::from(create_data.to_string()))
            .unwrap();

        app.clone().oneshot(request).await.unwrap();
    }

    // Sort by weight_kg (which has list_model=false but is sortable)
    let request = Request::builder()
        .method("GET")
        .uri("/products?sort_by=weight_kg&sort_order=asc")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let items: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    // Should be sorted by weight (ascending): Product 3, Product 2, Product 1
    assert_eq!(items[0]["name"], "Product 3");
    assert_eq!(items[1]["name"], "Product 2");
    assert_eq!(items[2]["name"], "Product 1");

    // But weight_kg should NOT be in the response
    for item in items {
        assert!(item.get("weight_kg").is_none());
    }
}
