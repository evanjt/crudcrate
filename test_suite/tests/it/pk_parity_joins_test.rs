//! Relationship-loading PARITY between i32 primary keys and Uuid primary keys.
//!
//! Integer/non-UUID PK support is generic across the trait, the `crud_handlers!`
//! macro `Path<PK>`, `CRUDOperations`, and the join-loading codegen. The
//! batch-loading path in `crudcrate-derive/src/codegen/joins/loading.rs` keys
//! related rows by the parent PK in a `HashMap<PK, ...>` and extracts the child
//! FK to group them. This file proves that path works EXACTLY the same when the
//! PK and FK are `i32` instead of `Uuid`.
//!
//! Two isolated subtrees (every PK and FK is `i32`):
//!
//!   `has_many` depth chain (mirrors `join_get_all_depth_coverage_test.rs`):
//!     Author (i32 PK)
//!       -> Book    (i32 PK, `author_id` i32 FK)   `has_many`, join(one, all, depth = 2)
//!            -> Chapter (i32 PK, `book_id` i32 FK) `has_many`, join(one, all, depth = 1)
//!
//!   Option<T> `belongs_to` (mirrors `option_belongs_to_join_all_test.rs`):
//!     Membership (i32 PK, `reader_id` Option<i32> FK)
//!       reader: Option<Reader>  `belongs_to`, join(one, all, depth = 1)
//!     Reader (i32 PK) does NOT join back, so there is no recursion cycle.
//!
//! Every assertion here is the i32 analogue of one in those UUID files; the only
//! difference is the PK/FK type and that the `Path<id>` parameter is an integer.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use serde_json::Value;
use tower::ServiceExt;

pub mod author {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "ppj_authors")]
    #[crudcrate(generate_router, api_struct = "PpjAuthor", derive_partial_eq)]
    pub struct Model {
        // Auto-increment i32 PK: the DB assigns the id, no on_create.
        #[sea_orm(primary_key)]
        #[crudcrate(primary_key, exclude(create, update))]
        pub id: i32,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        // has_many at depth = 2 so the batch loader recurses Book::get_one and
        // pulls grandchild chapters too. FK on the child (`author_id`) is i32.
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, exclude(create, update), join(one, all, depth = 2))]
        pub books: Vec<super::book::PpjBook>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::book::Entity")]
        Books,
    }

    impl Related<super::book::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Books.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod book {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "ppj_books")]
    #[crudcrate(generate_router, api_struct = "PpjBook", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key)]
        #[crudcrate(primary_key, exclude(create, update))]
        pub id: i32,

        // i32 FK back to the parent author.
        #[crudcrate(filterable)]
        pub author_id: i32,

        #[crudcrate(filterable, sortable)]
        pub title: String,

        // has_many grandchildren (relative to Author). Their FK (`book_id`) is i32.
        // No back-reference join to Author here, so the depth-2 chain terminates.
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, exclude(create, update), join(one, all, depth = 1))]
        pub chapters: Vec<super::chapter::PpjChapter>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::author::Entity",
            from = "Column::AuthorId",
            to = "super::author::Column::Id"
        )]
        Author,
        #[sea_orm(has_many = "super::chapter::Entity")]
        Chapters,
    }

    impl Related<super::author::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Author.def()
        }
    }

    impl Related<super::chapter::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Chapters.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod chapter {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "ppj_chapters")]
    #[crudcrate(generate_router, api_struct = "PpjChapter", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key)]
        #[crudcrate(primary_key, exclude(create, update))]
        pub id: i32,

        #[crudcrate(filterable)]
        pub book_id: i32,

        #[crudcrate(filterable, sortable)]
        pub title: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::book::Entity",
            from = "Column::BookId",
            to = "super::book::Column::Id"
        )]
        Book,
    }

    impl Related<super::book::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Book.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

// --- Option<T> belongs_to subtree (isolated, no recursion cycle) ---

