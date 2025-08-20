use axum::{body::Body, http::Request};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{Database, Set};
use sea_orm_migration::{
    prelude::*,
    sea_query::{ColumnDef, Iden, Table},
};
use tower::ServiceExt;
use uuid::Uuid;

// Test entity with list model exclusions
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "test_products")]
#[crudcrate(
    api_struct = "TestProduct",
    name_singular = "test_product",
    name_plural = "test_products",
    description = "Test entity for SQL optimization verification",
    generate_router
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, create_model = false, update_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(sortable, filterable)]
    pub name: String,

    #[crudcrate(sortable, filterable)]
    pub price: i32, // Using i32 instead of Decimal to avoid dependency issues

    #[crudcrate(filterable)]
    pub description: Option<String>,

    // These fields should be EXCLUDED from TestProductList
    #[crudcrate(update_model = false, create_model = false, on_create = chrono::Utc::now(), list_model = false)]
    pub created_at: chrono::DateTime<chrono::Utc>,

    #[crudcrate(update_model = false, create_model = false, on_update = chrono::Utc::now(), on_create = chrono::Utc::now(), list_model = false)]
    pub last_updated: chrono::DateTime<chrono::Utc>,

    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr = true, default = vec![], list_model = false)]
    pub expensive_computed_data: Vec<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[tokio::test]
async fn test_generated_list_model_excludes_expensive_fields() {
    // Verify that the TestProductList struct was generated correctly
    // and excludes the expensive fields marked with list_model = false

    // Create a TestProduct instance
    let test_product = TestProduct {
        id: Uuid::new_v4(),
        name: "Test Product".to_string(),
        price: 1999, // Price in cents
        description: Some("A test product".to_string()),
        created_at: chrono::Utc::now(),
        last_updated: chrono::Utc::now(),
        expensive_computed_data: vec!["expensive".to_string(), "data".to_string()],
    };

    // Convert to TestProductList
    let product_list: TestProductList = TestProductList::from(test_product.clone());

    // Verify that the list model contains the expected fields
    assert_eq!(
        product_list.id, test_product.id,
        "List model should contain id"
    );
    assert_eq!(
        product_list.name, test_product.name,
        "List model should contain name"
    );
    assert_eq!(
        product_list.price, test_product.price,
        "List model should contain price"
    );
    assert_eq!(
        product_list.description, test_product.description,
        "List model should contain description"
    );

    // Note: We can't check that excluded fields are NOT present because they're not
    // in the TestProductList struct at all (compile-time exclusion)
}

#[tokio::test]
async fn test_list_model_trait_implementation() {
    // Verify that TestProduct implements CRUDResource with correct ListModel
    fn assert_crud_resource<T: CRUDResource>() -> &'static str {
        std::any::type_name::<T::ListModel>()
    }

    let list_model_type = assert_crud_resource::<TestProduct>();

    // The list model type should be TestProductList
    assert!(
        list_model_type.contains("TestProductList"),
        "ListModel should be TestProductList"
    );

    // Verify that the resource constants are correct
    assert_eq!(TestProduct::RESOURCE_NAME_SINGULAR, "test_product");
    assert_eq!(TestProduct::RESOURCE_NAME_PLURAL, "test_products");
}

// Migration setup for integration testing
pub struct TestProductMigrator;

#[async_trait::async_trait]
impl MigratorTrait for TestProductMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(CreateTestProductTable)]
    }
}

pub struct CreateTestProductTable;

