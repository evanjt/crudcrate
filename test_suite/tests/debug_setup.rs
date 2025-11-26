// Database and application setup tests

mod common;
use common::{category, setup_test_app, setup_test_db};
use crudcrate::traits::CRUDResource;

#[tokio::test]
async fn test_database_setup() {
    let _db = setup_test_db().await.expect("Failed to setup database");
}

#[tokio::test]
async fn test_app_setup() {
    let db = setup_test_db().await.expect("Failed to setup database");
    let _app = setup_test_app(&db);
}

#[tokio::test]
async fn test_direct_category_create() {
    let db = setup_test_db().await.expect("Failed to setup database");

    use category::{Category, CategoryCreate};

    let create_data = CategoryCreate {
        name: "Test Category".to_string(),
        parent_id: None,
    };

    let result = Category::create(&db, create_data).await;
    assert!(result.is_ok(), "Direct create should succeed");
}

#[tokio::test]
async fn test_self_referencing_join_via_direct_api() {
    let db = setup_test_db().await.expect("Failed to setup database");

    use category::{Category, CategoryCreate};

    // Create parent category
    let parent_data = CategoryCreate {
        name: "Parent".to_string(),
        parent_id: None,
    };
    let parent = Category::create(&db, parent_data)
        .await
        .expect("Failed to create parent");
    let parent_id = parent.id;

    // Create child categories
    let child1_data = CategoryCreate {
        name: "Child 1".to_string(),
        parent_id: Some(parent_id),
    };
    Category::create(&db, child1_data)
        .await
        .expect("Failed to create child 1");

    let child2_data = CategoryCreate {
        name: "Child 2".to_string(),
        parent_id: Some(parent_id),
    };
    Category::create(&db, child2_data)
        .await
        .expect("Failed to create child 2");

    // Fetch parent - children should be loaded
    let parent_with_children = Category::get_one(&db, parent_id)
        .await
        .expect("Failed to get parent");

    assert_eq!(parent_with_children.children.len(), 2);
    assert!(parent_with_children.children.iter().any(|c| c.name == "Child 1"));
    assert!(parent_with_children.children.iter().any(|c| c.name == "Child 2"));
}