pub mod reader {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "ppj_readers")]
    #[crudcrate(generate_router, api_struct = "PpjReader", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key)]
        #[crudcrate(primary_key, exclude(create, update))]
        pub id: i32,

        #[crudcrate(filterable, sortable)]
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::membership::Entity")]
        Memberships,
    }

    impl Related<super::membership::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Memberships.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod membership {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "ppj_memberships")]
    #[crudcrate(generate_router, api_struct = "PpjMembership", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key)]
        #[crudcrate(primary_key, exclude(create, update))]
        pub id: i32,

        // Nullable i32 belongs_to FK: a membership may have no reader.
        #[crudcrate(filterable)]
        pub reader_id: Option<i32>,

        #[crudcrate(filterable, sortable)]
        pub tier: String,

        // belongs_to parent: FK (`reader_id`) is on THIS row, resolved via
        // find_related. Reader does not join back, so no cycle.
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, exclude(create, update), join(one, all, depth = 1))]
        pub reader: Option<super::reader::PpjReader>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::reader::Entity",
            from = "Column::ReaderId",
            to = "super::reader::Column::Id"
        )]
        Reader,
    }

    impl Related<super::reader::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Reader.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let db = Database::connect(&url).await?;
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    // Persistent backends (Postgres/MySQL) keep tables across tests within a binary;
    // drop first so every test starts from a clean schema. On sqlite::memory: each
    // connection is a fresh database, so the drops are harmless no-ops.
    // create_table_from_entity emits FK constraints from belongs_to relations, so
    // drop children before parents (reverse of the create order below).
    db.execute(
        &Table::drop()
            .table(membership::Entity)
            .if_exists()
            .to_owned(),
    )
    .await?;
    db.execute(&Table::drop().table(reader::Entity).if_exists().to_owned())
        .await?;
    db.execute(&Table::drop().table(chapter::Entity).if_exists().to_owned())
        .await?;
    db.execute(&Table::drop().table(book::Entity).if_exists().to_owned())
        .await?;
    db.execute(&Table::drop().table(author::Entity).if_exists().to_owned())
        .await?;

    db.execute(&schema.create_table_from_entity(author::Entity))
        .await?;
    db.execute(&schema.create_table_from_entity(book::Entity))
        .await?;
    db.execute(&schema.create_table_from_entity(chapter::Entity))
        .await?;
    db.execute(&schema.create_table_from_entity(reader::Entity))
        .await?;
    db.execute(&schema.create_table_from_entity(membership::Entity))
        .await?;

    Ok(db)
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new()
        .nest("/authors", author::PpjAuthor::router(db).into())
        .nest("/books", book::PpjBook::router(db).into())
        .nest("/chapters", chapter::PpjChapter::router(db).into())
        .nest("/readers", reader::PpjReader::router(db).into())
        .nest("/memberships", membership::PpjMembership::router(db).into())
}

async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
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
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

/// Ids assigned by the DB for the `has_many` subtree.
struct SeededTree {
    author_with_books: i32,
    childless_author: i32,
}

/// One author with two books (2 and 3 chapters), plus a childless author. All
/// ids assigned by the DB as i32.
async fn seed_tree(db: &DatabaseConnection) -> SeededTree {
    let author = author::PpjAuthor::create(
        db,
        author::PpjAuthorCreate {
            name: "Steve Klabnik".to_string(),
        },
    )
    .await
    .expect("create author");

    let childless = author::PpjAuthor::create(
        db,
        author::PpjAuthorCreate {
            name: "Nobody".to_string(),
        },
    )
    .await
    .expect("create childless author");

    for (title, n_chapters) in [("The Rust Programming Language", 2), ("Rust by Example", 3)] {
        let book = book::PpjBook::create(
            db,
            book::PpjBookCreate {
                author_id: author.id,
                title: title.to_string(),
            },
        )
        .await
        .expect("create book");

        for c in 0..n_chapters {
            chapter::PpjChapter::create(
                db,
                chapter::PpjChapterCreate {
                    book_id: book.id,
                    title: format!("{title} - chapter {c}"),
                },
            )
            .await
            .expect("create chapter");
        }
    }

    SeededTree {
        author_with_books: author.id,
        childless_author: childless.id,
    }
}

/// Ids for the Option `belongs_to` subtree.
struct SeededMemberships {
    reader_id: i32,
    member_ids: Vec<i32>,
    orphan_membership: i32,
}

