use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_app, setup_test_db, todo_entity::Todo};

async fn create_test_todos(app: &axum::Router) -> Vec<Todo> {
    let todos = vec![
        json!({"title": "Alpha Todo", "completed": false}),
        json!({"title": "Beta Todo", "completed": true}),
        json!({"title": "Gamma Todo", "completed": false}),
        json!({"title": "Delta Todo", "completed": true}),
        json!({"title": "Epsilon Todo", "completed": false}),
    ];

    let mut created_todos = Vec::new();

    for todo_data in todos {
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/todos")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&todo_data).unwrap()))
            .unwrap();

        let app_clone = app.clone();
        let response = app_clone.oneshot(request).await.unwrap();

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let todo: Todo = serde_json::from_slice(&body).unwrap();
        created_todos.push(todo);
    }

    created_todos
}

#[tokio::test]
async fn test_list_with_pagination() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Test first page
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?page=0&per_page=2")
        .body(Body::empty())
        .unwrap();

    let app_clone = app.clone();
    let response = app_clone.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let headers = response.headers().clone();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 2);
    // Check Content-Range header for pagination info
    assert!(headers.contains_key("content-range"));

    // Test second page
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?page=1&per_page=2")
        .body(Body::empty())
        .unwrap();

    let app_clone = app.clone();
    let response = app_clone.oneshot(request).await.unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 2);

    // Test last page
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?page=2&per_page=2")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 1);
}

#[tokio::test]
async fn test_list_with_sorting() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Test sorting by title ascending
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?sort=title&order=ASC")
        .body(Body::empty())
        .unwrap();

    let app_clone = app.clone();
    let response = app_clone.oneshot(request).await.unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos[0].title, "Alpha Todo");
    assert_eq!(todos[1].title, "Beta Todo");
    assert_eq!(todos[2].title, "Delta Todo");

    // Test sorting by title descending
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?sort=title&order=DESC")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos[0].title, "Gamma Todo");
    assert_eq!(todos[1].title, "Epsilon Todo");
    assert_eq!(todos[2].title, "Delta Todo");
}

#[tokio::test]
async fn test_list_with_filtering() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Test filtering by completed status
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?filter=%7B%22completed%22%3Atrue%7D")
        .body(Body::empty())
        .unwrap();

    let app_clone = app.clone();
    let response = app_clone.oneshot(request).await.unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 2);
    assert!(todos.iter().all(|todo| todo.completed));

    // Test filtering by title (contains)
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?filter=%7B%22title%22%3A%22Beta%22%7D")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].title, "Beta Todo");
}

#[tokio::test]
async fn test_list_with_combined_operations() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Test filtering + sorting + pagination
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?filter=%7B%22completed%22%3Afalse%7D&sort=title&order=DESC&page=0&per_page=2")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 2);
    // Only 3 incomplete todos total
    assert_eq!(todos[0].title, "Gamma Todo");
    assert_eq!(todos[1].title, "Epsilon Todo");
    assert!(!todos[0].completed);
    assert!(!todos[1].completed);
}

#[tokio::test]
async fn test_list_with_invalid_sort_field() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Test with invalid sort field - should still return results (ignoring invalid sort)
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?sort=invalid_field&order=ASC")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 5);
}

#[tokio::test]
async fn test_list_empty_results() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Don't create any todos

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 0);
}

#[tokio::test]
async fn test_list_default_pagination() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create many todos to test default pagination
    for i in 0..15 {
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

    // Test without pagination parameters (should use defaults)
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 10); // Default per_page should be 10
}
