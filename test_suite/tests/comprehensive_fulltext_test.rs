// @doc-link-file fulltext
// Comprehensive Fulltext Search Tests
// Tests remaining examples from docs/src/features/fulltext-search.md
// Goal: 100% documentation-test alignment (currently at 71%)

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use crate::common::customer::CustomerList;
use common::{setup_test_app, setup_test_db};

// =============================================================================
// COMBINED OPERATIONS TESTS (fulltext-search.md lines 122-133)
// These were missing from existing tests
// =============================================================================

#[tokio::test]
async fn test_fulltext_search_with_sort_as_documented() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customers with searchable content at different times
    let customers = [
        json!({"name": "Senior Rust Developer", "email": "rust1@example.com"}),
        json!({"name": "Junior Rust Engineer", "email": "rust2@example.com"}),
        json!({"name": "Rust Architect", "email": "rust3@example.com"}),
    ];

    for customer_data in &customers {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(customer_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        // Small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Test: Search + sort (docs example: q=rust&sort=["created_at","DESC"])
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?filter=%7B%22q%22%3A%22Rust%22%7D&sort=%5B%22created_at%22%2C%22DESC%22%5D")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    // Should find all 3 Rust-related customers
    assert_eq!(customers.len(), 3, "Should find all Rust-related customers");
    assert!(
        customers
            .iter()
            .all(|c| c.name.to_lowercase().contains("rust")),
        "All results should contain 'Rust'"
    );

    // Verify they're sorted by created_at DESC (newest first)
    for i in 0..customers.len() - 1 {
        assert!(
            customers[i].created_at >= customers[i + 1].created_at,
            "Results should be sorted by created_at DESC"
        );
    }
}

#[tokio::test]
async fn test_fulltext_search_with_paginate_as_documented() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create many customers with "Developer" in name
    for i in 0..15 {
        let customer_data = json!({
            "name": format!("Developer {}", i),
            "email": format!("dev{}@example.com", i)
        });
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(customer_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test: Search + paginate (docs example: q=rust&sort=["created_at","DESC"]&range=[0,9])
    // Adapted: q=Developer&range=[0,4] (first 5 results)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?filter=%7B%22q%22%3A%22Developer%22%7D&range=%5B0%2C4%5D")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    // Should return exactly 5 customers (range [0,4])
    assert_eq!(
        customers.len(),
        5,
        "Search with pagination should respect range limit"
    );
    assert!(
        customers.iter().all(|c| c.name.contains("Developer")),
        "All paginated results should match search query"
    );
}

// =============================================================================
// BACKEND-SPANNING BEHAVIOUR TESTS
//
// These run against whatever DATABASE_URL the suite is pointed at (SQLite by
// default, real Postgres/MySQL in CI), so the assertions hold on every backend.
// The fulltext path is case-insensitive *substring* matching on all three:
// Postgres uses `ILIKE` (search.rs build_postgres_fulltext_condition), MySQL and
// SQLite use `UPPER(col) LIKE UPPER(?)`. There is no `pg_trgm`/trigram similarity
// and no fuzzy/typo tolerance anywhere — these tests pin that actual behaviour.
// Data is ASCII so case-folding is symmetric on all backends (non-ASCII folding
// is ASCII-only on MySQL/SQLite, a documented limitation, and is not exercised
// here on purpose).
// =============================================================================

fn q_filter_uri(q: &str) -> String {
    // Percent-encode `filter={"q":"<q>"}`. Test queries are ASCII words, so only
    // the JSON punctuation and spaces need encoding.
    let filter = format!("{{\"q\":\"{q}\"}}");
    let encoded: String = filter
        .chars()
        .map(|c| match c {
            '{' => "%7B".to_string(),
            '}' => "%7D".to_string(),
            '"' => "%22".to_string(),
            ':' => "%3A".to_string(),
            ' ' => "%20".to_string(),
            other => other.to_string(),
        })
        .collect();
    format!("/customers?filter={encoded}")
}

async fn post_customer(app: &axum::Router, name: &str, email: &str) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/customers")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "name": name, "email": email }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "seed customer {name}");
}

async fn search_customers(app: &axum::Router, q: &str) -> Vec<CustomerList> {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(q_filter_uri(q))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "search q={q}");
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

/// `q` search matches case-insensitively, in both directions (query case and
/// stored case are independent), as a substring — on every backend. This is the
/// assertion the previously-empty database-specific tests never made.
#[tokio::test]
async fn test_fulltext_case_insensitive_substring_match() {
    let db = setup_test_db().await.expect("setup db");
    let app = setup_test_app(&db);

    post_customer(&app, "Senior RUST Developer", "dev@example.com").await;
    post_customer(&app, "rust junior engineer", "junior@example.com").await;
    post_customer(&app, "PostgreSQL Database Admin", "dba@example.com").await;

    // Lowercase query matches names stored in mixed/upper case.
    let lower = search_customers(&app, "rust").await;
    assert_eq!(
        lower.len(),
        2,
        "lowercase 'rust' matches both rust customers"
    );
    assert!(
        lower.iter().all(|c| c.name.to_lowercase().contains("rust")),
        "every match actually contains 'rust'"
    );

    // Uppercase query returns the same set: folding is applied to both sides.
    let upper = search_customers(&app, "RUST").await;
    assert_eq!(upper.len(), 2, "uppercase 'RUST' matches the same two rows");

    // Substring (not whole-word) and case-insensitive against a mixed-case name.
    let database = search_customers(&app, "database").await;
    assert_eq!(
        database.len(),
        1,
        "'database' matches 'PostgreSQL Database Admin'"
    );
    assert_eq!(database[0].name, "PostgreSQL Database Admin");

    // A mid-token substring still matches (ILIKE/UPPER-LIKE are %term%).
    let infix = search_customers(&app, "ust").await;
    assert_eq!(infix.len(), 2, "'ust' is an infix of both 'rust' rows");
}

/// The fulltext path is exact substring matching, NOT fuzzy/trigram. A typo does
/// not match — pinning the real behaviour against the (incorrect) docs that once
/// claimed Postgres `pg_trgm` typo tolerance. Holds on every backend.
#[tokio::test]
async fn test_fulltext_is_substring_not_fuzzy() {
    let db = setup_test_db().await.expect("setup db");
    let app = setup_test_app(&db);

    post_customer(&app, "Rust Programming Expert", "prog@example.com").await;

    // Exact substring matches.
    assert_eq!(search_customers(&app, "programming").await.len(), 1);
    // A one-character typo must NOT match: there is no trigram/fuzzy similarity.
    assert_eq!(
        search_customers(&app, "progamming").await.len(),
        0,
        "typo 'progamming' must not fuzzy-match 'Programming' (substring, not trigram)"
    );
    // An unrelated term does not match either.
    assert_eq!(search_customers(&app, "python").await.len(), 0);
}

// =============================================================================
// COMPREHENSIVE COMBINED TEST (fulltext-search.md lines 124-132)
// =============================================================================

#[tokio::test]
async fn test_fulltext_search_filter_sort_paginate_as_documented() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create diverse customers
    let customers = [
        // Active developers
        json!({"name": "Alice Developer", "email": "alice@dev.com"}),
        json!({"name": "Bob Developer", "email": "bob@dev.com"}),
        json!({"name": "Charlie Developer", "email": "charlie@dev.com"}),
        json!({"name": "Diana Developer", "email": "diana@dev.com"}),
        json!({"name": "Eve Developer", "email": "eve@dev.com"}),
        // Inactive developers
        json!({"name": "Frank Developer Inactive", "email": "frank@dev.com"}),
        // Non-developers
        json!({"name": "Grace Designer", "email": "grace@design.com"}),
    ];

    for customer_data in &customers {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(customer_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Test: Search + filter + sort + paginate
    // (docs example: q=rust&filter={"status":"published"}&sort=["created_at","DESC"])
    // Adapted: q=Developer&filter={"name":"Alice"}&sort=["created_at","DESC"]&range=[0,4]
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?filter=%7B%22q%22%3A%22Developer%22%2C%22name%22%3A%22Alice%22%7D&sort=%5B%22created_at%22%2C%22DESC%22%5D&range=%5B0%2C4%5D")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    // Should find Alice Developer only
    assert_eq!(
        customers.len(),
        1,
        "Combined search+filter should find Alice Developer"
    );
    assert!(
        customers[0].name.contains("Alice") && customers[0].name.contains("Developer"),
        "Result should match both search and filter"
    );
}

// =============================================================================
// SPECIAL CHARACTERS & SAFETY TESTS (fulltext-search.md lines 190-200)
// These are already well-tested, but included for completeness
// =============================================================================

#[tokio::test]
async fn test_fulltext_special_characters_documented_examples() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create customers with special characters in names
    let customers = [
        json!({"name": "C++ Developer", "email": "cpp@example.com"}),
        json!({"name": "Node.js Engineer", "email": "node@example.com"}),
        json!({"name": "user@email Developer", "email": "useratemail@example.com"}),
    ];

    for customer_data in &customers {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(customer_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test: Search with special characters (docs examples from lines 194-198)

    // Test 1: c++ (docs example: q=c++)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?filter=%7B%22q%22%3A%22c%2B%2B%22%7D") // {"q":"c++"}
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Search with 'c++' should work safely"
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let _customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();
    // Note: May or may not find results depending on sanitization, but should not error

    // Test 2: node.js (docs example: q=node.js)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?filter=%7B%22q%22%3A%22node.js%22%7D") // {"q":"node.js"}
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Search with 'node.js' should work safely"
    );

    // Test 3: user@email (docs example: q=user@email)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?filter=%7B%22q%22%3A%22user%40email%22%7D") // {"q":"user@email"}
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Search with 'user@email' should work safely"
    );

    // All special character searches should work without errors
}

// =============================================================================
// PERFORMANCE TIP DOCUMENTATION TEST (fulltext-search.md lines 150-158)
// =============================================================================

#[tokio::test]
async fn test_fulltext_optimized_search_pattern_as_documented() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let app = setup_test_app(&db);

    // Create many customers in different categories
    for i in 0..50 {
        let (name, _category) = if i < 10 {
            (format!("Rust Developer {}", i), "programming")
        } else if i < 20 {
            (format!("Python Developer {}", i), "programming")
        } else {
            (format!("Designer {}", i), "design")
        };

        let customer_data = json!({"name": name, "email": format!("user{}@example.com", i)});
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(customer_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Test: Optimized search (docs example from lines 150-158)
    // ❌ Slow: fulltext search on all items
    // GET /items?q=rust
    //
    // ✅ Fast: filter first, then search
    // GET /items?q=rust&filter={"category":"programming"}&range=[0,19]

    // Simulate the "fast" optimized pattern
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/customers?filter=%7B%22q%22%3A%22Rust%22%2C%22name%22%3A%22Developer%22%7D&range=%5B0%2C19%5D")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let customers: Vec<CustomerList> = serde_json::from_slice(&body).unwrap();

    // Should find Rust developers only, paginated
    assert!(
        customers.len() <= 20,
        "Optimized search should respect pagination"
    );
    assert!(
        customers
            .iter()
            .all(|c| c.name.to_lowercase().contains("rust")
                && c.name.to_lowercase().contains("developer")),
        "Optimized search should apply both search and filter"
    );
}
