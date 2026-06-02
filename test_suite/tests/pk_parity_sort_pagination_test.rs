//! Sorting + pagination parity for a non-UUID (auto-increment `i32`) primary key.
//!
//! Proves that an entity whose PK is an integer behaves EXACTLY like a Uuid-PK
//! entity across the sort and pagination surface of the generated list handler:
//! React-Admin JSON `sort=["col","ASC"/"DESC"]`, default ordering, `range`-based
//! windowing with `Content-Range` headers, `page`/`per_page`, and `max_page_size`
//! capping.
//!
//! These mirror the Uuid-PK tests in:
//! - `sort_http_coverage_test.rs` (sort by name/id, ASC + DESC)
//! - `pagination_content_range_http_coverage_test.rs` (range windowing,
//!   `page`/`per_page`, `max_page_size` cap, Content-Range)
//!
//! The decisive non-UUID assertion: sorting by the integer `id` column orders
//! NUMERICALLY (2 before 10), not lexically ("10" before "2") as a string PK
//! would. Self-contained: entity, schema, seeding, and router all live here.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use tower::ServiceExt;

pub mod thing {
    use super::*;

    // Integer auto-increment PK (no on_create — the DB assigns it), excluded
    // from create/update. `name` and `id` are both sortable. max_page_size is
    // set below the seeded row count so the cap is observable independently of
    // the data volume, exactly as the Uuid pagination coverage test does.
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "pps_things")]
    #[crudcrate(
        generate_router,
        api_struct = "Thing",
        name_plural = "pps_things",
        max_page_size = 20,
        derive_partial_eq
    )]
    pub struct Model {
        #[sea_orm(primary_key)]
        #[crudcrate(primary_key, exclude(create, update))]
        pub id: i32,

        #[crudcrate(filterable, sortable)]
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

use thing::{Thing, ThingList};

const SEED_COUNT: i32 = 25;

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let db = Database::connect("sqlite::memory:").await?;
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);
    db.execute(backend.build(&schema.create_table_from_entity(thing::Entity)))
        .await?;
    Ok(db)
}

/// Seed `SEED_COUNT` rows. Names are zero-padded so they sort identically
/// whether compared lexically or numerically — the seeded order matches the
/// auto-assigned id order so default/id-sort orderings are predictable.
async fn seed(db: &DatabaseConnection) {
    let items: Vec<thing::ThingCreate> = (0..SEED_COUNT)
        .map(|i| thing::ThingCreate {
            name: format!("thing-{i:02}"),
        })
        .collect();
    let created = Thing::create_many(db, items)
        .await
        .expect("seed create_many should succeed");
    assert_eq!(created.len(), SEED_COUNT as usize);
    // The DB assigns sequential integer ids starting at 1.
    let mut ids: Vec<i32> = created.iter().map(|t| t.id).collect();
    ids.sort_unstable();
    assert_eq!(ids, (1..=SEED_COUNT).collect::<Vec<_>>());
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/things", Thing::router(db).into())
}

/// GET the list endpoint with a raw query string; return status, the
/// Content-Range header value, and the parsed JSON body.
async fn list(
    db: &DatabaseConnection,
    query: &str,
) -> (StatusCode, Option<String>, serde_json::Value) {
    let uri = format!("/things?{query}");
    let resp = app(db)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let content_range = resp
        .headers()
        .get("content-range")
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    (status, content_range, json)
}

/// Parse the body as a typed list of `ThingList`. Confirms the integer `id`
/// round-trips through JSON deserialisation as well as through the wire.
fn typed(json: &serde_json::Value) -> Vec<ThingList> {
    serde_json::from_value(json.clone()).expect("list body should deserialise to Vec<ThingList>")
}

fn ids(json: &serde_json::Value) -> Vec<i32> {
    typed(json).iter().map(|t| t.id).collect()
}

fn names(json: &serde_json::Value) -> Vec<String> {
    typed(json).iter().map(|t| t.name.clone()).collect()
}

// React-Admin encodes the sort param as a URL-encoded JSON array. These match
// the encodings used in the Uuid sort/pagination coverage tests.
const SORT_NAME_ASC: &str = "sort=%5B%22name%22%2C%22ASC%22%5D";
const SORT_NAME_DESC: &str = "sort=%5B%22name%22%2C%22DESC%22%5D";
const SORT_ID_ASC: &str = "sort=%5B%22id%22%2C%22ASC%22%5D";
const SORT_ID_DESC: &str = "sort=%5B%22id%22%2C%22DESC%22%5D";

