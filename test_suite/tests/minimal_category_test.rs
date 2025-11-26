// Category model tests - verifies self-referencing relationship loading

use axum::body::Body;
use axum::http::Request;
use axum::Router;
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{category, setup_test_app, setup_test_db};

#[tokio::test]
async fn test_category_router_creation() {
    let db = setup_test_db().await.expect("Failed to setup database");
    let openapi_router = category::Category::router(&db);
    let _router: Router = openapi_router.into();
}

#[tokio::test]
async fn test_category_router_nested() {
    let db = setup_test_db().await.expect("Failed to setup database");
    let _router = Router::new().nest("/categories", category::Category::router(&db).into());
}

#[tokio::test]
async fn test_self_referencing_children_in_response() {
    let db = setup_test_db().await.expect("Failed to setup database");
    let app = setup_test_app(&db);

    // Create root category
    let root_data = json!({"name": "Electronics"});
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/categories")
                .header("content-type", "application/json")
                .body(Body::from(root_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let root_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let root: serde_json::Value = serde_json::from_slice(&root_body).unwrap();
    let root_id = root["id"].as_str().unwrap();

    // Create child category
    let child_data = json!({"name": "Laptops", "parent_id": root_id});
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/categories")
                .header("content-type", "application/json")
                .body(Body::from(child_data.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Fetch root category - children should be included
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/categories/{}", root_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let category: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify children field is present and populated
    assert!(
        category.get("children").is_some(),
        "Response should include children field"
    );
    let children = category["children"].as_array().unwrap();
    assert_eq!(children.len(), 1, "Should have 1 child");
    assert_eq!(children[0]["name"], "Laptops");
}
