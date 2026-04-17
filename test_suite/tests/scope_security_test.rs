/// Scope Security Tests
///
/// Validates that `ScopeCondition` + `exclude(scoped)` correctly:
/// - Filters private records from list and get_one endpoints
/// - Strips `is_private` from all response JSON (top-level and nested joins)
/// - Blocks all write operations (create, update, delete, batch) with 403
/// - Strips scoped columns from filterable/sortable lists
/// - Returns correct Content-Range counts reflecting the scoped condition
///
/// These tests use real SQLite-in-memory databases — no mocks.
mod common;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt;

use common::{setup_scoped_app, setup_test_app, setup_test_db};

fn encode_filter(filter: &Value) -> String {
    url_escape::encode_component(&filter.to_string()).to_string()
}

/// POST a record via the unscoped (admin) app, return status + JSON body.
/// Note: is_private defaults to false on create (exclude(create)), use admin_update to make private.
async fn admin_post(app: &axum::Router, path: &str, payload: Value) -> (StatusCode, Value) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

/// GET via any app, return status + JSON body + headers
async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value, axum::http::HeaderMap) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&body).unwrap_or(Value::Null),
        headers,
    )
}

/// PUT (update) a record via the unscoped (admin) app
async fn admin_update(app: &axum::Router, path: &str, payload: Value) -> (StatusCode, Value) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

/// Create a record and then mark it private via update. Returns the created JSON.
async fn create_private(app: &axum::Router, path: &str, payload: Value) -> Value {
    let (s, created) = admin_post(app, path, payload).await;
    assert_eq!(s, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap();
    let (s, updated) =
        admin_update(app, &format!("{path}/{id}"), json!({"is_private": true})).await;
    assert_eq!(s, StatusCode::OK);
    updated
}

/// Send an arbitrary method request, return status
async fn send(app: &axum::Router, method: &str, uri: &str, body: Option<Value>) -> StatusCode {
    let b = body
        .map(|v| Body::from(v.to_string()))
        .unwrap_or(Body::empty());
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(b)
                .unwrap(),
        )
        .await
        .unwrap();
    resp.status()
}

// =============================================================================
// 1. List: private records excluded
// =============================================================================

#[tokio::test]
async fn scope_list_excludes_private_records() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    admin_post(
        &admin,
        "/customers",
        json!({"name": "Public Alice", "email": "alice@example.com"}),
    )
    .await;
    create_private(
        &admin,
        "/customers",
        json!({"name": "Private Bob", "email": "bob@example.com"}),
    )
    .await;

    let (status, body, _) = get_json(&scoped, "/customers").await;
    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 1, "Only the public customer should be visible");
    assert_eq!(items[0]["name"], "Public Alice");
}

// =============================================================================
// 2. get_one: 404 for private records
// =============================================================================

