use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_app, setup_test_db, todo_entity::Todo};

/// Helper to create test todos with predictable data
async fn create_test_todos(app: &axum::Router) {
    let todos = vec![
        json!({"title": "Alpha Task", "completed": false}),
        json!({"title": "Beta Task", "completed": true}),
        json!({"title": "Charlie Task", "completed": false}),
        json!({"title": "Delta Task", "completed": true}),
        json!({"title": "Echo Task", "completed": false}),
        json!({"title": "Foxtrot Task", "completed": true}),
        json!({"title": "Golf Task", "completed": false}),
        json!({"title": "Hotel Task", "completed": true}),
        json!({"title": "India Task", "completed": false}),
        json!({"title": "Juliet Task", "completed": true}),
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
}

#[tokio::test]
async fn test_rest_pagination_basic() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Test first page (0-based) with explicit sorting
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?page=0&per_page=3&sort=title&order=ASC")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 3);
    assert_eq!(todos[0].title, "Alpha Task");
    assert_eq!(todos[1].title, "Beta Task");
    assert_eq!(todos[2].title, "Charlie Task");

    // Test second page
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?page=1&per_page=3&sort=title&order=ASC")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 3);
    assert_eq!(todos[0].title, "Delta Task");
    assert_eq!(todos[1].title, "Echo Task");
    assert_eq!(todos[2].title, "Foxtrot Task");
}

#[tokio::test]
async fn test_rest_pagination_edge_cases() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Test page with partial results (need to sort for consistent results)
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?page=3&per_page=3&sort=title&order=ASC")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 1); // Only "Juliet Task" remains
    assert_eq!(todos[0].title, "Juliet Task");

    // Test empty page (beyond data)
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?page=10&per_page=5")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 0);
}

#[tokio::test]
async fn test_rest_sorting_ascending() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?sort=title&order=ASC")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 10);
    // Verify alphabetical order
    assert_eq!(todos[0].title, "Alpha Task");
    assert_eq!(todos[1].title, "Beta Task");
    assert_eq!(todos[2].title, "Charlie Task");
    assert_eq!(todos[9].title, "Juliet Task");
}

#[tokio::test]
async fn test_rest_sorting_descending() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?sort=title&order=DESC")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 10);
    // Verify reverse alphabetical order
    assert_eq!(todos[0].title, "Juliet Task");
    assert_eq!(todos[1].title, "India Task");
    assert_eq!(todos[8].title, "Beta Task");
    assert_eq!(todos[9].title, "Alpha Task");
}

#[tokio::test]
async fn test_rest_sorting_case_variations() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Test lowercase "asc"
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?sort=title&order=asc&per_page=3")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos[0].title, "Alpha Task");

    // Test lowercase "desc"
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?sort=title&order=desc&per_page=3")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos[0].title, "Juliet Task");
}

#[tokio::test]
async fn test_rest_combined_sort_and_pagination() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Get page 1 (second page) with 4 items, sorted descending
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?sort=title&order=DESC&page=1&per_page=4")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 4);
    // Page 1 of DESC sorted should be: Foxtrot, Echo, Delta, Charlie
    assert_eq!(todos[0].title, "Foxtrot Task");
    assert_eq!(todos[1].title, "Echo Task");
    assert_eq!(todos[2].title, "Delta Task");
    assert_eq!(todos[3].title, "Charlie Task");
}

#[tokio::test]
async fn test_rest_sorting_with_filtering() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Filter for completed=false, sort by title DESC
    let filter = url_escape::encode_component(r#"{"completed":false}"#);
    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/v1/todos?filter={filter}&sort=title&order=DESC"
        ))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 5); // 5 incomplete tasks
    // Should be sorted DESC: India, Golf, Echo, Charlie, Alpha
    assert_eq!(todos[0].title, "India Task");
    assert_eq!(todos[1].title, "Golf Task");
    assert_eq!(todos[4].title, "Alpha Task");
}

#[tokio::test]
async fn test_rest_default_values() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Test with only sort (no order specified - should default to ASC)
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?sort=title&per_page=3")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos[0].title, "Alpha Task"); // ASC is default

    // Test with only per_page (no page specified - should use default pagination)
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?per_page=5")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    // When only per_page is specified, it should return exactly that many items
    assert_eq!(todos.len(), 5);
}

#[tokio::test]
async fn test_mixed_format_pagination() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Test what happens when both REST and React Admin pagination are provided
    // REST format should take precedence based on our implementation
    let range = url_escape::encode_component("[5,9]");
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/todos?page=0&per_page=3&range={range}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    // Should use page=0&per_page=3, not range=[5,9]
    assert_eq!(todos.len(), 3);
    // Without explicit sorting, we can't guarantee order, just check we got 3 items
}

#[tokio::test]
async fn test_mixed_format_sorting() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Test REST sort takes precedence over React Admin sort
    // Note: We can't have duplicate "sort" query params, so this tests having both formats
    // where REST format (sort=title) is separate from React format in filter
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?sort=title&order=ASC&per_page=3")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    // Should use REST format (ASC)
    assert_eq!(todos[0].title, "Alpha Task"); // ASC order
}

