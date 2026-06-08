//! Coverage for the `CRUDResource` default trait methods in
//! `crudcrate/src/core/traits.rs` that the derive macro does NOT override for a
//! model with no join fields.
//!
//! For a plain entity (Uuid PK + scalar columns, no `join(...)` field) the
//! derive emits the trait impl without overriding the join/scope helpers, so
//! the following default bodies are exercised here directly:
//! - `get_one_scoped` (atomic `id = ? AND <scope>` lookup)
//! - `get_all_scoped` (delegates to `get_all`)
//! - `resolve_joined_filters` (returns the incoming condition unchanged)
//! - `joined_field_has_scope` (always `false`)
//! - `delete_many` (de-duplicated, existence-checked, batch-limited)
//! - `total_count` (filtered row count)
//!
//! Every method is called directly on the generated API struct rather than
//! through the HTTP router so the default trait body is what runs.

use crudcrate::{CRUDResource, EntityToModels, FilterOperator, JoinedFilter};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{Condition, Database, DatabaseConnection, DbErr, Order, Schema};
use uuid::Uuid;

pub mod doc {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "tdc_docs")]
    #[crudcrate(generate_router, api_struct = "TdcDoc")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[crudcrate(filterable)]
        pub is_private: bool,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

use doc::{Column, TdcDoc, TdcDocCreate};

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let db = Database::connect(&url).await?;
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    // Persistent backends (Postgres/MySQL) keep tables across tests within a binary;
    // drop first so every test starts from a clean schema. On sqlite::memory: each
    // connection is a fresh database, so the drop is a harmless no-op.
    db.execute(backend.build(&Table::drop().table(doc::Entity).if_exists().to_owned()))
        .await?;
    db.execute(backend.build(&schema.create_table_from_entity(doc::Entity)))
        .await?;
    Ok(db)
}

async fn create_doc(db: &DatabaseConnection, name: &str, is_private: bool) -> TdcDoc {
    TdcDoc::create(
        db,
        TdcDocCreate {
            name: name.to_string(),
            is_private,
        },
    )
    .await
    .expect("create should succeed")
}

fn is_not_found(err: &crudcrate::ApiError) -> bool {
    matches!(err, crudcrate::ApiError::NotFound { .. })
}

// --- get_one_scoped ---------------------------------------------------------

#[tokio::test]
async fn get_one_scoped_returns_row_when_scope_matches() {
    let db = setup_test_db().await.unwrap();
    let doc = create_doc(&db, "alpha", false).await;

    // Scope selects exactly the row's column value: id = ? AND is_private = false.
    let scope = Condition::all().add(Column::IsPrivate.eq(false));
    let fetched = TdcDoc::get_one_scoped(&db, doc.id, &scope)
        .await
        .expect("matching scope should return the row");

    assert_eq!(fetched.id, doc.id);
    assert_eq!(fetched.name, "alpha");
    assert!(!fetched.is_private);
}

#[tokio::test]
async fn get_one_scoped_empty_scope_matches_any_row() {
    let db = setup_test_db().await.unwrap();
    let doc = create_doc(&db, "beta", true).await;

    // Condition::all() with no clauses is a no-op AND; only the id filter narrows.
    let scope = Condition::all();
    let fetched = TdcDoc::get_one_scoped(&db, doc.id, &scope)
        .await
        .expect("empty scope should still find the row by id");

    assert_eq!(fetched.id, doc.id);
    assert!(fetched.is_private);
}

#[tokio::test]
async fn get_one_scoped_returns_not_found_when_scope_excludes_row() {
    let db = setup_test_db().await.unwrap();
    let doc = create_doc(&db, "gamma", true).await;

    // Row has is_private = true; scope demands is_private = false, so the
    // combined `id = ? AND is_private = false` query matches nothing.
    let scope = Condition::all().add(Column::IsPrivate.eq(false));
    let err = TdcDoc::get_one_scoped(&db, doc.id, &scope)
        .await
        .expect_err("excluding scope must yield an error");

    assert!(is_not_found(&err), "expected NotFound, got {err:?}");
}

#[tokio::test]
async fn get_one_scoped_returns_not_found_for_missing_id() {
    let db = setup_test_db().await.unwrap();
    let _ = create_doc(&db, "delta", false).await;

    let missing = Uuid::new_v4();
    let scope = Condition::all();
    let err = TdcDoc::get_one_scoped(&db, missing, &scope)
        .await
        .expect_err("unknown id must yield an error");

    assert!(is_not_found(&err), "expected NotFound, got {err:?}");
}

// --- get_all_scoped (delegates to get_all) ----------------------------------