/// One reader, two memberships pointing at it, one orphan membership (no reader).
async fn seed_memberships(db: &DatabaseConnection) -> SeededMemberships {
    let reader = reader::PpjReader::create(
        db,
        reader::PpjReaderCreate {
            name: "Ada".to_string(),
        },
    )
    .await
    .expect("create reader");

    let mut member_ids = Vec::new();
    for tier in ["gold", "silver"] {
        let m = membership::PpjMembership::create(
            db,
            membership::PpjMembershipCreate {
                reader_id: Some(reader.id),
                tier: tier.to_string(),
            },
        )
        .await
        .expect("create owned membership");
        member_ids.push(m.id);
    }

    let orphan = membership::PpjMembership::create(
        db,
        membership::PpjMembershipCreate {
            reader_id: None,
            tier: "trial".to_string(),
        },
    )
    .await
    .expect("create orphan membership");

    SeededMemberships {
        reader_id: reader.id,
        member_ids,
        orphan_membership: orphan.id,
    }
}

/// The DB-assigned ids must be integers, not UUID strings: confirms the PK
/// round-trips as JSON numbers everywhere downstream.
#[tokio::test]
async fn seeded_ids_are_integers() {
    let db = setup_test_db().await.expect("db setup");
    let s = seed_tree(&db).await;

    let app = app(&db);
    let (status, body) = get_json(&app, &format!("/authors/{}", s.author_with_books)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["id"].is_i64() || body["id"].is_u64(),
        "author id must be an integer in JSON, got {:?}",
        body["id"]
    );
    assert_eq!(body["id"].as_i64(), Some(i64::from(s.author_with_books)));
}

/// LIST exercises the batch `get_all` loader. Each author's `books` must be
/// populated keyed by the integer `author_id` FK, the `HashMap<i32, _>` group
/// step in the batch loader. Mirrors
/// `join_get_all_depth_coverage_test::list_orgs_populates_teams_and_grandchild_members`.
#[tokio::test]
async fn list_authors_batch_loads_books_keyed_by_integer_fk() {
    let db = setup_test_db().await.expect("db setup");
    let s = seed_tree(&db).await;

    let app = app(&db);
    let (status, body) = get_json(&app, "/authors").await;
    assert_eq!(status, StatusCode::OK, "authors list: {body:?}");

    let authors = body.as_array().expect("list response is an array");
    assert_eq!(authors.len(), 2, "both authors returned");

    let with_books = authors
        .iter()
        .find(|a| a["id"].as_i64() == Some(i64::from(s.author_with_books)))
        .expect("author with books present");
    let books = with_books["books"]
        .as_array()
        .expect("books populated on list row");
    assert_eq!(
        books.len(),
        2,
        "author should batch-load 2 books via i32 FK"
    );

    // Every loaded book's i32 author_id FK must point back at this author.
    for book in books {
        assert_eq!(
            book["author_id"].as_i64(),
            Some(i64::from(s.author_with_books)),
            "child FK back-reference must be the integer parent id"
        );
    }
}

/// The childless author must come back with an EMPTY books array (not
/// null/missing): the batch loader still produces an entry for a parent id
/// that has no children. Mirrors
/// `list_org_with_no_teams_yields_empty_teams_array`.
#[tokio::test]
async fn childless_author_gets_empty_books_array() {
    let db = setup_test_db().await.expect("db setup");
    let s = seed_tree(&db).await;

    let app = app(&db);
    let (status, body) = get_json(&app, "/authors").await;
    assert_eq!(status, StatusCode::OK);

    let childless = body
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"].as_i64() == Some(i64::from(s.childless_author)))
        .expect("childless author present");
    let books = childless["books"]
        .as_array()
        .expect("books is an array even when empty");
    assert!(books.is_empty(), "childless author has empty books array");
}

