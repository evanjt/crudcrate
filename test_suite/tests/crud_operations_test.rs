// Feature Group 1: Auto-Generated CRUD Operations
// Tests HTTP endpoints, status codes, JSON responses, router generation

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_app, setup_test_db, todo_entity::Todo};

#[tokio::test]
async fn test_create_todo_endpoint() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    let create_data = json!({
        "title": "Test Todo",
        "completed": false
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&create_data).unwrap()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todo: Todo = serde_json::from_slice(&body).unwrap();
    assert_eq!(todo.title, "Test Todo");
    assert!(!todo.completed);
    assert!(!todo.id.is_nil());
}

#[tokio::test]
async fn test_get_one_todo_endpoint() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create a todo first
    let create_data = json!({
        "title": "Get Test Todo",
        "completed": true
    });

    let create_request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&create_data).unwrap()))
        .unwrap();

    let create_response = app.clone().oneshot(create_request).await.unwrap();
    let create_body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_todo: Todo = serde_json::from_slice(&create_body).unwrap();

    // Get the todo by ID
    let get_request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/todos/{}", created_todo.id))
        .body(Body::empty())
        .unwrap();

    let get_response = app.clone().oneshot(get_request).await.unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);

    let get_body = axum::body::to_bytes(get_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let retrieved_todo: Todo = serde_json::from_slice(&get_body).unwrap();
    assert_eq!(retrieved_todo.id, created_todo.id);
    assert_eq!(retrieved_todo.title, "Get Test Todo");
    assert!(retrieved_todo.completed);
}

#[tokio::test]
async fn test_get_all_todos_endpoint() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create multiple todos
    for i in 1..=3 {
        let create_data = json!({
            "title": format!("Todo {}", i),
            "completed": i % 2 == 0
        });

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/todos")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&create_data).unwrap()))
            .unwrap();

        app.clone().oneshot(request).await.unwrap();
    }

    // Get all todos
    let get_all_request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(get_all_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();
    assert_eq!(todos.len(), 3);
}

#[tokio::test]
async fn test_update_todo_endpoint() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create a todo
    let create_data = json!({
        "title": "Update Me",
        "completed": false
    });

    let create_request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&create_data).unwrap()))
        .unwrap();

    let create_response = app.clone().oneshot(create_request).await.unwrap();
    let create_body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_todo: Todo = serde_json::from_slice(&create_body).unwrap();

    // Update the todo
    let update_data = json!({
        "title": "Updated Title",
        "completed": true
    });

    let update_request = Request::builder()
        .method("PUT")
        .uri(&format!("/api/v1/todos/{}", created_todo.id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&update_data).unwrap()))
        .unwrap();

    let update_response = app.clone().oneshot(update_request).await.unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);

    let update_body = axum::body::to_bytes(update_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated_todo: Todo = serde_json::from_slice(&update_body).unwrap();
    assert_eq!(updated_todo.id, created_todo.id);
    assert_eq!(updated_todo.title, "Updated Title");
    assert!(updated_todo.completed);
}

#[tokio::test]
async fn test_delete_todo_endpoint() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create a todo
    let create_data = json!({
        "title": "Delete Me",
        "completed": false
    });

    let create_request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&create_data).unwrap()))
        .unwrap();

    let create_response = app.clone().oneshot(create_request).await.unwrap();
    let create_body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_todo: Todo = serde_json::from_slice(&create_body).unwrap();

    // Delete the todo
    let delete_request = Request::builder()
        .method("DELETE")
        .uri(&format!("/api/v1/todos/{}", created_todo.id))
        .body(Body::empty())
        .unwrap();

    let delete_response = app.clone().oneshot(delete_request).await.unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    // Verify it's deleted by trying to get it
    let get_request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/todos/{}", created_todo.id))
        .body(Body::empty())
        .unwrap();

    let get_response = app.clone().oneshot(get_request).await.unwrap();
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_batch_delete_endpoint() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create multiple todos
    let mut todo_ids = Vec::new();
    for i in 1..=3 {
        let create_data = json!({
            "title": format!("Delete Me {}", i),
            "completed": false
        });

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/todos")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&create_data).unwrap()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let todo: Todo = serde_json::from_slice(&body).unwrap();
        todo_ids.push(todo.id);
    }

    // Batch delete
    let delete_request = Request::builder()
        .method("DELETE")
        .uri("/api/v1/todos/batch")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&todo_ids).unwrap()))
        .unwrap();

    let delete_response = app.clone().oneshot(delete_request).await.unwrap();
    assert_eq!(delete_response.status(), StatusCode::OK);

    let delete_body = axum::body::to_bytes(delete_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let deleted_ids: Vec<uuid::Uuid> = serde_json::from_slice(&delete_body).unwrap();
    assert_eq!(deleted_ids.len(), 3);
}

#[tokio::test]
async fn test_error_handling_not_found() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    let fake_id = uuid::Uuid::new_v4();
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/todos/{}", fake_id))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_error_handling_invalid_json() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from("invalid json"))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}