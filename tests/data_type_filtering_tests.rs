use axum::http::{Request, StatusCode};
use axum::body::Body;
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_db_with_tasks, setup_task_app, task_entity::{Task, Priority, Status}};

/// Comprehensive tests for filtering by different data types
/// Including: boolean, enum, string, float, integer, nullable fields

async fn create_diverse_test_tasks(app: &axum::Router) -> Vec<Task> {
    let test_tasks = vec![
        // Task 1: Basic completed task
        json!({
            "title": "Complete project setup",
            "description": "Set up the new project infrastructure",
            "completed": true,
            "priority": "High",
            "status": "Done",
            "score": 95.5,
            "points": 100,
            "estimated_hours": 8.5,
            "assignee_count": 2,
            "is_public": true
        }),
        // Task 2: In-progress task
        json!({
            "title": "Implement authentication",
            "description": "Add user login and registration",
            "completed": false,
            "priority": "Urgent",
            "status": "InProgress",
            "score": 67.25,
            "points": 75,
            "estimated_hours": 12.0,
            "assignee_count": 3,
            "is_public": false
        }),
        // Task 3: Low priority todo
        json!({
            "title": "Write documentation",
            "description": null, // Test nullable field
            "completed": false,
            "priority": "Low",
            "status": "Todo",
            "score": 0.0,
            "points": 25,
            "estimated_hours": null, // Test nullable field
            "assignee_count": 1,
            "is_public": true
        }),
        // Task 4: Medium priority cancelled
        json!({
            "title": "Legacy system migration",
            "description": "Move from old system to new one",
            "completed": false,
            "priority": "Medium",
            "status": "Cancelled",
            "score": 15.75,
            "points": 150,
            "estimated_hours": 40.0,
            "assignee_count": 5,
            "is_public": false
        }),
        // Task 5: Another completed task with different values
        json!({
            "title": "Bug fixes",
            "description": "Fix critical production bugs",
            "completed": true,
            "priority": "Urgent",
            "status": "Done",
            "score": 88.0,
            "points": 50,
            "estimated_hours": 4.25,
            "assignee_count": 1,
            "is_public": true
        }),
    ];

    let mut created_tasks = Vec::new();

    for task_data in test_tasks {
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/tasks")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&task_data).unwrap()))
            .unwrap();

        let app_clone = app.clone();
        let response = app_clone.oneshot(request).await.unwrap();
        
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        
        assert!(status.is_success(), "Task creation failed with status {}: {}", status, String::from_utf8_lossy(&body));
        
        let body_str = String::from_utf8_lossy(&body);
        assert!(!body_str.is_empty(), "Empty response body from task creation");
        
        let task: Task = serde_json::from_slice(&body)
            .map_err(|e| format!("Failed to parse task JSON '{body_str}': {e}"))
            .unwrap();
        created_tasks.push(task);
    }

    created_tasks
}

#[tokio::test]
async fn test_filter_by_boolean_completed() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_diverse_test_tasks(&app).await;

    // Test filtering by completed = true
    let filter_param = url_escape::encode_component("{\"completed\":true}");
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tasks?filter={filter_param}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should only return completed tasks
    for task in &tasks {
        assert!(task.completed, "Task '{}' should be completed", task.title);
    }
    
    // We created 2 completed tasks
    assert_eq!(tasks.len(), 2, "Should find 2 completed tasks, found {}", tasks.len());
}

#[tokio::test]
async fn test_filter_by_boolean_is_public() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_diverse_test_tasks(&app).await;

    // Test filtering by is_public = false
    let filter_param = url_escape::encode_component("{\"is_public\":false}");
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tasks?filter={filter_param}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should only return private tasks
    for task in &tasks {
        assert!(!task.is_public, "Task '{}' should be private", task.title);
    }
    
    // We created 2 private tasks
    assert_eq!(tasks.len(), 2, "Should find 2 private tasks, found {}", tasks.len());
}

#[tokio::test]
async fn test_filter_by_enum_priority() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_diverse_test_tasks(&app).await;

    // Test filtering by priority = "urgent" (case insensitive)
    let filter_param = url_escape::encode_component("{\"priority\":\"urgent\"}");
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tasks?filter={filter_param}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should only return urgent priority tasks
    for task in &tasks {
        assert_eq!(task.priority, Priority::Urgent, "Task '{}' should have urgent priority", task.title);
    }
    
    // We created 2 urgent tasks
    assert_eq!(tasks.len(), 2, "Should find 2 urgent tasks, found {}", tasks.len());
}

