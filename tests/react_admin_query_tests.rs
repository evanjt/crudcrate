use axum::body::Body;
use axum::http::{HeaderName, Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;
use url::Url;

mod common;
use common::{setup_test_app, setup_test_db, todo_entity::Todo};

/// Tests for React Admin simple-rest data provider compatibility
/// Based on: https://marmelab.com/react-admin/DataProviders.html

#[tokio::test]
async fn test_getlist_with_pagination_range() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create test data
    for i in 0..10 {
        let todo_data = json!({"title": format!("Todo {}", i), "completed": i % 2 == 0});
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/todos")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&todo_data).unwrap()))
            .unwrap();

        let app_clone = app.clone();
        app_clone.oneshot(request).await.unwrap();
    }

    // Test React Admin range format: range=[0, 4]
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?range=[0,4]")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Check Content-Range header is present (required by React Admin)
    let headers = response.headers().clone();
    assert!(headers.contains_key("content-range"));

    let content_range = headers.get("content-range").unwrap().to_str().unwrap();
    // Should be in format: "todos 0-4/10"
    assert!(content_range.contains("todos"));
    assert!(content_range.contains("0-4"));
    assert!(content_range.contains("/10"));

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 5); // Should return 5 items (0-4 inclusive)

    // Verify each item has an id (required by React Admin)
    for todo in &todos {
        assert!(!todo.id.is_nil());
    }
}

#[tokio::test]
async fn test_getlist_with_sorting_react_admin_format() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create test data with different titles
    let titles = vec!["Zebra", "Alpha", "Beta", "Charlie"];
    for title in titles {
        let todo_data = json!({"title": title, "completed": false});
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/todos")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&todo_data).unwrap()))
            .unwrap();

        let app_clone = app.clone();
        app_clone.oneshot(request).await.unwrap();
    }

    // Test React Admin sort format: sort=["title","ASC"]
    let sort_param = url_escape::encode_component("[\"title\",\"ASC\"]");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/todos?sort={}", sort_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    // Verify sorting: Alpha, Beta, Charlie, Zebra
    assert_eq!(todos[0].title, "Alpha");
    assert_eq!(todos[1].title, "Beta");
    assert_eq!(todos[2].title, "Charlie");
    assert_eq!(todos[3].title, "Zebra");
}

#[tokio::test]
async fn test_getlist_with_filter_react_admin_format() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create test data
    let todo_data1 = json!({"title": "Work Todo", "completed": false});
    let todo_data2 = json!({"title": "Personal Todo", "completed": true});
    let todo_data3 = json!({"title": "Work Task", "completed": false});

    for todo_data in [todo_data1, todo_data2, todo_data3] {
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/todos")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&todo_data).unwrap()))
            .unwrap();

        let app_clone = app.clone();
        app_clone.oneshot(request).await.unwrap();
    }

    // Test React Admin filter format: filter={"completed":false}
    let filter_param = url_escape::encode_component("{\"completed\":false}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/todos?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 2); // Only incomplete todos
    assert!(todos.iter().all(|todo| !todo.completed));
}

#[tokio::test]
async fn test_getmany_with_ids_filter() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create test data and collect IDs
    let mut created_ids = Vec::new();
    for i in 0..5 {
        let todo_data = json!({"title": format!("Todo {}", i), "completed": false});
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
        created_ids.push(todo.id);
    }

    // Test React Admin getMany format: filter={"ids":[id1,id2,id3]}
    let selected_ids = &created_ids[0..3];
    let filter = json!({"ids": selected_ids});
    let filter_str = serde_json::to_string(&filter).unwrap();
    let encoded_filter = url_escape::encode_component(&filter_str);

    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/todos?filter={}", encoded_filter))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 3);

    // Verify we got the correct todos
    let returned_ids: Vec<_> = todos.iter().map(|t| t.id).collect();
    for id in selected_ids {
        assert!(returned_ids.contains(id));
    }
}

#[tokio::test]
async fn test_getone_react_admin_compatible() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create a todo
    let todo_data = json!({"title": "Test Todo", "completed": false});
    let create_request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&todo_data).unwrap()))
        .unwrap();

    let app_clone = app.clone();
    let create_response = app_clone.oneshot(create_request).await.unwrap();

    let body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_todo: Todo = serde_json::from_slice(&body).unwrap();

    // Test getOne: GET /resource/id
    let get_request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/todos/{}", created_todo.id))
        .body(Body::empty())
        .unwrap();

    let get_response = app.oneshot(get_request).await.unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(get_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let retrieved_todo: Todo = serde_json::from_slice(&body).unwrap();

    // Verify it has an id and matches created todo
    assert_eq!(retrieved_todo.id, created_todo.id);
    assert_eq!(retrieved_todo.title, "Test Todo");
    assert!(!retrieved_todo.id.is_nil());
}

