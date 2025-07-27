use axum::http::{Request, StatusCode};
use axum::body::Body;
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_db_with_tasks, setup_task_app, task_entity::Task};

/// Comprehensive tests for React Admin numeric comparison operators
/// Testing: _gte, _lte, _gt, _lt, _neq operators for all numeric types
/// Based on React Admin filtering conventions: https://marmelab.com/react-admin/FilteringTutorial.html

async fn create_numeric_test_tasks(app: &axum::Router) -> Vec<Task> {
    let test_tasks = vec![
        // Task with very low values
        json!({
            "title": "Low Value Task",
            "description": "Task with minimal values",
            "completed": false,
            "priority": "Low",
            "status": "Todo",
            "score": 10.5,           // Low score
            "points": 5,             // Low points  
            "estimated_hours": 1.0,  // Low hours
            "assignee_count": 1,     // Min assignees
            "is_public": true
        }),
        // Task with medium values
        json!({
            "title": "Medium Value Task",
            "description": "Task with average values",
            "completed": false,
            "priority": "Medium",
            "status": "InProgress",
            "score": 50.0,           // Medium score
            "points": 50,            // Medium points
            "estimated_hours": 8.0,  // Medium hours
            "assignee_count": 3,     // Medium assignees
            "is_public": true
        }),
        // Task with high values
        json!({
            "title": "High Value Task", 
            "description": "Task with maximum values",
            "completed": true,
            "priority": "High",
            "status": "Done",
            "score": 95.7,           // High score
            "points": 100,           // High points
            "estimated_hours": 40.0, // High hours
            "assignee_count": 10,    // Max assignees
            "is_public": false
        }),
        // Task with exact boundary values
        json!({
            "title": "Boundary Value Task",
            "description": "Task with exact test values",
            "completed": true,
            "priority": "Urgent",
            "status": "Done",
            "score": 75.0,           // Exact boundary
            "points": 75,            // Exact boundary
            "estimated_hours": 20.0, // Exact boundary
            "assignee_count": 5,     // Exact boundary
            "is_public": true
        }),
        // Task with zero/null values
        json!({
            "title": "Zero Value Task",
            "description": null,
            "completed": false,
            "priority": "Low",
            "status": "Cancelled",
            "score": 0.0,            // Zero score
            "points": 0,             // Zero points
            "estimated_hours": null, // Null hours
            "assignee_count": 0,     // Zero assignees (invalid but testing)
            "is_public": false
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
        
        if !status.is_success() {
            panic!("Task creation failed with status {}: {}", status, String::from_utf8_lossy(&body));
        }
        
        let body_str = String::from_utf8_lossy(&body);
        if body_str.is_empty() {
            panic!("Empty response body from task creation");
        }
        
        let task: Task = serde_json::from_slice(&body)
            .map_err(|e| format!("Failed to parse task JSON '{}': {}", body_str, e))
            .unwrap();
        created_tasks.push(task);
    }

    created_tasks
}

// ===== GREATER THAN OR EQUAL (_gte) TESTS =====

#[tokio::test]
async fn test_filter_score_gte_float() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_numeric_test_tasks(&app).await;

    // Test score_gte = 50.0 (should find Medium, High, Boundary tasks)
    let filter_param = url_escape::encode_component("{\"score_gte\":50.0}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with score >= 50.0
    for task in &tasks {
        assert!(task.score >= 50.0, "Task '{}' has score {} which is < 50.0", task.title, task.score);
    }
    
    // Should find 3 tasks: Medium (50.0), High (95.7), Boundary (75.0)
    assert_eq!(tasks.len(), 3, "Should find 3 tasks with score >= 50.0, found {}", tasks.len());
}

#[tokio::test]
async fn test_filter_points_gte_integer() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_numeric_test_tasks(&app).await;

    // Test points_gte = 50 (should find Medium, High, Boundary tasks)
    let filter_param = url_escape::encode_component("{\"points_gte\":50}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with points >= 50
    for task in &tasks {
        assert!(task.points >= 50, "Task '{}' has points {} which is < 50", task.title, task.points);
    }
    
    // Should find 3 tasks: Medium (50), High (100), Boundary (75)
    assert_eq!(tasks.len(), 3, "Should find 3 tasks with points >= 50, found {}", tasks.len());
}

#[tokio::test]
async fn test_filter_assignee_count_gte_small_integer() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_numeric_test_tasks(&app).await;

    // Test assignee_count_gte = 3 (should find Medium, High, Boundary tasks)
    let filter_param = url_escape::encode_component("{\"assignee_count_gte\":3}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with assignee_count >= 3
    for task in &tasks {
        assert!(task.assignee_count >= 3, "Task '{}' has assignee_count {} which is < 3", task.title, task.assignee_count);
    }
    
    // Should find 3 tasks: Medium (3), High (10), Boundary (5)
    assert_eq!(tasks.len(), 3, "Should find 3 tasks with assignee_count >= 3, found {}", tasks.len());
}

// ===== LESS THAN OR EQUAL (_lte) TESTS =====

#[tokio::test]
async fn test_filter_score_lte_float() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_numeric_test_tasks(&app).await;

    // Test score_lte = 50.0 (should find Low, Medium, Zero tasks)
    let filter_param = url_escape::encode_component("{\"score_lte\":50.0}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with score <= 50.0
    for task in &tasks {
        assert!(task.score <= 50.0, "Task '{}' has score {} which is > 50.0", task.title, task.score);
    }
    
    // Should find 3 tasks: Low (10.5), Medium (50.0), Zero (0.0)
    assert_eq!(tasks.len(), 3, "Should find 3 tasks with score <= 50.0, found {}", tasks.len());
}

#[tokio::test]
async fn test_filter_points_lte_integer() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_numeric_test_tasks(&app).await;

    // Test points_lte = 50 (should find Low, Medium, Zero tasks)
    let filter_param = url_escape::encode_component("{\"points_lte\":50}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with points <= 50
    for task in &tasks {
        assert!(task.points <= 50, "Task '{}' has points {} which is > 50", task.title, task.points);
    }
    
    // Should find 3 tasks: Low (5), Medium (50), Zero (0)
    assert_eq!(tasks.len(), 3, "Should find 3 tasks with points <= 50, found {}", tasks.len());
}

// ===== GREATER THAN (_gt) TESTS =====

#[tokio::test]
async fn test_filter_score_gt_float() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_numeric_test_tasks(&app).await;

    // Test score_gt = 50.0 (should find High, Boundary tasks, NOT Medium)
    let filter_param = url_escape::encode_component("{\"score_gt\":50.0}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with score > 50.0 (NOT equal to 50.0)
    for task in &tasks {
        assert!(task.score > 50.0, "Task '{}' has score {} which is <= 50.0", task.title, task.score);
    }
    
    // Should find 2 tasks: High (95.7), Boundary (75.0) - NOT Medium (50.0)
    assert_eq!(tasks.len(), 2, "Should find 2 tasks with score > 50.0, found {}", tasks.len());
}

#[tokio::test]
async fn test_filter_points_gt_integer() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_numeric_test_tasks(&app).await;

    // Test points_gt = 50 (should find High, Boundary tasks, NOT Medium)
    let filter_param = url_escape::encode_component("{\"points_gt\":50}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with points > 50 (NOT equal to 50)
    for task in &tasks {
        assert!(task.points > 50, "Task '{}' has points {} which is <= 50", task.title, task.points);
    }
    
    // Should find 2 tasks: High (100), Boundary (75) - NOT Medium (50)
    assert_eq!(tasks.len(), 2, "Should find 2 tasks with points > 50, found {}", tasks.len());
}

// ===== LESS THAN (_lt) TESTS =====

#[tokio::test]
async fn test_filter_score_lt_float() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_numeric_test_tasks(&app).await;

    // Test score_lt = 50.0 (should find Low, Zero tasks, NOT Medium)
    let filter_param = url_escape::encode_component("{\"score_lt\":50.0}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with score < 50.0 (NOT equal to 50.0)
    for task in &tasks {
        assert!(task.score < 50.0, "Task '{}' has score {} which is >= 50.0", task.title, task.score);
    }
    
    // Should find 2 tasks: Low (10.5), Zero (0.0) - NOT Medium (50.0)
    assert_eq!(tasks.len(), 2, "Should find 2 tasks with score < 50.0, found {}", tasks.len());
}

#[tokio::test]
async fn test_filter_assignee_count_lt_small_integer() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_numeric_test_tasks(&app).await;

    // Test assignee_count_lt = 3 (should find Low, Zero tasks, NOT Medium)
    let filter_param = url_escape::encode_component("{\"assignee_count_lt\":3}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with assignee_count < 3 (NOT equal to 3)
    for task in &tasks {
        assert!(task.assignee_count < 3, "Task '{}' has assignee_count {} which is >= 3", task.title, task.assignee_count);
    }
    
    // Should find 2 tasks: Low (1), Zero (0) - NOT Medium (3)
    assert_eq!(tasks.len(), 2, "Should find 2 tasks with assignee_count < 3, found {}", tasks.len());
}

// ===== NOT EQUAL (_neq) TESTS =====

#[tokio::test]
async fn test_filter_score_neq_float() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_numeric_test_tasks(&app).await;

    // Test score_neq = 50.0 (should find all tasks EXCEPT Medium)
    let filter_param = url_escape::encode_component("{\"score_neq\":50.0}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with score != 50.0
    for task in &tasks {
        assert!(task.score != 50.0, "Task '{}' has score {} which equals 50.0", task.title, task.score);
    }
    
    // Should find 4 tasks: Low (10.5), High (95.7), Boundary (75.0), Zero (0.0) - NOT Medium (50.0)
    assert_eq!(tasks.len(), 4, "Should find 4 tasks with score != 50.0, found {}", tasks.len());
}

#[tokio::test]
async fn test_filter_points_neq_integer() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_numeric_test_tasks(&app).await;

    // Test points_neq = 50 (should find all tasks EXCEPT Medium)
    let filter_param = url_escape::encode_component("{\"points_neq\":50}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with points != 50
    for task in &tasks {
        assert!(task.points != 50, "Task '{}' has points {} which equals 50", task.title, task.points);
    }
    
    // Should find 4 tasks: Low (5), High (100), Boundary (75), Zero (0) - NOT Medium (50)
    assert_eq!(tasks.len(), 4, "Should find 4 tasks with points != 50, found {}", tasks.len());
}

#[tokio::test]
async fn test_filter_boolean_neq() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_numeric_test_tasks(&app).await;

    // Test completed_neq = false (should find only completed tasks)
    let filter_param = url_escape::encode_component("{\"completed_neq\":false}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with completed != false (i.e., completed = true)
    for task in &tasks {
        assert!(task.completed, "Task '{}' should be completed (completed != false)", task.title);
    }
    
    // Should find 2 tasks: High (true), Boundary (true)
    assert_eq!(tasks.len(), 2, "Should find 2 completed tasks (completed != false), found {}", tasks.len());
}

// ===== RANGE QUERIES (BETWEEN) TESTS =====

#[tokio::test]
async fn test_filter_score_range_between() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_numeric_test_tasks(&app).await;

    // Test score between 25.0 and 80.0 (score_gte=25.0 AND score_lte=80.0)
    let filter_param = url_escape::encode_component("{\"score_gte\":25.0,\"score_lte\":80.0}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with 25.0 <= score <= 80.0
    for task in &tasks {
        assert!(task.score >= 25.0 && task.score <= 80.0, 
            "Task '{}' has score {} which is not between 25.0 and 80.0", task.title, task.score);
    }
    
    // Should find 2 tasks: Medium (50.0), Boundary (75.0)
    // NOT Low (10.5), NOT High (95.7), NOT Zero (0.0)
    assert_eq!(tasks.len(), 2, "Should find 2 tasks with score between 25.0 and 80.0, found {}", tasks.len());
}

#[tokio::test]
async fn test_filter_points_range_between() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_numeric_test_tasks(&app).await;

    // Test points between 20 and 80 (points_gte=20 AND points_lte=80)
    let filter_param = url_escape::encode_component("{\"points_gte\":20,\"points_lte\":80}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with 20 <= points <= 80
    for task in &tasks {
        assert!(task.points >= 20 && task.points <= 80, 
            "Task '{}' has points {} which is not between 20 and 80", task.title, task.points);
    }
    
    // Should find 2 tasks: Medium (50), Boundary (75)
    // NOT Low (5), NOT High (100), NOT Zero (0)
    assert_eq!(tasks.len(), 2, "Should find 2 tasks with points between 20 and 80, found {}", tasks.len());
}

// ===== NULLABLE FIELD COMPARISON TESTS =====

#[tokio::test]
async fn test_filter_estimated_hours_gte_nullable() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_numeric_test_tasks(&app).await;

    // Test estimated_hours_gte = 10.0 (should only find non-null values >= 10.0)
    let filter_param = url_escape::encode_component("{\"estimated_hours_gte\":10.0}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks with estimated_hours >= 10.0 (ignoring null values)
    for task in &tasks {
        match task.estimated_hours {
            Some(hours) => assert!(hours >= 10.0, 
                "Task '{}' has estimated_hours {} which is < 10.0", task.title, hours),
            None => panic!("Task '{}' has null estimated_hours, should be filtered out", task.title),
        }
    }
    
    // Should find 2 tasks: High (40.0), Boundary (20.0)
    // NOT Low (1.0), NOT Medium (8.0), NOT Zero (null)
    assert_eq!(tasks.len(), 2, "Should find 2 tasks with estimated_hours >= 10.0, found {}", tasks.len());
}

// ===== COMBINATION TESTS =====

#[tokio::test]
async fn test_filter_multiple_comparison_operators() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_numeric_test_tasks(&app).await;

    // Test complex filter: score_gte=40.0 AND points_lte=80 AND assignee_count_neq=10
    let filter_param = url_escape::encode_component("{\"score_gte\":40.0,\"points_lte\":80,\"assignee_count_neq\":10}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find tasks that match ALL conditions
    for task in &tasks {
        assert!(task.score >= 40.0, "Task '{}' has score {} which is < 40.0", task.title, task.score);
        assert!(task.points <= 80, "Task '{}' has points {} which is > 80", task.title, task.points);
        assert!(task.assignee_count != 10, "Task '{}' has assignee_count {} which equals 10", task.title, task.assignee_count);
    }
    
    // Should find 2 tasks: Medium (score=50.0, points=50, assignee_count=3), Boundary (score=75.0, points=75, assignee_count=5)
    // NOT Low (score=10.5 < 40.0), NOT High (assignee_count=10), NOT Zero (score=0.0 < 40.0)
    assert_eq!(tasks.len(), 2, "Should find 2 tasks matching all conditions, found {}", tasks.len());
}

// ===== EDGE CASE TESTS =====

#[tokio::test]
async fn test_filter_zero_boundary_values() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_numeric_test_tasks(&app).await;

    // Test score_gt = 0.0 (should exclude only zero values)
    let filter_param = url_escape::encode_component("{\"score_gt\":0.0}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap();
    
    // Should find all tasks except the zero task
    for task in &tasks {
        assert!(task.score > 0.0, "Task '{}' has score {} which is <= 0.0", task.title, task.score);
    }
    
    // Should find 4 tasks: Low (10.5), Medium (50.0), High (95.7), Boundary (75.0) - NOT Zero (0.0)
    assert_eq!(tasks.len(), 4, "Should find 4 tasks with score > 0.0, found {}", tasks.len());
}

#[tokio::test]
async fn test_filter_invalid_comparison_operator() {
    let db = setup_test_db_with_tasks().await.expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_numeric_test_tasks(&app).await;

    // Test unsupported operator: score_invalid = 50.0 (should be treated as regular field or ignored)
    let filter_param = url_escape::encode_component("{\"score_invalid\":50.0}");
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    
    // This should either:
    // 1. Return all tasks (filter ignored)
    // 2. Return no tasks (field doesn't exist)
    // 3. Return an error (invalid operator)
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Vec<Task> = serde_json::from_slice(&body).unwrap_or_default();
    
    // The exact behavior depends on how crudcrate handles invalid operators
    // This test documents the current behavior
    println!("Invalid operator returned {} tasks", tasks.len());
    
    // For now, we just verify it doesn't crash
    assert!(tasks.len() <= 5, "Should not return more tasks than exist");
}