#[tokio::test]
async fn get_all_scoped_returns_filtered_rows_like_get_all() {
    let db = setup_test_db().await.unwrap();
    create_doc(&db, "pub-1", false).await;
    create_doc(&db, "pub-2", false).await;
    create_doc(&db, "priv-1", true).await;

    let public_only = Condition::all().add(Column::IsPrivate.eq(false));

    let scoped = TdcDoc::get_all_scoped(&db, &public_only, Column::Name, Order::Asc, 0, 100)
        .await
        .expect("get_all_scoped should succeed");

    // Default impl delegates to get_all: same condition, same result set.
    let plain = TdcDoc::get_all(&db, &public_only, Column::Name, Order::Asc, 0, 100)
        .await
        .expect("get_all should succeed");

    assert_eq!(scoped.len(), 2, "only the two public rows match");
    assert_eq!(scoped.len(), plain.len());
    assert_eq!(scoped[0].name, "pub-1");
    assert_eq!(scoped[1].name, "pub-2");
}

#[tokio::test]
async fn get_all_scoped_respects_order_and_pagination() {
    let db = setup_test_db().await.unwrap();
    for n in ["c", "a", "b"] {
        create_doc(&db, n, false).await;
    }

    let all = Condition::all();
    let desc_first_page = TdcDoc::get_all_scoped(&db, &all, Column::Name, Order::Desc, 0, 2)
        .await
        .expect("get_all_scoped should succeed");

    assert_eq!(desc_first_page.len(), 2, "limit caps the page to 2 rows");
    assert_eq!(desc_first_page[0].name, "c");
    assert_eq!(desc_first_page[1].name, "b");

    let desc_second_page = TdcDoc::get_all_scoped(&db, &all, Column::Name, Order::Desc, 2, 2)
        .await
        .expect("get_all_scoped should succeed");
    assert_eq!(
        desc_second_page.len(),
        1,
        "offset=2 leaves a single remaining row"
    );
    assert_eq!(desc_second_page[0].name, "a");
}

// --- resolve_joined_filters (default: condition unchanged) ------------------

#[tokio::test]
async fn resolve_joined_filters_empty_returns_condition_unchanged() {
    let db = setup_test_db().await.unwrap();
    create_doc(&db, "one", false).await;
    create_doc(&db, "two", false).await;

    // A condition that selects exactly one row.
    let condition = Condition::all().add(Column::Name.eq("one"));

    let resolved = TdcDoc::resolve_joined_filters(&db, condition, &[])
        .await
        .expect("empty joined filters resolve cleanly");

    // The returned condition must still be the incoming one: it still selects
    // exactly the single matching row.
    assert_eq!(TdcDoc::total_count(&db, &resolved).await, 1);
}

#[tokio::test]
async fn resolve_joined_filters_nonempty_ignored_by_default() {
    let db = setup_test_db().await.unwrap();
    create_doc(&db, "keep", false).await;
    create_doc(&db, "other", false).await;

    // Incoming condition narrows to a single row.
    let condition = Condition::all().add(Column::Name.eq("keep"));

    // A non-empty joined filter referencing a field this no-join model does not
    // have. The default impl logs-and-ignores it, returning the condition as-is.
    let joined = vec![JoinedFilter {
        join_field: "phantom".to_string(),
        column: "whatever".to_string(),
        operator: FilterOperator::Eq,
        value: serde_json::json!("nope"),
    }];

    let resolved = TdcDoc::resolve_joined_filters(&db, condition, &joined)
        .await
        .expect("default resolve_joined_filters must not error on non-empty input");

    // Unchanged condition: still selects exactly the single "keep" row, proving
    // the phantom joined filter was NOT applied (which would have errored or
    // dropped the count to 0).
    assert_eq!(
        TdcDoc::total_count(&db, &resolved).await,
        1,
        "joined filter must be ignored, leaving the incoming condition intact"
    );
}

#[tokio::test]
async fn resolve_joined_filters_preserves_match_all_condition() {
    let db = setup_test_db().await.unwrap();
    create_doc(&db, "x", false).await;
    create_doc(&db, "y", true).await;

    let resolved = TdcDoc::resolve_joined_filters(&db, Condition::all(), &[])
        .await
        .expect("resolve should succeed");

    // Condition::all() unchanged still matches every row.
    assert_eq!(TdcDoc::total_count(&db, &resolved).await, 2);
}

// --- joined_field_has_scope (default false) ---------------------------------

#[tokio::test]
async fn joined_field_has_scope_defaults_to_false() {
    // No join fields exist; the default returns false for any name.
    assert!(!TdcDoc::joined_field_has_scope("anything"));
    assert!(!TdcDoc::joined_field_has_scope(""));
    assert!(!TdcDoc::joined_field_has_scope("name"));
    assert!(!TdcDoc::joined_field_has_scope("is_private"));
}