#[tokio::test]
async fn test_create_react_admin_compatible() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Test create: POST /resource
    let todo_data = json!({"title": "New Todo", "completed": false});
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&todo_data).unwrap()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todo: Todo = serde_json::from_slice(&body).unwrap();

    // Verify returned object has id (required by React Admin)
    assert!(!todo.id.is_nil());
    assert_eq!(todo.title, "New Todo");
    assert_eq!(todo.completed, false);
}

#[tokio::test]
async fn test_update_react_admin_compatible() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create a todo first
    let todo_data = json!({"title": "Original Title", "completed": false});
    let create_request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&todo_data).unwrap()))
        .unwrap();

    let app_clone = app.clone();
    let create_response = app_clone.oneshot(create_request).await.unwrap();

    let body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_todo: Todo = serde_json::from_slice(&body).unwrap();

    // Test update: PUT /resource/id
    let update_data = json!({"title": "Updated Title", "completed": true});
    let update_request = Request::builder()
        .method("PUT")
        .uri(&format!("/api/v1/todos/{}", created_todo.id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&update_data).unwrap()))
        .unwrap();

    let update_response = app.oneshot(update_request).await.unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(update_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let updated_todo: Todo = serde_json::from_slice(&body).unwrap();

    // Verify returned object has same id and updated fields
    assert_eq!(updated_todo.id, created_todo.id);
    assert_eq!(updated_todo.title, "Updated Title");
    assert_eq!(updated_todo.completed, true);
}

#[tokio::test]
async fn test_delete_react_admin_compatible() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create a todo first
    let todo_data = json!({"title": "To Delete", "completed": false});
    let create_request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&todo_data).unwrap()))
        .unwrap();

    let app_clone = app.clone();
    let create_response = app_clone.oneshot(create_request).await.unwrap();

    let body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_todo: Todo = serde_json::from_slice(&body).unwrap();

    // Test delete: DELETE /resource/id
    let delete_request = Request::builder()
        .method("DELETE")
        .uri(&format!("/api/v1/todos/{}", created_todo.id))
        .body(Body::empty())
        .unwrap();

    let delete_response = app.oneshot(delete_request).await.unwrap();
    // React Admin expects successful deletion
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_complex_query_combination() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create diverse test data
    let test_data = vec![
        json!({"title": "Alpha Work", "completed": false}),
        json!({"title": "Beta Personal", "completed": true}),
        json!({"title": "Charlie Work", "completed": false}),
        json!({"title": "Delta Personal", "completed": true}),
        json!({"title": "Echo Work", "completed": false}),
        json!({"title": "Foxtrot Personal", "completed": true}),
    ];

    for todo_data in test_data {
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/todos")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&todo_data).unwrap()))
            .unwrap();

        let app_clone = app.clone();
        app_clone.oneshot(request).await.unwrap();
    }

    // Test combination: filter + sort + range (typical React Admin query)
    let filter_param = url_escape::encode_component("{\"completed\":false}");
    let sort_param = url_escape::encode_component("[\"title\",\"ASC\"]");
    let range_param = url_escape::encode_component("[0,1]");
    let request = Request::builder()
        .method("GET")
        .uri(&format!(
            "/api/v1/todos?filter={}&sort={}&range={}",
            filter_param, sort_param, range_param
        ))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Check Content-Range header
    let headers = response.headers().clone();
    assert!(headers.contains_key("content-range"));

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();

    assert_eq!(todos.len(), 2); // Range [0,1] = 2 items
    assert!(todos.iter().all(|todo| !todo.completed)); // All should be incomplete
    // Should be sorted: Alpha Work, Charlie Work
    assert_eq!(todos[0].title, "Alpha Work");
    assert_eq!(todos[1].title, "Charlie Work");
}

#[tokio::test]
async fn test_cors_headers_for_react_admin() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create some test data
    let todo_data = json!({"title": "Test Todo", "completed": false});
    let create_request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&todo_data).unwrap()))
        .unwrap();

    let app_clone = app.clone();
    app_clone.oneshot(create_request).await.unwrap();

    // Test that Content-Range header is exposed (required for CORS)
    let list_request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(list_request).await.unwrap();
    let headers = response.headers().clone();

    // Content-Range must be present for React Admin pagination
    assert!(headers.contains_key("content-range"));

    // In a real app, you'd also check for Access-Control-Expose-Headers
    // but that's typically handled by CORS middleware
}

#[tokio::test]
async fn test_error_handling_react_admin_compatible() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Test 404 for non-existent resource
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/todos/{}", uuid::Uuid::new_v4()))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Test 422 for invalid data
    let invalid_data = json!({"completed": false}); // Missing required title
    let create_request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&invalid_data).unwrap()))
        .unwrap();

    let app_clone = app.clone();
    let response = app_clone.oneshot(create_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
