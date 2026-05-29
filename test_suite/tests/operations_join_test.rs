// Scenario: an entity with both `operations = ...` and `join(one, all)` fields.
// Expected behaviour: join loading still populates related entities on get_one
// and get_all, even though operations is set.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_test_app, setup_test_db};

#[tokio::test]
async fn test_operations_get_one_includes_joins() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    let request = Request::builder()
        .method("POST")
        .uri("/managed_authors")
        .header("content-type", "application/json")
        .body(Body::from(json!({"name": "Octavia Butler"}).to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let author: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let author_id = author["id"].as_str().unwrap();

    for title in ["Kindred", "Parable of the Sower"] {
        let request = Request::builder()
            .method("POST")
            .uri("/books")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({"title": title, "author_ref": author_id}).to_string(),
            ))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    let request = Request::builder()
        .method("GET")
        .uri(format!("/managed_authors/{author_id}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let author: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let books = author["books"]
        .as_array()
        .expect("get_one response should have a books array when operations + join(one) are set");
    assert_eq!(
        books.len(),
        2,
        "get_one should load 2 books via join(one) even with operations set"
    );
}

#[tokio::test]
async fn test_operations_get_all_includes_joins() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    let request = Request::builder()
        .method("POST")
        .uri("/managed_authors")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"name": "Samuel Delany"}).to_string(),
        ))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let author: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let author_id = author["id"].as_str().unwrap();

    let request = Request::builder()
        .method("POST")
        .uri("/books")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"title": "Dhalgren", "author_ref": author_id}).to_string(),
        ))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let request = Request::builder()
        .method("GET")
        .uri("/managed_authors")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let authors = parsed
        .as_array()
        .or_else(|| parsed["data"].as_array())
        .expect("Expected array or { data: [...] } response");
    let our_author = authors
        .iter()
        .find(|a| a["id"] == author_id)
        .expect("Author not found in list");

    let books = our_author["books"]
        .as_array()
        .expect("get_all response should have a books array when operations + join(all) are set");
    assert_eq!(
        books.len(),
        1,
        "get_all batch loading should include books via join(all) even with operations set"
    );
    assert_eq!(books[0]["title"], "Dhalgren");
}