// Without explicit pagination the handler applies a default page size of 10
// (identical for a Uuid PK). Sort-ordering tests that need the full 25-row
// ordering ask for a wide-enough range — capped at max_page_size (20) — so the
// ordering is observable across the digit boundary (single vs double digit).
const WIDE_RANGE: &str = "range=%5B0%2C19%5D";

// =============================================================================
// Sorting parity (mirrors sort_http_coverage_test.rs json_array_sort_* tests).
// =============================================================================

#[tokio::test]
async fn sort_name_asc_and_desc() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // First 20 names ASC (capped by max_page_size): thing-00..thing-19.
    let (status, _, json) = list(&db, &format!("{WIDE_RANGE}&{SORT_NAME_ASC}")).await;
    assert_eq!(status, StatusCode::OK);
    let asc = names(&json);
    let expected_asc: Vec<String> = (0..20).map(|i| format!("thing-{i:02}")).collect();
    assert_eq!(asc, expected_asc, "name ASC must be alphabetical");

    // First 20 names DESC: thing-24..thing-05.
    let (status, _, json) = list(&db, &format!("{WIDE_RANGE}&{SORT_NAME_DESC}")).await;
    assert_eq!(status, StatusCode::OK);
    let desc = names(&json);
    let expected_desc: Vec<String> = (5..SEED_COUNT)
        .rev()
        .map(|i| format!("thing-{i:02}"))
        .collect();
    assert_eq!(desc, expected_desc, "name DESC is reverse-alphabetical");
}

/// The non-UUID hinge: sorting by the integer PK column orders numerically.
/// With 25 rows the ids span single and double digits, so a lexical (string)
/// ordering would place 10 before 2; numeric ordering must place 2 before 10.
#[tokio::test]
async fn sort_id_asc_is_numeric_not_lexical() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // Wide range so the slice spans the single/double-digit boundary.
    let (status, _, json) = list(&db, &format!("{WIDE_RANGE}&{SORT_ID_ASC}")).await;
    assert_eq!(status, StatusCode::OK);
    let asc = ids(&json);
    assert_eq!(
        asc,
        (1..=20).collect::<Vec<_>>(),
        "id ASC must be numeric: 1,2,...,9,10,...,20"
    );

    // Pin the numeric-vs-lexical distinction explicitly: 2 precedes 10.
    let pos_2 = asc.iter().position(|&i| i == 2).unwrap();
    let pos_10 = asc.iter().position(|&i| i == 10).unwrap();
    assert!(
        pos_2 < pos_10,
        "integer id 2 must sort before 10 (numeric, not lexical): {asc:?}"
    );
}

#[tokio::test]
async fn sort_id_desc_is_numeric() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    // First 20 ids DESC (capped): 25,24,...,6.
    let (status, _, json) = list(&db, &format!("{WIDE_RANGE}&{SORT_ID_DESC}")).await;
    assert_eq!(status, StatusCode::OK);
    let desc = ids(&json);
    assert_eq!(
        desc,
        (6..=SEED_COUNT).rev().collect::<Vec<_>>(),
        "id DESC must be numeric descending: 25,24,...,6"
    );

    let pos_10 = desc.iter().position(|&i| i == 10).unwrap();
    let pos_25 = desc.iter().position(|&i| i == 25).unwrap();
    assert!(
        pos_25 < pos_10,
        "in DESC, integer id 25 must sort before 10 (numeric, not lexical): {desc:?}"
    );
}

/// No sort and no pagination params -> default ordering is `id ASC`
/// (`DEFAULT_SORT_COLUMN`) under the default page size of 10 (identical for a
/// Uuid PK). For an integer PK that is the first ten ids in numeric ascending
/// order, proving the default ordering is numeric (2 before 10), not lexical.
/// Mirrors `no_sort_param_uses_default_ordering` in the Uuid sort coverage test.
#[tokio::test]
async fn default_ordering_is_numeric_id_asc() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let (status, content_range, json) = list(&db, "").await;
    assert_eq!(status, StatusCode::OK, "default sort must not error");
    assert_eq!(
        typed(&json).len(),
        10,
        "default page size is 10, same as a Uuid PK"
    );
    assert_eq!(
        ids(&json),
        (1..=10).collect::<Vec<_>>(),
        "default ordering must be numeric id ASC (2 before 10)"
    );
    // Default page reports offset 0-9 of the full total 25.
    assert_eq!(content_range.as_deref(), Some("pps_things 0-9/25"));
}

