// Non-standard FK Join Test
// Tests that joins work correctly when the FK column name doesn't follow
// the {ParentStructName}Id convention.
//
// The Book entity has `author_ref` instead of `author_id` as its FK to Author.
// CrudCrate must resolve the correct FK column from SeaORM's RelationDef
// rather than guessing based on the parent struct name.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::author::AuthorResponse;
use common::{setup_test_app, setup_test_db};

#[tokio::test]
async fn test_nonstandard_fk_get_one_includes_books() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create an author
    let request = Request::builder()
        .method("POST")
        .uri("/authors")
        .header("content-type", "application/json")
        .body(Body::from(json!({"name": "Ursula K. Le Guin"}).to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let author: AuthorResponse = serde_json::from_slice(&body).expect("Failed to parse author");
    let author_id = author.id;

    // Create two books with author_ref pointing to the author
    for title in ["The Left Hand of Darkness", "The Dispossessed"] {
        let request = Request::builder()
            .method("POST")
            .uri("/books")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({"title": title, "author_ref": author_id}).to_string(),
            ))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::CREATED,
            "Failed to create book '{title}'"
        );
    }

    // GET the author — books should be populated via join(one)
    let request = Request::builder()
        .method("GET")
        .uri(format!("/authors/{author_id}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let author: AuthorResponse = serde_json::from_slice(&body).expect("Failed to parse author");

    assert_eq!(
        author.books.len(),
        2,
        "get_one should include 2 books via join(one) with non-standard FK (author_ref)"
    );
}

#[tokio::test]
async fn test_nonstandard_fk_get_all_includes_books() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create an author
    let request = Request::builder()
        .method("POST")
        .uri("/authors")
        .header("content-type", "application/json")
        .body(Body::from(json!({"name": "Philip K. Dick"}).to_string()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let author: AuthorResponse = serde_json::from_slice(&body).expect("Failed to parse author");
    let author_id = author.id;

    // Create a book
    let request = Request::builder()
        .method("POST")
        .uri("/books")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"title": "Do Androids Dream of Electric Sheep?", "author_ref": author_id})
                .to_string(),
        ))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // GET all authors — books should be populated via join(all) batch loading
    let request = Request::builder()
        .method("GET")
        .uri("/authors")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8_lossy(&body);
    let parsed: serde_json::Value =
        serde_json::from_str(&body_str).expect("Failed to parse list response");

    let authors = parsed
        .as_array()
        .or_else(|| parsed["data"].as_array())
        .expect("Expected array or { data: [...] } response");
    let our_author = authors
        .iter()
        .find(|a| a["id"] == author_id.to_string())
        .expect("Author not found in list");

    let books = our_author["books"]
        .as_array()
        .expect("Expected books array");
    assert_eq!(
        books.len(),
        1,
        "get_all batch loading should include books via join(all) with non-standard FK (author_ref)"
    );
    assert_eq!(books[0]["title"], "Do Androids Dream of Electric Sheep?");
}