/// depth > 1: the author's `books` is `join(one, all, depth = 2)`, so the batch
/// loader recurses into each book via `PpjBook::get_one` and populates the
/// grandchild `chapters`. Grandchildren are grouped by the integer `book_id`
/// FK. Mirrors the depth>1 grandchild-member assertions in the UUID file.
#[tokio::test]
async fn list_authors_loads_grandchild_chapters_at_depth_two() {
    let db = setup_test_db().await.expect("db setup");
    let s = seed_tree(&db).await;

    let app = app(&db);
    let (status, body) = get_json(&app, "/authors").await;
    assert_eq!(status, StatusCode::OK);

    let with_books = body
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"].as_i64() == Some(i64::from(s.author_with_books)))
        .expect("author with books present");

    let books = with_books["books"].as_array().unwrap();
    assert_eq!(books.len(), 2);

    let mut chapter_counts: Vec<usize> = books
        .iter()
        .map(|b| {
            let chapters = b["chapters"]
                .as_array()
                .expect("grandchild chapters populated at depth 2");
            // Each chapter's i32 book_id must point back at its parent book.
            let book_id = b["id"].as_i64().unwrap();
            for ch in chapters {
                assert_eq!(
                    ch["book_id"].as_i64(),
                    Some(book_id),
                    "grandchild FK must be the integer parent book id"
                );
            }
            chapters.len()
        })
        .collect();
    chapter_counts.sort_unstable();
    assert_eq!(
        chapter_counts,
        vec![2, 3],
        "grandchild chapters grouped correctly by integer book_id"
    );
}

/// `get_one` on the parent (path param is the INTEGER author id) must agree
/// with the LIST shape: books and grandchild chapters identically populated.
/// Mirrors `get_one_org_populates_teams_and_grandchild_members`.
#[tokio::test]
async fn get_one_author_agrees_with_list_at_depth_two() {
    let db = setup_test_db().await.expect("db setup");
    let s = seed_tree(&db).await;

    let app = app(&db);

    // Integer path param, not a UUID.
    let (status, one) = get_json(&app, &format!("/authors/{}", s.author_with_books)).await;
    assert_eq!(status, StatusCode::OK);
    let one_books = one["books"].as_array().expect("books on get_one");
    assert_eq!(one_books.len(), 2);

    let (_, list) = get_json(&app, "/authors").await;
    let from_list = list
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"].as_i64() == Some(i64::from(s.author_with_books)))
        .unwrap()
        .clone();

    let chapter_total = |author: &Value| -> usize {
        author["books"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["chapters"].as_array().unwrap().len())
            .sum()
    };
    assert_eq!(
        chapter_total(&one),
        chapter_total(&from_list),
        "get_one and get_all must load the same grandchild chapter count"
    );
    assert_eq!(chapter_total(&one), 5, "2 + 3 chapters across two books");
}

/// LIST of memberships exercises the `Option<Reader>` `belongs_to` batch path:
/// the FK (`reader_id`) is on the membership row, so each owned membership
/// carries its reader and the orphan (`reader_id` = None) stays null. Mirrors
/// `option_belongs_to_join_all_test::list_widgets_populates_belongs_to_owner_and_leaves_orphan_null`.
#[tokio::test]
async fn list_memberships_populates_belongs_to_reader_and_leaves_orphan_null() {
    let db = setup_test_db().await.expect("db setup");
    let s = seed_memberships(&db).await;

    let app = app(&db);
    let (status, body) = get_json(&app, "/memberships").await;
    assert_eq!(status, StatusCode::OK, "memberships list: {body:?}");

    let memberships = body.as_array().expect("list response is an array");
    assert_eq!(memberships.len(), 3, "all three memberships returned");

    for id in &s.member_ids {
        let row = memberships
            .iter()
            .find(|m| m["id"].as_i64() == Some(i64::from(*id)))
            .expect("owned membership present in list");
        let reader = &row["reader"];
        assert!(
            reader.is_object(),
            "owned membership's belongs_to reader must be populated in get_all, got {reader}"
        );
        assert_eq!(
            reader["id"].as_i64(),
            Some(i64::from(s.reader_id)),
            "belongs_to reader resolved via the integer FK on the membership row"
        );
        assert_eq!(reader["name"].as_str(), Some("Ada"));
    }

    let orphan_row = memberships
        .iter()
        .find(|m| m["id"].as_i64() == Some(i64::from(s.orphan_membership)))
        .expect("orphan membership present in list");
    assert!(
        orphan_row["reader"].is_null(),
        "orphan membership (reader_id = None) must have reader = null, got {}",
        orphan_row["reader"]
    );
}