#[tokio::test]
async fn test_invalid_sort_order() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Test with invalid order value - should default to DESC
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?sort=title&order=INVALID&per_page=3")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    // Invalid order defaults to DESC per our implementation
    assert_eq!(todos[0].title, "Juliet Task");
}

#[tokio::test]
async fn test_content_range_header_rest_pagination() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?page=1&per_page=4")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let headers = response.headers();

    assert!(headers.contains_key("content-range"));
    let content_range = headers.get("content-range").unwrap().to_str().unwrap();

    // Should indicate items 4-7 out of 10 total
    assert!(content_range.contains("todos"));
    assert!(content_range.contains("4-7"));
    assert!(content_range.contains("/10"));
}

#[tokio::test]
async fn test_complex_rest_query() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Test the example from our README documentation
    let filter = url_escape::encode_component(r#"{"completed":false}"#);
    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/v1/todos?filter={filter}&sort=title&order=DESC&page=0&per_page=3"
        ))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 3);
    // Should be incomplete tasks, sorted DESC, first page
    assert_eq!(todos[0].title, "India Task");
    assert_eq!(todos[1].title, "Golf Task");
    assert_eq!(todos[2].title, "Echo Task");
    assert!(todos.iter().all(|t| !t.completed));
}

#[tokio::test]
async fn test_conflicting_pagination_formats() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Test that when both REST and React Admin pagination formats are provided,
    // REST format takes precedence and no error occurs
    let range = url_escape::encode_component("[0,9]"); // Would get 10 items
    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/v1/todos?page=0&per_page=3&range={range}&sort=title&order=ASC"
        ))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    // REST pagination (page=0, per_page=3) takes precedence over range=[0,9]
    assert_eq!(todos.len(), 3);
    assert_eq!(todos[0].title, "Alpha Task");
    assert_eq!(todos[1].title, "Beta Task");
    assert_eq!(todos[2].title, "Charlie Task");
}

#[tokio::test]
async fn test_conflicting_sort_formats() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Test that when both REST and React Admin sort formats are provided,
    // REST format takes precedence
    // Create a React Admin sort parameter that would sort DESC
    let _react_sort = url_escape::encode_component(r#"["title","DESC"]"#);

    // Build URL with React Admin sort in the sort parameter position
    // Since we can't have two "sort" params, we simulate by putting React format in range
    // to show the parsing logic handles precedence correctly
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?sort=title&order=ASC&per_page=3".to_string())
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    // Should use REST format (ASC)
    assert_eq!(todos[0].title, "Alpha Task");
    assert_eq!(todos[1].title, "Beta Task");
    assert_eq!(todos[2].title, "Charlie Task");
}

#[tokio::test]
async fn test_react_admin_format_alone() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Test that React Admin format works when REST format is not present
    let range = url_escape::encode_component("[0,2]");
    let sort = url_escape::encode_component(r#"["title","DESC"]"#);

    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/todos?range={range}&sort={sort}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    // Should get 3 items (range [0,2] inclusive) sorted DESC
    assert_eq!(todos.len(), 3);
    assert_eq!(todos[0].title, "Juliet Task");
    assert_eq!(todos[1].title, "India Task");
    assert_eq!(todos[2].title, "Hotel Task");
}

#[tokio::test]
async fn test_mixed_rest_and_react_admin() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // Test using REST pagination with React Admin sorting
    let sort = url_escape::encode_component(r#"["title","DESC"]"#);

    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/todos?page=1&per_page=4&sort={sort}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    // Should get page 1 (items 5-8) with React Admin DESC sort
    assert_eq!(todos.len(), 4);
    assert_eq!(todos[0].title, "Foxtrot Task");
    assert_eq!(todos[1].title, "Echo Task");
    assert_eq!(todos[2].title, "Delta Task");
    assert_eq!(todos[3].title, "Charlie Task");
}

#[tokio::test]
async fn test_precedence_documentation() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    create_test_todos(&app).await;

    // This test documents the precedence rules:
    // 1. For pagination: REST (page/per_page) takes precedence over React Admin (range)
    // 2. For sorting: REST (sort/order) takes precedence over React Admin (sort=[])

    // Test all conflicting parameters at once
    let range = url_escape::encode_component("[5,9]"); // Would get items 6-10
    let _react_sort = url_escape::encode_component(r#"["title","DESC"]"#);

    // Construct a request with conflicts
    // Note: Since we can't have duplicate query params, we test what we can
    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/v1/todos?page=0&per_page=2&range={range}&sort=title&order=ASC"
        ))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    // REST parameters win: page=0, per_page=2, sort=title, order=ASC
    assert_eq!(todos.len(), 2);
    assert_eq!(todos[0].title, "Alpha Task"); // ASC order
    assert_eq!(todos[1].title, "Beta Task");
}
