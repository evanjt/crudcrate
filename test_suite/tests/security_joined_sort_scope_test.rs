//! The joined-sort path must carry the same scope discipline as the joined-filter
//! path. Under an active scope with the secure profile, ordering parent rows by a
//! column on an unscoped child is an ordering oracle, so it is rejected, exactly as
//! the equivalent joined filter already is. The guard only fires under scope, so an
//! unscoped request (and a joined sort on a scoped child) still succeeds.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use common::{setup_scoped_app, setup_test_app, setup_test_db};

fn encode(value: &str) -> String {
    percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC).to_string()
}

async fn get_status(app: &axum::Router, uri: &str) -> StatusCode {
    app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

/// VehiclePart carries no scope_condition, so ordering scoped vehicles by a parts
/// column would leak child existence through the row order. Rejected with 400.
#[tokio::test]
async fn scoped_joined_sort_on_unscoped_child_is_rejected() {
    let db = setup_test_db().await.expect("setup");
    let app = setup_scoped_app(&db);

    let sort = encode(r#"["parts.name","ASC"]"#);
    let status = get_status(&app, &format!("/vehicles?sort={sort}")).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "joined sort on an unscoped child must be rejected under strict scope"
    );
}

/// Vehicle carries a scope_condition, so a joined sort on it stays allowed.
#[tokio::test]
async fn scoped_joined_sort_on_scoped_child_is_allowed() {
    let db = setup_test_db().await.expect("setup");
    let app = setup_scoped_app(&db);

    let sort = encode(r#"["vehicles.year","DESC"]"#);
    let status = get_status(&app, &format!("/customers?sort={sort}")).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "joined sort on a scoped child must still succeed"
    );
}

/// The guard only fires when a scope is active, so an unscoped request is unaffected.
#[tokio::test]
async fn unscoped_joined_sort_on_unscoped_child_is_allowed() {
    let db = setup_test_db().await.expect("setup");
    let app = setup_test_app(&db);

    let sort = encode(r#"["parts.name","ASC"]"#);
    let status = get_status(&app, &format!("/vehicles?sort={sort}")).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "without an active scope the joined sort must succeed"
    );
}

/// A scope-excluded column declared joined-sortable must not be orderable through a
/// join when scoped: the parser drops it to a default column sort. Vehicle is a
/// scoped child (so the entity-level guard allows it), but its `is_private` column is
/// `exclude(scoped)`, so ordering on it would still leak a hidden column.
#[test]
fn scope_excluded_joined_sort_column_is_dropped() {
    use common::customer::Customer;
    use crudcrate::traits::CRUDResource;
    use crudcrate::{SortConfig, parse_sorting_with_joins};

    let params = crudcrate::models::FilterOptions {
        sort: Some(r#"["vehicles.is_private","ASC"]"#.to_string()),
        ..Default::default()
    };
    let sortable = Customer::sortable_columns();
    let default_col = Customer::default_index_column();

    let unscoped = parse_sorting_with_joins::<Customer, _>(&params, &sortable, default_col, &[]);
    assert!(
        matches!(unscoped, SortConfig::Joined { .. }),
        "unscoped: the joined sort on is_private is honoured"
    );

    let scoped =
        parse_sorting_with_joins::<Customer, _>(&params, &sortable, default_col, &["is_private"]);
    assert!(
        matches!(scoped, SortConfig::Column { .. }),
        "scoped: a scope-excluded joined column must fall back to a column sort"
    );
}