/// `get_one` on a membership (integer path param) must agree with the LIST
/// shape for the `belongs_to` reader. Mirrors
/// `option_belongs_to_join_all_test::get_one_widget_agrees_with_list`.
#[tokio::test]
async fn get_one_membership_agrees_on_belongs_to_reader() {
    let db = setup_test_db().await.expect("db setup");
    let s = seed_memberships(&db).await;

    let app = app(&db);

    let (status, owned) = get_json(&app, &format!("/memberships/{}", s.member_ids[0])).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        owned["reader"]["id"].as_i64(),
        Some(i64::from(s.reader_id)),
        "get_one must populate the belongs_to reader via integer FK"
    );
    assert_eq!(owned["reader"]["name"].as_str(), Some("Ada"));

    let (status, orphan) = get_json(&app, &format!("/memberships/{}", s.orphan_membership)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        orphan["reader"].is_null(),
        "get_one orphan membership must have reader = null, got {}",
        orphan["reader"]
    );
}

/// A non-existent INTEGER id returns 404 (not a UUID parse error / 400). This
/// is the parity assertion against `integer_pk_test::test_integer_pk_get_one`.
#[tokio::test]
async fn missing_integer_id_returns_404_not_parse_error() {
    let db = setup_test_db().await.expect("db setup");
    let _ = seed_tree(&db).await;

    let app = app(&db);
    let (status, _) = get_json(&app, "/authors/99999").await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "missing integer id must be 404, never a UUID parse error"
    );
}

/// Direct trait call rather than HTTP: `PpjAuthor::get_all` returns api structs
/// whose typed `books` and nested `chapters` fields are populated with i32 ids
/// and FKs. Verifies the generated struct fields, not just JSON.
#[tokio::test]
async fn get_all_trait_call_populates_typed_nested_fields_with_i32() {
    let db = setup_test_db().await.expect("db setup");
    let s = seed_tree(&db).await;

    let authors = author::PpjAuthor::get_all(
        &db,
        &sea_orm::Condition::all(),
        author::Column::Name,
        sea_orm::Order::Asc,
        0,
        100,
    )
    .await
    .expect("get_all");

    let author = authors
        .iter()
        .find(|a| a.id == s.author_with_books)
        .expect("seeded author present");
    assert_eq!(author.books.len(), 2, "typed books field populated");

    let mut chapter_counts: Vec<usize> = author
        .books
        .iter()
        .map(|b| {
            assert_eq!(
                b.author_id, s.author_with_books,
                "typed child FK is the integer parent id"
            );
            b.chapters.len()
        })
        .collect();
    chapter_counts.sort_unstable();
    assert_eq!(
        chapter_counts,
        vec![2, 3],
        "typed grandchild chapters populated"
    );

    let childless = authors
        .iter()
        .find(|a| a.id == s.childless_author)
        .expect("childless author present");
    assert!(
        childless.books.is_empty(),
        "typed empty books for childless author"
    );
}

/// Direct trait call for the Option `belongs_to` side: `PpjMembership::get_all`
/// returns api structs whose typed `reader: Option<PpjReader>` is `Some` with
/// the right i32 id for owned rows and `None` for the orphan.
#[tokio::test]
async fn get_all_trait_call_populates_typed_option_reader_with_i32() {
    let db = setup_test_db().await.expect("db setup");
    let s = seed_memberships(&db).await;

    let memberships = membership::PpjMembership::get_all(
        &db,
        &sea_orm::Condition::all(),
        membership::Column::Tier,
        sea_orm::Order::Asc,
        0,
        100,
    )
    .await
    .expect("get_all");

    for id in &s.member_ids {
        let m = memberships
            .iter()
            .find(|m| m.id == *id)
            .expect("owned membership present");
        let reader = m.reader.as_ref().expect("typed Option reader is Some");
        assert_eq!(reader.id, s.reader_id);
        assert_eq!(reader.name, "Ada");
    }

    let orphan = memberships
        .iter()
        .find(|m| m.id == s.orphan_membership)
        .expect("orphan membership present");
    assert!(
        orphan.reader.is_none(),
        "orphan membership's typed reader is None"
    );
}