#[tokio::test]
async fn test_case_insensitive_enum_filtering() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_diverse_test_tasks(&app).await;

    // Test various case combinations for enum filtering
    let test_cases = vec![
        ("urgent", 2),    // lowercase
        ("URGENT", 2),    // uppercase
        ("Urgent", 2),    // proper case
        ("uRgEnT", 2),    // mixed case
        ("high", 1),      // lowercase
        ("HIGH", 1),      // uppercase  
        ("High", 1),      // proper case
        ("done", 2),      // lowercase status
        ("DONE", 2),      // uppercase status
        ("Done", 2),      // proper case status
        ("inprogress", 1), // lowercase compound
        ("INPROGRESS", 1), // uppercase compound
        ("InProgress", 1), // proper case compound
    ];

    for (priority_value, expected_count) in test_cases {
        let filter = if priority_value.to_lowercase().contains("done") || 
                       priority_value.to_lowercase().contains("progress") ||
                       priority_value.to_lowercase().contains("cancelled") ||
                       priority_value.to_lowercase().contains("todo") {
            format!("{{\"status\":\"{priority_value}\"}}")
        } else {
            format!("{{\"priority\":\"{priority_value}\"}}")
        };
        
        let filter_param = url_escape::encode_component(&filter);
        let request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/tasks?filter={filter_param}"))
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
        
        assert_eq!(tasks.len(), expected_count, 
            "Case-insensitive filter '{}' should find {} tasks, found {}", 
            priority_value, expected_count, tasks.len());
    }
}

#[tokio::test]
async fn test_filter_by_enum_status() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_diverse_test_tasks(&app).await;

    // Test filtering by status = "done" (case insensitive)
    let filter_param = url_escape::encode_component("{\"status\":\"done\"}");
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tasks?filter={filter_param}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should only return done status tasks
    for task in &tasks {
        assert_eq!(task.status, Status::Done, "Task '{}' should have done status", task.title);
    }
    
    // We created 2 done tasks
    assert_eq!(tasks.len(), 2, "Should find 2 done tasks, found {}", tasks.len());
}

#[tokio::test]
async fn test_filter_by_string_title() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_diverse_test_tasks(&app).await;

    // Test filtering by title containing "authentication"
    let filter_param = url_escape::encode_component("{\"title\":\"authentication\"}");
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tasks?filter={filter_param}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with "authentication" in title
    for task in &tasks {
        assert!(task.title.to_lowercase().contains("authentication"), 
            "Task '{}' should contain 'authentication'", task.title);
    }
    
    // We created 1 task with "authentication" in title
    assert_eq!(tasks.len(), 1, "Should find 1 authentication task, found {}", tasks.len());
}

#[tokio::test]
async fn test_filter_by_float_score() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_diverse_test_tasks(&app).await;

    // Test filtering by score = 95.5 (exact match)
    let filter_param = url_escape::encode_component("{\"score\":95.5}");
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tasks?filter={filter_param}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with score = 95.5
    for task in &tasks {
        assert!((task.score - 95.5).abs() < f64::EPSILON, 
            "Task '{}' should have score 95.5, got {}", task.title, task.score);
    }
    
    // We created 1 task with score 95.5
    assert_eq!(tasks.len(), 1, "Should find 1 task with score 95.5, found {}", tasks.len());
}

#[tokio::test]
async fn test_filter_by_integer_points() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_diverse_test_tasks(&app).await;

    // Test filtering by points = 100
    let filter_param = url_escape::encode_component("{\"points\":100}");
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tasks?filter={filter_param}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with points = 100
    for task in &tasks {
        assert_eq!(task.points, 100, "Task '{}' should have 100 points, got {}", task.title, task.points);
    }
    
    // We created 1 task with 100 points
    assert_eq!(tasks.len(), 1, "Should find 1 task with 100 points, found {}", tasks.len());
}

#[tokio::test]
async fn test_filter_by_small_integer_assignee_count() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_diverse_test_tasks(&app).await;

    // Test filtering by assignee_count = 1
    let filter_param = url_escape::encode_component("{\"assignee_count\":1}");
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tasks?filter={filter_param}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with assignee_count = 1
    for task in &tasks {
        assert_eq!(task.assignee_count, 1, "Task '{}' should have 1 assignee, got {}", task.title, task.assignee_count);
    }
    
    // We created 2 tasks with 1 assignee
    assert_eq!(tasks.len(), 2, "Should find 2 tasks with 1 assignee, found {}", tasks.len());
}