// =============================================================================
// Pagination parity (mirrors pagination_content_range_http_coverage_test.rs).
// =============================================================================

/// range=[0,9] yields the first ten ids in numeric order with the offset header.
/// Mirrors `first_range_returns_first_ten_with_header`.
#[tokio::test]
async fn first_range_returns_first_ten_with_header() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let query = format!("range=%5B0%2C9%5D&{SORT_ID_ASC}");
    let (status, content_range, json) = list(&db, &query).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(typed(&json).len(), 10, "range=[0,9] should yield 10 rows");
    assert_eq!(
        content_range.as_deref(),
        Some("pps_things 0-9/25"),
        "Content-Range must report offset 0-9 of total 25"
    );
    assert_eq!(ids(&json), (1..=10).collect::<Vec<_>>());
}

/// range=[10,19] is the next ten ids; the header offset advances to 10.
/// Mirrors `second_range_offsets_header_and_slice`.
#[tokio::test]
async fn second_range_offsets_header_and_slice() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let query = format!("range=%5B10%2C19%5D&{SORT_ID_ASC}");
    let (status, content_range, json) = list(&db, &query).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(typed(&json).len(), 10, "range=[10,19] should yield 10 rows");
    assert_eq!(content_range.as_deref(), Some("pps_things 10-19/25"));
    assert_eq!(ids(&json), (11..=20).collect::<Vec<_>>());
}

/// `page=2&per_page=5` => offset 5, limit 5 (ids 6..=10).
/// Mirrors `page_and_per_page_paginate`.
#[tokio::test]
async fn page_and_per_page_paginate() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let query = format!("page=2&per_page=5&{SORT_ID_ASC}");
    let (status, content_range, json) = list(&db, &query).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        typed(&json).len(),
        5,
        "page 2 of size 5 should yield 5 rows"
    );
    assert_eq!(content_range.as_deref(), Some("pps_things 5-9/25"));
    assert_eq!(ids(&json), (6..=10).collect::<Vec<_>>());
}

/// A `per_page` above `max_page_size` (20) is capped at the resource limit even
/// though 25 rows exist. Mirrors `test_max_page_size_enforced_at_handler_level`
/// and the huge-range cap test, asserting the capped Content-Range as well.
#[tokio::test]
async fn per_page_above_max_is_capped() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let query = format!("page=1&per_page=1000&{SORT_ID_ASC}");
    let (status, content_range, json) = list(&db, &query).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        typed(&json).len(),
        20,
        "per_page=1000 must be capped at max_page_size (20), not 25"
    );
    assert_eq!(
        content_range.as_deref(),
        Some("pps_things 0-19/25"),
        "Content-Range end reflects the capped page; total still 25"
    );
    // Capped page is still numeric: ids 1..=20, not a lexical slice.
    assert_eq!(ids(&json), (1..=20).collect::<Vec<_>>());
}

/// A huge range end must not overflow/panic and caps to `max_page_size`.
/// Mirrors `huge_range_end_does_not_panic_and_caps_to_max_page_size`.
#[tokio::test]
async fn huge_range_end_caps_to_max_page_size() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let query = format!("range=%5B0%2C18446744073709551615%5D&{SORT_ID_ASC}");
    let (status, content_range, json) = list(&db, &query).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "huge range must return 200, not panic"
    );
    assert_eq!(
        typed(&json).len(),
        20,
        "limit capped at max_page_size (20) even though 25 rows exist"
    );
    assert_eq!(content_range.as_deref(), Some("pps_things 0-19/25"));
    assert_eq!(ids(&json), (1..=20).collect::<Vec<_>>());
}

/// An offset past the total returns 200 with an empty array and a sane header
/// whose start never exceeds end. Mirrors `out_of_range_offset_returns_empty`.
#[tokio::test]
async fn out_of_range_offset_returns_empty_with_sane_header() {
    let db = setup_test_db().await.unwrap();
    seed(&db).await;

    let query = format!("range=%5B100%2C109%5D&{SORT_ID_ASC}");
    let (status, content_range, json) = list(&db, &query).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(typed(&json).len(), 0, "offset beyond total yields no rows");

    let header = content_range.expect("Content-Range header must be present");
    assert_eq!(header, "pps_things 100-100/25");

    let range_part = header.split(' ').nth(1).unwrap();
    let nums: Vec<u64> = range_part
        .split('/')
        .next()
        .unwrap()
        .split('-')
        .map(|s| s.parse().unwrap())
        .collect();
    assert!(
        nums[0] <= nums[1],
        "range start must not exceed end: {header}"
    );
}