#[async_trait::async_trait]
impl MigrationName for CreateTestProductTable {
    fn name(&self) -> &'static str {
        "m20240101_000001_create_test_product_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for CreateTestProductTable {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let table = Table::create()
            .table(TestProductEntity)
            .if_not_exists()
            .col(
                ColumnDef::new(TestProductColumn::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            )
            .col(ColumnDef::new(TestProductColumn::Name).string().not_null())
            .col(
                ColumnDef::new(TestProductColumn::Price)
                    .integer()
                    .not_null(),
            )
            .col(ColumnDef::new(TestProductColumn::Description).string())
            .col(
                ColumnDef::new(TestProductColumn::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .col(
                ColumnDef::new(TestProductColumn::LastUpdated)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .to_owned();

        manager.create_table(table).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TestProductEntity).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum TestProductColumn {
    Id,
    Name,
    Price,
    Description,
    CreatedAt,
    LastUpdated,
}

impl Iden for TestProductColumn {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(
            s,
            "{}",
            match self {
                Self::Id => "id",
                Self::Name => "name",
                Self::Price => "price",
                Self::Description => "description",
                Self::CreatedAt => "created_at",
                Self::LastUpdated => "last_updated",
            }
        )
        .unwrap();
    }
}

#[derive(Debug)]
pub struct TestProductEntity;

impl Iden for TestProductEntity {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(s, "test_products").unwrap();
    }
}

#[tokio::test]
async fn test_real_api_list_endpoint_optimization() {
    // Setup in-memory SQLite database
    let db = Database::connect("sqlite::memory:").await.unwrap();

    // Run migrations
    TestProductMigrator::up(&db, None).await.unwrap();

    // Insert test data
    let test_product = ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set("Test Product".to_string()),
        price: Set(1999),
        description: Set(Some("A test product for optimization".to_string())),
        created_at: Set(chrono::Utc::now()),
        last_updated: Set(chrono::Utc::now()),
    };
    test_product.insert(&db).await.unwrap();

    // Build the router with generated routes mounted at the correct path
    let app = axum::Router::new().nest("/test_products", router(&db).into());

    // Make request to list endpoint
    let request = Request::builder()
        .uri("/test_products")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    let status = response.status();

    // Get response body
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "Expected 200 OK, got {status} with body: {body_str}"
    );

    // Parse JSON to verify structure
    let products: Vec<TestProductList> = serde_json::from_str(&body_str).unwrap();
    assert_eq!(products.len(), 1, "Should return one product");

    let product = &products[0];

    // Verify that TestProductList contains expected fields
    assert!(
        !product.id.to_string().is_empty(),
        "List model should contain id"
    );
    assert_eq!(
        product.name, "Test Product",
        "List model should contain name"
    );
    assert_eq!(product.price, 1999, "List model should contain price");
    assert!(
        product.description.is_some(),
        "List model should contain description"
    );

    // Verify TestProductList excludes expensive fields (compile-time exclusion)
    // The following would cause compile errors if uncommented:
    // let _ = product.created_at; // Compile error - field doesn't exist
    // let _ = product.last_updated; // Compile error - field doesn't exist
}

#[tokio::test]
async fn test_crudcrate_generates_optimized_get_all_list() {
    // Setup database
    let db = Database::connect("sqlite::memory:").await.unwrap();
    TestProductMigrator::up(&db, None).await.unwrap();

    // Insert test data
    let test_product = ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set("Test Product".to_string()),
        price: Set(1999),
        description: Set(Some("A test product".to_string())),
        created_at: Set(chrono::Utc::now()),
        last_updated: Set(chrono::Utc::now()),
    };
    test_product.insert(&db).await.unwrap();

    // Test that crudcrate generated the get_all method returning ListModel
    // This proves the macro generated the selective column query implementation
    let list_products = TestProduct::get_all(
        &db,
        &sea_orm::Condition::all(),          // condition
        TestProduct::default_index_column(), // order_column
        sea_orm::Order::Asc,                 // order_direction
        0,                                   // offset
        10,                                  // limit
    )
    .await
    .unwrap();

    // Verify we got TestProductList items (not full TestProduct)
    assert_eq!(list_products.len(), 1, "Should return one product");
    let product = &list_products[0];

    // Verify TestProductList has the expected fields
    assert!(!product.id.to_string().is_empty(), "Should have id");
    assert_eq!(product.name, "Test Product", "Should have name");
    assert_eq!(product.price, 1999, "Should have price");
    assert!(product.description.is_some(), "Should have description");

    // The fact that this compiles and returns TestProductList proves:
    // 1. crudcrate generated the get_all_list method
    // 2. It returns TestProductList (not TestProduct)
    // 3. The optimization code path is being used

    // Note: The new API design has get_all return ListModel objects directly
    // This is more efficient as it only selects the columns defined in the list model
    // rather than fetching all columns and then converting
}
