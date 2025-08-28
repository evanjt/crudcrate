use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_app, setup_test_db, todo_entity::Todo};

#[tokio::test]
async fn test_create_todo() {
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

    let response = app.oneshot(request).await.unwrap();
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
async fn test_get_todo_by_id() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // First create a todo
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

    let app_clone = app.clone();
    let create_response = app_clone.oneshot(create_request).await.unwrap();

    let body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_todo: Todo = serde_json::from_slice(&body).unwrap();

    // Now get the todo by ID
    let get_request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/todos/{}", created_todo.id))
        .body(Body::empty())
        .unwrap();

    let get_response = app.oneshot(get_request).await.unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(get_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let retrieved_todo: Todo = serde_json::from_slice(&body).unwrap();
    assert_eq!(retrieved_todo.id, created_todo.id);
    assert_eq!(retrieved_todo.title, "Get Test Todo");
    assert!(retrieved_todo.completed);
}

#[tokio::test]
async fn test_update_todo() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // First create a todo
    let create_data = json!({
        "title": "Original Title",
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

    // Small delay to ensure updated_at timestamp is different across all database backends
    // (MySQL has second precision, while SQLite/PostgreSQL have microsecond precision)
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Update the todo
    let update_data = json!({
        "title": "Updated Title",
        "completed": true
    });

    let update_request = Request::builder()
        .method("PUT")
        .uri(format!("/api/v1/todos/{}", created_todo.id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&update_data).unwrap()))
        .unwrap();

    let update_response = app.oneshot(update_request).await.unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(update_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated_todo: Todo = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated_todo.id, created_todo.id);
    assert_eq!(updated_todo.title, "Updated Title");
    assert!(updated_todo.completed);
    assert!(updated_todo.updated_at > created_todo.updated_at);
}

#[tokio::test]
async fn test_partial_update_todo() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create a todo
    let create_data = json!({
        "title": "Partial Update Test",
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

    // Partial update - only update completed field
    let update_data = json!({
        "completed": true
    });

    let update_request = Request::builder()
        .method("PUT")
        .uri(format!("/api/v1/todos/{}", created_todo.id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&update_data).unwrap()))
        .unwrap();

    let update_response = app.oneshot(update_request).await.unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(update_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated_todo: Todo = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated_todo.title, "Partial Update Test"); // Title should remain unchanged
    assert!(updated_todo.completed); // Only completed should be updated
}

#[tokio::test]
async fn test_delete_todo() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // First create a todo
    let create_data = json!({
        "title": "Delete Test Todo",
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

    // Delete the todo
    let delete_request = Request::builder()
        .method("DELETE")
        .uri(format!("/api/v1/todos/{}", created_todo.id))
        .body(Body::empty())
        .unwrap();

    let app_clone2 = app.clone();
    let delete_response = app_clone2.oneshot(delete_request).await.unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    // Verify it's deleted by trying to get it
    let get_request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/todos/{}", created_todo.id))
        .body(Body::empty())
        .unwrap();

    let get_response = app.oneshot(get_request).await.unwrap();
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_list_todos() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create multiple todos
    let todos = vec![
        json!({"title": "Todo 1", "completed": false}),
        json!({"title": "Todo 2", "completed": true}),
        json!({"title": "Todo 3", "completed": false}),
    ];

    for todo_data in todos {
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/todos")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&todo_data).unwrap()))
            .unwrap();

        let app_clone = app.clone();
        app_clone.oneshot(request).await.unwrap();
    }

    // List all todos
    let list_request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?page=0&per_page=10")
        .body(Body::empty())
        .unwrap();

    let list_response = app.oneshot(list_request).await.unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(list_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 3);
}

#[tokio::test]
async fn test_get_nonexistent_todo() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    let random_id = uuid::Uuid::new_v4();
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/todos/{random_id}"))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_create_todo_invalid_data() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Missing required field (title)
    let invalid_data = json!({
        "completed": false
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&invalid_data).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_update_nonexistent_todo() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    let random_id = uuid::Uuid::new_v4();
    let update_data = json!({
        "title": "Updated Title"
    });

    let request = Request::builder()
        .method("PUT")
        .uri(format!("/api/v1/todos/{random_id}"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&update_data).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