#[tokio::test]
async fn scope_get_one_404_for_private() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    let created = create_private(
        &admin,
        "/customers",
        json!({"name": "Secret", "email": "s@example.com"}),
    )
    .await;
    let id = created["id"].as_str().unwrap();

    let (status, _, _) = get_json(&scoped, &format!("/customers/{id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// =============================================================================
// 3. get_one: public record accessible
// =============================================================================

#[tokio::test]
async fn scope_get_one_ok_for_public() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    let (_, created) = admin_post(
        &admin,
        "/customers",
        json!({"name": "Visible", "email": "v@example.com"}),
    )
    .await;
    let id = created["id"].as_str().unwrap();

    let (status, body, _) = get_json(&scoped, &format!("/customers/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "Visible");
}

// =============================================================================
// 4. is_private absent from scoped list response
// =============================================================================

#[tokio::test]
async fn scope_list_response_omits_is_private() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    admin_post(
        &admin,
        "/customers",
        json!({"name": "Test", "email": "t@example.com"}),
    )
    .await;

    let (_, body, _) = get_json(&scoped, "/customers").await;
    let items = body.as_array().unwrap();
    assert!(!items.is_empty());
    assert!(
        items[0].get("is_private").is_none(),
        "is_private must not appear in scoped list response, got keys: {:?}",
        items[0].as_object().unwrap().keys().collect::<Vec<_>>()
    );
}

// =============================================================================
// 5. is_private absent from scoped get_one response
// =============================================================================

#[tokio::test]
async fn scope_get_one_response_omits_is_private() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    let (_, created) = admin_post(
        &admin,
        "/customers",
        json!({"name": "Test", "email": "t@example.com"}),
    )
    .await;
    let id = created["id"].as_str().unwrap();

    let (_, body, _) = get_json(&scoped, &format!("/customers/{id}")).await;
    assert!(
        body.get("is_private").is_none(),
        "is_private must not appear in scoped get_one response"
    );
}

// =============================================================================
// 6. is_private absent from NESTED join entities in scoped response
// =============================================================================

#[tokio::test]
async fn scope_nested_join_omits_is_private() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    // Create a public customer
    let (_, cust) = admin_post(
        &admin,
        "/customers",
        json!({"name": "Owner", "email": "o@example.com"}),
    )
    .await;
    let cust_id = cust["id"].as_str().unwrap();

    // Create a public vehicle under that customer
    admin_post(
        &admin,
        "/vehicles",
        json!({"customer_id": cust_id, "make": "Toyota", "model": "Corolla", "year": 2020, "vin": "VIN001"}),
    )
    .await;

    // List customers — the joined vehicles array should also omit is_private
    let (_, body, _) = get_json(&scoped, "/customers").await;
    let customers = body.as_array().unwrap();
    assert!(!customers.is_empty());
    let vehicles = customers[0]["vehicles"].as_array().unwrap();
    assert!(!vehicles.is_empty(), "Vehicle should be joined in response");
    assert!(
        vehicles[0].get("is_private").is_none(),
        "is_private must not appear in nested vehicle within scoped customer list response, got keys: {:?}",
        vehicles[0].as_object().unwrap().keys().collect::<Vec<_>>()
    );
}

// =============================================================================
// 7. is_private absent from nested joins in get_one response
// =============================================================================

#[tokio::test]
async fn scope_get_one_nested_join_omits_is_private() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    let (_, cust) = admin_post(
        &admin,
        "/customers",
        json!({"name": "Owner2", "email": "o2@example.com"}),
    )
    .await;
    let cust_id = cust["id"].as_str().unwrap();

    admin_post(
        &admin,
        "/vehicles",
        json!({"customer_id": cust_id, "make": "Honda", "model": "Civic", "year": 2021, "vin": "VIN002"}),
    )
    .await;

    let (status, body, _) = get_json(&scoped, &format!("/customers/{cust_id}")).await;
    assert_eq!(status, StatusCode::OK);
    let vehicles = body["vehicles"].as_array().unwrap();
    assert!(!vehicles.is_empty());
    assert!(
        vehicles[0].get("is_private").is_none(),
        "is_private must not appear in nested vehicle within scoped get_one response"
    );
}

// =============================================================================
// 8. Admin (unscoped) response DOES include is_private
// =============================================================================

#[tokio::test]
async fn admin_response_includes_is_private() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);

    let created = create_private(
        &admin,
        "/customers",
        json!({"name": "Admin View", "email": "a@example.com"}),
    )
    .await;
    let id = created["id"].as_str().unwrap();

    // List
    let (_, body, _) = get_json(&admin, "/customers").await;
    let items = body.as_array().unwrap();
    assert!(
        items[0].get("is_private").is_some(),
        "Admin list should include is_private"
    );

    // get_one
    let (_, body, _) = get_json(&admin, &format!("/customers/{id}")).await;
    assert!(
        body.get("is_private").is_some(),
        "Admin get_one should include is_private"
    );
}

// =============================================================================
// 9. Write: POST (create) blocked with 403
// =============================================================================

