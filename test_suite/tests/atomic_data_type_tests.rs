use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{
    setup_task_app, setup_test_db_with_tasks,
    task_entity::{Priority, Status, Task},
};

/// Setup function that creates diverse test data once and returns the app with data
async fn setup_app_with_test_data() -> axum::Router {
    let db = setup_test_db_with_tasks()
        .await
        .expect("Failed to setup test database");
    let app = setup_task_app(db);

    // Create diverse test data
    let test_tasks = vec![
        json!({
            "title": "Alpha Work",
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
        json!({
            "title": "Beta Personal",
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
        json!({
            "title": "Gamma Todo",
            "description": null,
            "completed": false,
            "priority": "Low",
            "status": "Todo",
            "score": 0.0,
            "points": 25,
            "estimated_hours": null,
            "assignee_count": 1,
            "is_public": true
        }),
        json!({
            "title": "Delta Migration",
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
        json!({
            "title": "Echo Fixes",
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
        if !status.is_success() {
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            panic!(
                "Task creation failed with status {}: {}",
                status,
                String::from_utf8_lossy(&body)
            );
        }
    }

    app
}

/// Helper function to execute a filter and return tasks
async fn filter_tasks(app: &axum::Router, filter_json: &str) -> Vec<Task> {
    let filter_param = url_escape::encode_component(filter_json);
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
    serde_json::from_slice(&body).unwrap()
}

// ===== BOOLEAN FILTERING TESTS =====

#[tokio::test]
async fn test_filter_boolean_completed_true() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"completed\":true}").await;

    for task in &tasks {
        assert!(task.completed, "Task '{}' should be completed", task.title);
    }
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 completed tasks, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_boolean_completed_false() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"completed\":false}").await;

    for task in &tasks {
        assert!(
            !task.completed,
            "Task '{}' should not be completed",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        3,
        "Should find 3 incomplete tasks, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_boolean_is_public_true() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"is_public\":true}").await;

    for task in &tasks {
        assert!(task.is_public, "Task '{}' should be public", task.title);
    }
    assert_eq!(
        tasks.len(),
        3,
        "Should find 3 public tasks, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_boolean_is_public_false() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"is_public\":false}").await;

    for task in &tasks {
        assert!(!task.is_public, "Task '{}' should be private", task.title);
    }
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 private tasks, found {}",
        tasks.len()
    );
}

// ===== ENUM FILTERING TESTS =====

#[tokio::test]
async fn test_filter_enum_priority_high() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"priority\":\"High\"}").await;

    for task in &tasks {
        assert_eq!(
            task.priority,
            Priority::High,
            "Task '{}' should have High priority",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 High priority task, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_enum_priority_urgent() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"priority\":\"Urgent\"}").await;

    for task in &tasks {
        assert_eq!(
            task.priority,
            Priority::Urgent,
            "Task '{}' should have Urgent priority",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 Urgent priority tasks, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_enum_priority_medium() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"priority\":\"Medium\"}").await;

    for task in &tasks {
        assert_eq!(
            task.priority,
            Priority::Medium,
            "Task '{}' should have Medium priority",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 Medium priority task, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_enum_priority_low() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"priority\":\"Low\"}").await;

    for task in &tasks {
        assert_eq!(
            task.priority,
            Priority::Low,
            "Task '{}' should have Low priority",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 Low priority task, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_enum_status_done() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"status\":\"Done\"}").await;

    for task in &tasks {
        assert_eq!(
            task.status,
            Status::Done,
            "Task '{}' should have Done status",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 Done status tasks, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_enum_status_inprogress() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"status\":\"InProgress\"}").await;

    for task in &tasks {
        assert_eq!(
            task.status,
            Status::InProgress,
            "Task '{}' should have InProgress status",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 InProgress status task, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_enum_status_todo() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"status\":\"Todo\"}").await;

    for task in &tasks {
        assert_eq!(
            task.status,
            Status::Todo,
            "Task '{}' should have Todo status",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 Todo status task, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_enum_status_cancelled() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"status\":\"Cancelled\"}").await;

    for task in &tasks {
        assert_eq!(
            task.status,
            Status::Cancelled,
            "Task '{}' should have Cancelled status",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 Cancelled status task, found {}",
        tasks.len()
    );
}

// ===== STRING FILTERING TESTS =====

#[tokio::test]
async fn test_filter_string_title_alpha() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"title\":\"Alpha\"}").await;

    for task in &tasks {
        assert!(
            task.title.contains("Alpha"),
            "Task '{}' should contain 'Alpha'",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 Alpha title task, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_string_title_work() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"title\":\"Work\"}").await;

    for task in &tasks {
        assert!(
            task.title.contains("Work"),
            "Task '{}' should contain 'Work'",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 Work title task, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_string_description_user() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"description\":\"user\"}").await;

    for task in &tasks {
        assert!(
            task.description.as_ref().unwrap().contains("user"),
            "Task '{}' should contain 'user' in description",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 task with 'user' in description, found {}",
        tasks.len()
    );
}

// ===== NUMERIC FILTERING TESTS =====

#[tokio::test]
async fn test_filter_float_score_exact_95_5() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"score\":95.5}").await;

    for task in &tasks {
        assert!(
            (task.score - 95.5).abs() < f64::EPSILON,
            "Task '{}' should have score 95.5, got {}",
            task.title,
            task.score
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 task with score 95.5, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_float_score_exact_67_25() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"score\":67.25}").await;

    for task in &tasks {
        assert!(
            (task.score - 67.25).abs() < f64::EPSILON,
            "Task '{}' should have score 67.25, got {}",
            task.title,
            task.score
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 task with score 67.25, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_float_score_exact_zero() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"score\":0.0}").await;

    for task in &tasks {
        assert!(
            (task.score - 0.0).abs() < f64::EPSILON,
            "Task '{}' should have score 0.0, got {}",
            task.title,
            task.score
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 task with score 0.0, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_integer_points_100() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"points\":100}").await;

    for task in &tasks {
        assert_eq!(
            task.points, 100,
            "Task '{}' should have 100 points, got {}",
            task.title, task.points
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 task with 100 points, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_integer_points_75() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"points\":75}").await;

    for task in &tasks {
        assert_eq!(
            task.points, 75,
            "Task '{}' should have 75 points, got {}",
            task.title, task.points
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 task with 75 points, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_integer_points_25() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"points\":25}").await;

    for task in &tasks {
        assert_eq!(
            task.points, 25,
            "Task '{}' should have 25 points, got {}",
            task.title, task.points
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 task with 25 points, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_small_integer_assignee_count_1() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"assignee_count\":1}").await;

    for task in &tasks {
        assert_eq!(
            task.assignee_count, 1,
            "Task '{}' should have 1 assignee, got {}",
            task.title, task.assignee_count
        );
    }
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 tasks with 1 assignee, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_small_integer_assignee_count_3() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"assignee_count\":3}").await;

    for task in &tasks {
        assert_eq!(
            task.assignee_count, 3,
            "Task '{}' should have 3 assignees, got {}",
            task.title, task.assignee_count
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 task with 3 assignees, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_small_integer_assignee_count_5() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"assignee_count\":5}").await;

    for task in &tasks {
        assert_eq!(
            task.assignee_count, 5,
            "Task '{}' should have 5 assignees, got {}",
            task.title, task.assignee_count
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 task with 5 assignees, found {}",
        tasks.len()
    );
}

// ===== NULLABLE FIELD TESTS =====

#[tokio::test]
async fn test_filter_nullable_description_null() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"description\":null}").await;

    for task in &tasks {
        assert!(
            task.description.is_none(),
            "Task '{}' should have null description",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 task with null description, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_nullable_estimated_hours_null() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"estimated_hours\":null}").await;

    for task in &tasks {
        assert!(
            task.estimated_hours.is_none(),
            "Task '{}' should have null estimated_hours",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 task with null estimated_hours, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_nullable_estimated_hours_8_5() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"estimated_hours\":8.5}").await;

    for task in &tasks {
        match task.estimated_hours {
            Some(hours) => assert!(
                (hours - 8.5).abs() < f32::EPSILON,
                "Task '{}' should have 8.5 estimated_hours, got {}",
                task.title,
                hours
            ),
            None => panic!(
                "Task '{}' should have 8.5 estimated_hours, got null",
                task.title
            ),
        }
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 task with 8.5 estimated_hours, found {}",
        tasks.len()
    );
}

// ===== MULTI-FIELD FILTERING TESTS =====

#[tokio::test]
async fn test_filter_multi_completed_and_priority() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"completed\":true,\"priority\":\"Urgent\"}").await;

    for task in &tasks {
        assert!(task.completed, "Task '{}' should be completed", task.title);
        assert_eq!(
            task.priority,
            Priority::Urgent,
            "Task '{}' should have Urgent priority",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 completed Urgent task, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_multi_status_and_points() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"status\":\"Done\",\"points\":50}").await;

    for task in &tasks {
        assert_eq!(
            task.status,
            Status::Done,
            "Task '{}' should have Done status",
            task.title
        );
        assert_eq!(
            task.points, 50,
            "Task '{}' should have 50 points",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 Done task with 50 points, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_multi_is_public_and_assignee_count() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"is_public\":false,\"assignee_count\":3}").await;

    for task in &tasks {
        assert!(!task.is_public, "Task '{}' should be private", task.title);
        assert_eq!(
            task.assignee_count, 3,
            "Task '{}' should have 3 assignees",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        1,
        "Should find 1 private task with 3 assignees, found {}",
        tasks.len()
    );
}

// ===== EDGE CASE TESTS =====

#[tokio::test]
async fn test_filter_nonexistent_field() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"nonexistent_field\":\"value\"}").await;

    // Should return all tasks since field doesn't exist (filter ignored)
    assert_eq!(
        tasks.len(),
        5,
        "Should return all 5 tasks for nonexistent field, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_invalid_enum_value() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"priority\":\"InvalidPriority\"}").await;

    // Should return no tasks for invalid enum value
    assert_eq!(
        tasks.len(),
        0,
        "Should find no tasks with invalid priority value, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_empty_string() {
    let app = setup_app_with_test_data().await;
    let tasks = filter_tasks(&app, "{\"title\":\"\"}").await;

    // Should return no tasks for empty string (no titles are empty)
    assert_eq!(
        tasks.len(),
        0,
        "Should find no tasks with empty title, found {}",
        tasks.len()
    );
}