// --- delete_many (dedup, existence check, batch limit) ----------------------

#[tokio::test]
async fn delete_many_dedups_and_filters_to_existing_rows() {
    let db = setup_test_db().await.unwrap();
    let a = create_doc(&db, "a", false).await;
    let missing = Uuid::new_v4();

    // Input [a, a, b] where a exists (twice) and b does not exist.
    let deleted = TdcDoc::delete_many(&db, vec![a.id, a.id, missing])
        .await
        .expect("delete_many should succeed");

    assert_eq!(
        deleted.len(),
        1,
        "duplicate a collapses, missing b excluded"
    );
    assert_eq!(deleted[0], a.id);

    // The row really is gone.
    assert_eq!(TdcDoc::total_count(&db, &Condition::all()).await, 0);
}

#[tokio::test]
async fn delete_many_empty_input_returns_empty() {
    let db = setup_test_db().await.unwrap();
    create_doc(&db, "kept", false).await;

    let deleted = TdcDoc::delete_many(&db, vec![])
        .await
        .expect("empty delete_many is a no-op Ok");
    assert!(deleted.is_empty());

    // Nothing was deleted.
    assert_eq!(TdcDoc::total_count(&db, &Condition::all()).await, 1);
}

#[tokio::test]
async fn delete_many_all_missing_returns_empty() {
    let db = setup_test_db().await.unwrap();
    create_doc(&db, "survivor", false).await;

    let deleted = TdcDoc::delete_many(&db, vec![Uuid::new_v4(), Uuid::new_v4()])
        .await
        .expect("delete_many over non-existent ids is Ok(empty)");
    assert!(
        deleted.is_empty(),
        "no ids existed, so none reported deleted"
    );

    assert_eq!(TdcDoc::total_count(&db, &Condition::all()).await, 1);
}

#[tokio::test]
async fn delete_many_over_batch_limit_errors() {
    let db = setup_test_db().await.unwrap();

    // Default batch_limit() is 100; 101 ids exceeds it.
    let limit = TdcDoc::batch_limit();
    assert_eq!(limit, 100, "this model uses the default batch limit");

    let ids: Vec<Uuid> = (0..=limit).map(|_| Uuid::new_v4()).collect();
    assert_eq!(ids.len(), limit + 1);

    let err = TdcDoc::delete_many(&db, ids)
        .await
        .expect_err("over-limit delete_many must error before touching the db");

    let msg = format!("{err:?}");
    assert!(
        msg.contains("Batch delete limited to 100 items"),
        "expected batch-limit error, got: {msg}"
    );
}

#[tokio::test]
async fn delete_many_at_batch_limit_is_allowed() {
    let db = setup_test_db().await.unwrap();
    let limit = TdcDoc::batch_limit();

    // Exactly batch_limit ids (none exist) is within bounds: Ok(empty), no error.
    let ids: Vec<Uuid> = (0..limit).map(|_| Uuid::new_v4()).collect();
    let deleted = TdcDoc::delete_many(&db, ids)
        .await
        .expect("delete_many at exactly the batch limit is allowed");
    assert!(deleted.is_empty());
}

// --- total_count ------------------------------------------------------------

#[tokio::test]
async fn total_count_counts_all_rows() {
    let db = setup_test_db().await.unwrap();
    assert_eq!(
        TdcDoc::total_count(&db, &Condition::all()).await,
        0,
        "empty table counts zero"
    );

    create_doc(&db, "1", false).await;
    create_doc(&db, "2", true).await;
    create_doc(&db, "3", false).await;

    assert_eq!(TdcDoc::total_count(&db, &Condition::all()).await, 3);
}

#[tokio::test]
async fn total_count_respects_condition() {
    let db = setup_test_db().await.unwrap();
    create_doc(&db, "pub-a", false).await;
    create_doc(&db, "pub-b", false).await;
    create_doc(&db, "priv-a", true).await;

    let private_only = Condition::all().add(Column::IsPrivate.eq(true));
    assert_eq!(TdcDoc::total_count(&db, &private_only).await, 1);

    let public_only = Condition::all().add(Column::IsPrivate.eq(false));
    assert_eq!(TdcDoc::total_count(&db, &public_only).await, 2);

    let by_name = Condition::all().add(Column::Name.eq("priv-a"));
    assert_eq!(TdcDoc::total_count(&db, &by_name).await, 1);

    let no_match = Condition::all().add(Column::Name.eq("nonexistent"));
    assert_eq!(TdcDoc::total_count(&db, &no_match).await, 0);
}