#[tokio::test]
async fn scope_create_blocked() {
    let db = setup_test_db().await.unwrap();
    let scoped = setup_scoped_app(&db);

    let status = send(
        &scoped,
        "POST",
        "/customers",
        Some(json!({"name": "Hack", "email": "h@x.com"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// =============================================================================
// 10. Write: PUT (update) blocked with 403
// =============================================================================

#[tokio::test]
async fn scope_update_blocked() {
    let db = setup_test_db().await.unwrap();
    let scoped = setup_scoped_app(&db);

    let status = send(
        &scoped,
        "PUT",
        "/customers/00000000-0000-0000-0000-000000000001",
        Some(json!({"name": "Hacked"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// =============================================================================
// 11. Write: DELETE blocked with 403
// =============================================================================

#[tokio::test]
async fn scope_delete_blocked() {
    let db = setup_test_db().await.unwrap();
    let scoped = setup_scoped_app(&db);

    let status = send(
        &scoped,
        "DELETE",
        "/customers/00000000-0000-0000-0000-000000000001",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// =============================================================================
// 12. Write: batch POST blocked with 403
// =============================================================================

#[tokio::test]
async fn scope_batch_create_blocked() {
    let db = setup_test_db().await.unwrap();
    let scoped = setup_scoped_app(&db);

    let status = send(
        &scoped,
        "POST",
        "/customers/batch",
        Some(json!([{"name": "A", "email": "a@x.com"}])),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// =============================================================================
// 13. Write: batch DELETE blocked with 403
// =============================================================================

#[tokio::test]
async fn scope_batch_delete_blocked() {
    let db = setup_test_db().await.unwrap();
    let scoped = setup_scoped_app(&db);

    let status = send(
        &scoped,
        "DELETE",
        "/customers/batch",
        Some(json!(["00000000-0000-0000-0000-000000000001"])),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// =============================================================================
// 14. Write: batch PATCH (update) blocked with 403
// =============================================================================

#[tokio::test]
async fn scope_batch_update_blocked() {
    let db = setup_test_db().await.unwrap();
    let scoped = setup_scoped_app(&db);

    let status = send(
        &scoped,
        "PATCH",
        "/customers/batch",
        Some(json!([{"id": "00000000-0000-0000-0000-000000000001", "name": "Hacked"}])),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

// =============================================================================
// 15. Filter on is_private ignored in scoped context
// =============================================================================

#[tokio::test]
async fn scope_filter_on_excluded_column_ignored() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    admin_post(
        &admin,
        "/customers",
        json!({"name": "Public", "email": "p@example.com"}),
    )
    .await;
    create_private(
        &admin,
        "/customers",
        json!({"name": "Private", "email": "pr@example.com"}),
    )
    .await;

    // Attempt to filter is_private=true on the scoped endpoint — should be silently ignored
    let filter = encode_filter(&json!({"is_private": true}));
    let (status, body, _) = get_json(&scoped, &format!("/customers?filter={filter}")).await;
    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().unwrap();
    // The filter on is_private should be dropped; scope still filters to public only
    assert_eq!(
        items.len(),
        1,
        "Filtering on is_private should be ignored in scoped context, only public records shown"
    );
    assert_eq!(items[0]["name"], "Public");
}

// =============================================================================
// 16. Filter is_private=false in scoped context also ignored (same result)
// =============================================================================

#[tokio::test]
async fn scope_filter_is_private_false_ignored() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    admin_post(
        &admin,
        "/customers",
        json!({"name": "Pub1", "email": "p1@example.com"}),
    )
    .await;
    admin_post(
        &admin,
        "/customers",
        json!({"name": "Pub2", "email": "p2@example.com"}),
    )
    .await;

    // With the filter
    let filter = encode_filter(&json!({"is_private": false}));
    let (_, with_filter, _) = get_json(&scoped, &format!("/customers?filter={filter}")).await;
    // Without the filter
    let (_, without_filter, _) = get_json(&scoped, "/customers").await;

    assert_eq!(
        with_filter.as_array().unwrap().len(),
        without_filter.as_array().unwrap().len(),
        "Filter on is_private=false should be ignored, giving same results as no filter"
    );
}

// =============================================================================
// 17. Content-Range header reflects scoped count
// =============================================================================

#[tokio::test]
async fn scope_content_range_reflects_scoped_count() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    // Create 2 public + 3 private
    for i in 0..2 {
        admin_post(
            &admin,
            "/customers",
            json!({"name": format!("Pub{i}"), "email": format!("pub{i}@ex.com")}),
        )
        .await;
    }
    for i in 0..3 {
        create_private(
            &admin,
            "/customers",
            json!({"name": format!("Priv{i}"), "email": format!("priv{i}@ex.com")}),
        )
        .await;
    }

    // Admin sees all 5
    let (_, _, admin_headers) = get_json(&admin, "/customers").await;
    let admin_range = admin_headers
        .get("content-range")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        admin_range.contains("/5"),
        "Admin Content-Range should show total 5, got: {admin_range}"
    );

    // Scoped sees only 2
    let (_, _, scoped_headers) = get_json(&scoped, "/customers").await;
    let scoped_range = scoped_headers
        .get("content-range")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        scoped_range.contains("/2"),
        "Scoped Content-Range should show total 2, got: {scoped_range}"
    );
}

// =============================================================================
// 18. Mixed public/private children in joins — only public shown
// =============================================================================

#[tokio::test]
async fn scope_join_filters_private_children_from_parent_list() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    let (_, cust) = admin_post(
        &admin,
        "/customers",
        json!({"name": "Parent", "email": "p@ex.com"}),
    )
    .await;
    let cust_id = cust["id"].as_str().unwrap();

    // Create 1 public + 1 private vehicle
    admin_post(
        &admin,
        "/vehicles",
        json!({"customer_id": cust_id, "make": "Public", "model": "Car", "year": 2020, "vin": "V1"}),
    )
    .await;
    create_private(
        &admin,
        "/vehicles",
        json!({"customer_id": cust_id, "make": "Private", "model": "Car", "year": 2021, "vin": "V2"}),
    )
    .await;

    // Scoped vehicle list should only show the public one
    let (_, body, _) = get_json(&scoped, "/vehicles").await;
    let vehicles = body.as_array().unwrap();
    assert_eq!(vehicles.len(), 1);
    assert_eq!(vehicles[0]["make"], "Public");
}

// =============================================================================
// 19. Nonexistent UUID returns 404, same as private (no timing leak)
// =============================================================================

#[tokio::test]
async fn scope_nonexistent_and_private_both_404() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    let created = create_private(
        &admin,
        "/customers",
        json!({"name": "Hidden", "email": "h@x.com"}),
    )
    .await;
    let private_id = created["id"].as_str().unwrap();
    let fake_id = "00000000-0000-0000-0000-ffffffffffff";

    let (s1, _, _) = get_json(&scoped, &format!("/customers/{private_id}")).await;
    let (s2, _, _) = get_json(&scoped, &format!("/customers/{fake_id}")).await;
    assert_eq!(s1, StatusCode::NOT_FOUND);
    assert_eq!(s2, StatusCode::NOT_FOUND);
}

// =============================================================================
// 20. HEAD request allowed in scoped context
// =============================================================================

#[tokio::test]
async fn scope_head_request_allowed() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    admin_post(
        &admin,
        "/customers",
        json!({"name": "HeadTest", "email": "ht@ex.com"}),
    )
    .await;

    let resp = scoped
        .clone()
        .oneshot(
            Request::builder()
                .method("HEAD")
                .uri("/customers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// =============================================================================
// 21. Scoped parent get_one excludes private children from Vec joins
// =============================================================================

#[tokio::test]
async fn scope_parent_get_one_excludes_private_join_children() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    // Create a customer (customers don't have is_private, so always visible)
    let (_, cust) = admin_post(
        &admin,
        "/customers",
        json!({"name": "Parent", "email": "p@ex.com"}),
    )
    .await;
    let cust_id = cust["id"].as_str().unwrap();

    // Create 1 public + 1 private vehicle under this customer
    admin_post(
        &admin,
        "/vehicles",
        json!({"customer_id": cust_id, "make": "Public", "model": "Car", "year": 2020, "vin": "PUB1"}),
    )
    .await;
    create_private(
        &admin,
        "/vehicles",
        json!({"customer_id": cust_id, "make": "Private", "model": "Car", "year": 2021, "vin": "PRV1"}),
    )
    .await;

    // Admin get_one sees both vehicles in the join
    let (s, body, _) = get_json(&admin, &format!("/customers/{cust_id}")).await;
    assert_eq!(s, StatusCode::OK);
    let vehicles = body["vehicles"].as_array().expect("should have vehicles");
    assert_eq!(vehicles.len(), 2, "Admin should see both vehicles");

    // Scoped get_one should only see the public vehicle
    let (s, body, _) = get_json(&scoped, &format!("/customers/{cust_id}")).await;
    assert_eq!(s, StatusCode::OK);
    let vehicles = body["vehicles"].as_array().expect("should have vehicles");
    assert_eq!(
        vehicles.len(),
        1,
        "Scoped get_one should only show public vehicle, got: {vehicles:?}"
    );
    assert_eq!(vehicles[0]["make"], "Public");
}

// =============================================================================
// 22. Scoped parent list excludes private children from Vec joins
// =============================================================================

#[tokio::test]
async fn scope_parent_list_excludes_private_join_children() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    let (_, cust) = admin_post(
        &admin,
        "/customers",
        json!({"name": "ListParent", "email": "lp@ex.com"}),
    )
    .await;
    let cust_id = cust["id"].as_str().unwrap();

    admin_post(
        &admin,
        "/vehicles",
        json!({"customer_id": cust_id, "make": "Visible", "model": "X", "year": 2022, "vin": "VIS1"}),
    )
    .await;
    create_private(
        &admin,
        "/vehicles",
        json!({"customer_id": cust_id, "make": "Hidden", "model": "Y", "year": 2023, "vin": "HID1"}),
    )
    .await;

    // Scoped list — customer's vehicles join should only contain the public one
    let (s, body, _) = get_json(&scoped, "/customers").await;
    assert_eq!(s, StatusCode::OK);
    let customers = body.as_array().unwrap();
    assert_eq!(customers.len(), 1);
    let vehicles = customers[0]["vehicles"]
        .as_array()
        .expect("should have vehicles");
    assert_eq!(
        vehicles.len(),
        1,
        "Scoped list should only show public vehicle in join"
    );
    assert_eq!(vehicles[0]["make"], "Visible");
}

// =============================================================================
// 23. Sort on scoped-excluded column silently ignored (no error, returns data)
// =============================================================================

#[tokio::test]
async fn scope_sort_on_excluded_column_ignored() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    admin_post(
        &admin,
        "/customers",
        json!({"name": "Zulu", "email": "z@example.com"}),
    )
    .await;
    admin_post(
        &admin,
        "/customers",
        json!({"name": "Alpha", "email": "a@example.com"}),
    )
    .await;

    // Attempt to sort by is_private — should not error, returns 200 with data
    let sort = url_escape::encode_component(r#"["is_private","DESC"]"#).to_string();
    let (status, body, _) = get_json(&scoped, &format!("/customers?sort={sort}")).await;
    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 2, "Both public customers should be returned");

    // Verify is_private is NOT in response (scoped field stripping still works)
    assert!(
        items[0].get("is_private").is_none(),
        "is_private must not appear in scoped response even when used as sort column"
    );
}

// =============================================================================
// 24. Search (q= / like) in scoped context only returns public matches
// =============================================================================

#[tokio::test]
async fn scope_search_respects_scope_filter() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    // Create 2 public and 1 private customer with similar names
    admin_post(
        &admin,
        "/customers",
        json!({"name": "Alice Public", "email": "ap@example.com"}),
    )
    .await;
    admin_post(
        &admin,
        "/customers",
        json!({"name": "Bob Public", "email": "bp@example.com"}),
    )
    .await;
    create_private(
        &admin,
        "/customers",
        json!({"name": "Alice Secret", "email": "as@example.com"}),
    )
    .await;

    // Search for "Alice" in scoped context — should only return the public Alice
    // Customer `name` is `like_filterable`, so `{"name": "Alice"}` does LIKE matching
    let filter = encode_filter(&json!({"name": "Alice"}));
    let (status, body, _) = get_json(&scoped, &format!("/customers?filter={filter}")).await;
    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().unwrap();
    assert_eq!(
        items.len(),
        1,
        "Scoped search for 'Alice' should only return public match"
    );
    assert_eq!(items[0]["name"], "Alice Public");

    // Admin search should find both Alices
    let (_, admin_body, _) = get_json(&admin, &format!("/customers?filter={filter}")).await;
    let admin_items = admin_body.as_array().unwrap();
    assert_eq!(
        admin_items.len(),
        2,
        "Admin search for 'Alice' should find both public and private"
    );
}

// =============================================================================
// 25. Atomic scope check: flip to private then verify 404
// =============================================================================

#[tokio::test]
async fn scope_get_one_atomic_single_query() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    // Create public customer
    let (_, created) = admin_post(
        &admin,
        "/customers",
        json!({"name": "WillGoPrivate", "email": "flip@example.com"}),
    )
    .await;
    let id = created["id"].as_str().unwrap();

    // Confirm visible in scoped
    let (s, _, _) = get_json(&scoped, &format!("/customers/{id}")).await;
    assert_eq!(s, StatusCode::OK, "Public customer should be visible");

    // Flip to private
    admin_update(
        &admin,
        &format!("/customers/{id}"),
        json!({"is_private": true}),
    )
    .await;

    // Scoped get_one must return 404 — the scope condition is part of the fetch,
    // not a separate verification query (atomic single-query check).
    let (s, _, _) = get_json(&scoped, &format!("/customers/{id}")).await;
    assert_eq!(
        s,
        StatusCode::NOT_FOUND,
        "After flipping to private, scoped get_one must return 404"
    );
}

// =============================================================================
// 26. Scoped get_one preserves join loading with scope filtering
// =============================================================================

#[tokio::test]
async fn scope_get_one_scoped_preserves_join_loading() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    let (_, cust) = admin_post(
        &admin,
        "/customers",
        json!({"name": "JoinParent", "email": "jp@example.com"}),
    )
    .await;
    let cust_id = cust["id"].as_str().unwrap();

    // Create 2 public + 1 private vehicle
    admin_post(
        &admin,
        "/vehicles",
        json!({"customer_id": cust_id, "make": "Pub1", "model": "X", "year": 2020, "vin": "JP1"}),
    )
    .await;
    admin_post(
        &admin,
        "/vehicles",
        json!({"customer_id": cust_id, "make": "Pub2", "model": "Y", "year": 2021, "vin": "JP2"}),
    )
    .await;
    create_private(
        &admin,
        "/vehicles",
        json!({"customer_id": cust_id, "make": "Priv1", "model": "Z", "year": 2022, "vin": "JP3"}),
    )
    .await;

    // Scoped get_one: should return customer with only 2 public vehicles
    let (s, body, _) = get_json(&scoped, &format!("/customers/{cust_id}")).await;
    assert_eq!(s, StatusCode::OK);

    let vehicles = body["vehicles"]
        .as_array()
        .expect("should have vehicles join");
    assert_eq!(
        vehicles.len(),
        2,
        "Scoped get_one should only load public vehicles in join, got: {vehicles:?}"
    );

    // All vehicles should omit is_private
    for v in vehicles {
        assert!(
            v.get("is_private").is_none(),
            "is_private must not appear in scoped join vehicle: {v:?}"
        );
    }
}

// =============================================================================
// 27. Scoped get_one for private record: 404, no data leak
// =============================================================================

#[tokio::test]
async fn scope_get_one_scoped_404_does_not_leak_existence() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    // Create private customer with a vehicle
    let private_cust = create_private(
        &admin,
        "/customers",
        json!({"name": "SecretOwner", "email": "secret@example.com"}),
    )
    .await;
    let cust_id = private_cust["id"].as_str().unwrap();

    admin_post(
        &admin,
        "/vehicles",
        json!({"customer_id": cust_id, "make": "Hidden", "model": "Car", "year": 2020, "vin": "HID99"}),
    )
    .await;

    // Scoped get_one must return 404 — NOT 200 with empty joins
    let (s, body, _) = get_json(&scoped, &format!("/customers/{cust_id}")).await;
    assert_eq!(s, StatusCode::NOT_FOUND, "Private customer must return 404");

    // Body must NOT contain any customer fields
    assert!(
        body.get("name").is_none(),
        "404 response must not leak customer name, got: {body}"
    );
    assert!(
        body.get("vehicles").is_none(),
        "404 response must not leak vehicles join, got: {body}"
    );
}

// =============================================================================
// 28. Unscoped get_one regression: still returns full data
// =============================================================================

#[tokio::test]
async fn scope_get_one_unscoped_still_works() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);

    let (_, cust) = admin_post(
        &admin,
        "/customers",
        json!({"name": "FullData", "email": "full@example.com"}),
    )
    .await;
    let cust_id = cust["id"].as_str().unwrap();

    admin_post(
        &admin,
        "/vehicles",
        json!({"customer_id": cust_id, "make": "AdminCar", "model": "X", "year": 2024, "vin": "ADM1"}),
    )
    .await;

    // Admin (unscoped) get_one: full data including is_private
    let (s, body, _) = get_json(&admin, &format!("/customers/{cust_id}")).await;
    assert_eq!(s, StatusCode::OK);
    assert!(
        body.get("is_private").is_some(),
        "Admin get_one must include is_private field"
    );

    let vehicles = body["vehicles"].as_array().expect("should have vehicles");
    assert_eq!(vehicles.len(), 1, "Admin should see all vehicles");
    assert!(
        vehicles[0].get("is_private").is_some(),
        "Admin vehicle join must include is_private field"
    );
}
