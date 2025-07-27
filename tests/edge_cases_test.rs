use axum::http::{Request, StatusCode};
use axum::body::Body;
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_app, setup_test_db, todo_entity::Todo};

#[tokio::test]
async fn test_create_with_null_fields() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Test creating with explicit null (should use default)
    let create_data = json!({
        "title": "Test with null",
        "completed": null
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&create_data).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todo: Todo = serde_json::from_slice(&body).unwrap();
    assert_eq!(todo.completed, false); // Should use default value
}

#[tokio::test]
async fn test_update_with_null_to_unset() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create a todo
    let create_data = json!({
        "title": "Original Title",
        "completed": true
    });

    let create_request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&create_data).unwrap()))
        .unwrap();

    let app_clone = app.clone();
    let create_response = app_clone.oneshot(create_request).await.unwrap();
    
    let body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_todo: Todo = serde_json::from_slice(&body).unwrap();

    // Update with null value - for non-nullable fields this should be ignored
    let update_data = json!({
        "title": null,
        "completed": false
    });

    let update_request = Request::builder()
        .method("PUT")
        .uri(&format!("/api/v1/todos/{}", created_todo.id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&update_data).unwrap()))
        .unwrap();

    let update_response = app.oneshot(update_request).await.unwrap();
    
    // This should fail because title is required and cannot be null
    assert_eq!(update_response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_empty_string_title() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Test creating with empty string title
    let create_data = json!({
        "title": "",
        "completed": false
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&create_data).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    // This should succeed - empty string is still a valid string
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todo: Todo = serde_json::from_slice(&body).unwrap();
    assert_eq!(todo.title, "");
}

#[tokio::test]
async fn test_very_long_title() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Test with a very long title
    let long_title = "A".repeat(1000);
    let create_data = json!({
        "title": long_title.clone(),
        "completed": false
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&create_data).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todo: Todo = serde_json::from_slice(&body).unwrap();
    assert_eq!(todo.title, long_title);
}

#[tokio::test]
async fn test_malformed_json() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    let malformed_json = r#"{"title": "Test", "completed": false"#; // Missing closing brace

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(malformed_json))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_invalid_uuid_format() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    let invalid_uuid = "not-a-uuid";
    
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/todos/{}", invalid_uuid))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_wrong_content_type() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    let create_data = json!({
        "title": "Test Todo",
        "completed": false
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "text/plain") // Wrong content type
        .body(Body::from(serde_json::to_string(&create_data).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn test_concurrent_updates() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create a todo
    let create_data = json!({
        "title": "Concurrent Test",
        "completed": false
    });

    let create_request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&create_data).unwrap()))
        .unwrap();

    let app_clone = app.clone();
    let create_response = app_clone.oneshot(create_request).await.unwrap();
    
    let body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_todo: Todo = serde_json::from_slice(&body).unwrap();

    // Simulate concurrent updates
    let update_data1 = json!({
        "title": "Updated by Request 1"
    });

    let update_data2 = json!({
        "title": "Updated by Request 2"
    });

    let update_request1 = Request::builder()
        .method("PUT")
        .uri(&format!("/api/v1/todos/{}", created_todo.id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&update_data1).unwrap()))
        .unwrap();

    let update_request2 = Request::builder()
        .method("PUT")
        .uri(&format!("/api/v1/todos/{}", created_todo.id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&update_data2).unwrap()))
        .unwrap();

    let app_clone1 = app.clone();
    let app_clone2 = app.clone();

    // Execute both updates
    let response1 = app_clone1.oneshot(update_request1).await.unwrap();
    let response2 = app_clone2.oneshot(update_request2).await.unwrap();

    // Both should succeed
    assert_eq!(response1.status(), StatusCode::OK);
    assert_eq!(response2.status(), StatusCode::OK);

    // Check final state
    let get_request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/todos/{}", created_todo.id))
        .body(Body::empty())
        .unwrap();

    let get_response = app.oneshot(get_request).await.unwrap();
    
    let body = axum::body::to_bytes(get_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let final_todo: Todo = serde_json::from_slice(&body).unwrap();
    
    // The title should be one of the two updates
    assert!(final_todo.title == "Updated by Request 1" || final_todo.title == "Updated by Request 2");
}

#[tokio::test]
async fn test_special_characters_in_title() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Test with special characters
    let special_title = "Todo with émojis 🎉 and special chars: <>&\"'";
    let create_data = json!({
        "title": special_title,
        "completed": false
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&create_data).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todo: Todo = serde_json::from_slice(&body).unwrap();
    assert_eq!(todo.title, special_title);
}

#[tokio::test]
async fn test_pagination_out_of_bounds() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create only 3 todos
    for i in 0..3 {
        let todo_data = json!({"title": format!("Todo {}", i), "completed": false});
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/todos")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&todo_data).unwrap()))
            .unwrap();

        let app_clone = app.clone();
        app_clone.oneshot(request).await.unwrap();
    }

    // Request page that doesn't exist
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?page=10&per_page=10")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(todos.len(), 0); // Should return empty results
}