#[tokio::test]
async fn test_filter_by_nullable_field_estimated_hours() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_diverse_test_tasks(&app).await;

    // Test filtering by estimated_hours = null
    let filter_param = url_escape::encode_component("{\"estimated_hours\":null}");
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tasks?filter={filter_param}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with null estimated_hours
    for task in &tasks {
        assert!(task.estimated_hours.is_none(), "Task '{}' should have null estimated_hours", task.title);
    }
    
    // We created 1 task with null estimated_hours
    assert_eq!(tasks.len(), 1, "Should find 1 task with null estimated_hours, found {}", tasks.len());
}

#[tokio::test]
async fn test_filter_by_nullable_field_description() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_diverse_test_tasks(&app).await;

    // Test filtering by description = null
    let filter_param = url_escape::encode_component("{\"description\":null}");
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tasks?filter={filter_param}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with null description
    for task in &tasks {
        assert!(task.description.is_none(), "Task '{}' should have null description", task.title);
    }
    
    // We created 1 task with null description
    assert_eq!(tasks.len(), 1, "Should find 1 task with null description, found {}", tasks.len());
}

#[tokio::test]
async fn test_complex_multi_type_filtering() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_diverse_test_tasks(&app).await;

    // Test complex filter: completed = false AND priority = "low" (case insensitive)
    let filter_param = url_escape::encode_component("{\"completed\":false,\"priority\":\"low\"}");
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tasks?filter={filter_param}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find incomplete tasks with low priority
    for task in &tasks {
        assert!(!task.completed, "Task '{}' should not be completed", task.title);
        assert_eq!(task.priority, Priority::Low, "Task '{}' should have low priority", task.title);
    }
    
    // We created 1 incomplete low priority task
    assert_eq!(tasks.len(), 1, "Should find 1 incomplete low priority task, found {}", tasks.len());
}

#[tokio::test]
async fn test_complex_enum_and_numeric_filtering() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_diverse_test_tasks(&app).await;

    // Test complex filter: status = "done" AND points = 100 (case insensitive)
    let filter_param = url_escape::encode_component("{\"status\":\"done\",\"points\":100}");
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tasks?filter={filter_param}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find done tasks with 100 points
    for task in &tasks {
        assert_eq!(task.status, Status::Done, "Task '{}' should be done", task.title);
        assert_eq!(task.points, 100, "Task '{}' should have 100 points", task.title);
    }
    
    // We created 1 done task with 100 points
    assert_eq!(tasks.len(), 1, "Should find 1 done task with 100 points, found {}", tasks.len());
}

#[tokio::test]
async fn test_sorting_with_different_data_types() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_diverse_test_tasks(&app).await;

    // Test sorting by score (float) DESC
    let sort_param = url_escape::encode_component("[\"score\",\"DESC\"]");
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tasks?sort={sort_param}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should be sorted by score descending
    let mut prev_score = f64::INFINITY;
    for task in &tasks {
        assert!(task.score <= prev_score, 
            "Tasks should be sorted by score DESC, but {} (score: {}) came after score {}", 
            task.title, task.score, prev_score);
        prev_score = task.score;
    }
    
    assert_eq!(tasks.len(), 5, "Should return all 5 tasks");
}

#[tokio::test]
async fn test_sorting_by_enum_priority() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_diverse_test_tasks(&app).await;

    // Test sorting by priority ASC
    let sort_param = url_escape::encode_component("[\"priority\",\"ASC\"]");
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tasks?sort={sort_param}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should be sorted by priority (alphabetically in this case)
    // Expected order: high, low, medium, urgent, urgent
    let expected_priorities = [Priority::High, Priority::Low, Priority::Medium, Priority::Urgent, Priority::Urgent];
    
    for (i, task) in tasks.iter().enumerate() {
        assert_eq!(task.priority, expected_priorities[i], 
            "Task at position {} should have priority {:?}, got {:?}", 
            i, expected_priorities[i], task.priority);
    }
    
    assert_eq!(tasks.len(), 5, "Should return all 5 tasks");
}

#[tokio::test]
async fn test_invalid_enum_value_filtering() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_diverse_test_tasks(&app).await;

    // Test filtering by invalid priority value
    let filter_param = url_escape::encode_component("{\"priority\":\"invalid_priority\"}");
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tasks?filter={filter_param}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    
    // This should either return no results or handle the error gracefully
    // The exact behavior depends on how crudcrate handles invalid enum values
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap_or_default();
    
    // Should return no results for invalid enum value
    assert_eq!(tasks.len(), 0, "Should find no tasks with invalid priority value, found {}", tasks.len());
}