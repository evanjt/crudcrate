//! The single-row lifecycle runs inside one transaction the orchestrator opens.
//!
//! Scenario: an `after_create` hook fails after the insert has already run.
//!
//! Expected behaviour: the row the insert wrote is not there afterwards. The hook and the write it
//! guards succeed or fail together, and `after_begin` is where a consumer issues the session
//! statements that write needs (`SET LOCAL`), because it runs on the same transaction.

use crudcrate::{ApiError, CRUDOperations, EntityToModels};
use serial_test::serial;
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr};
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid::Uuid;

/// Counted across the process, so every test here is `#[serial]`: two running at once would each
/// see the other's lifecycles.
static AFTER_BEGIN: AtomicUsize = AtomicUsize::new(0);

pub mod audit_note {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "audit_notes")]
    #[crudcrate(api_struct = "AuditNote")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        pub subject: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod ledger_entry {
    use super::*;
    use super::audit_note::{AuditNote, AuditNoteCreate};

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "ledger_entries")]
    #[crudcrate(api_struct = "LedgerEntry", operations = LedgerOps)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}

    pub struct LedgerOps;

    #[allow(clippy::unused_async_trait_impl)]
    impl CRUDOperations for LedgerOps {
        type Resource = LedgerEntry;

        async fn after_begin<C: sea_orm::ConnectionTrait>(&self, db: &C) -> Result<(), ApiError> {
            // A real consumer issues `SET LOCAL` here. Any statement proves the hook is handed a
            // connection it can write on.
            db.execute_unprepared("SELECT 1").await?;
            AFTER_BEGIN.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn after_update<C: sea_orm::ConnectionTrait>(
            &self,
            _db: &C,
            entity: &mut LedgerEntry,
        ) -> Result<(), ApiError> {
            if entity.name == "doomed" {
                return Err(ApiError::bad_request("the after hook refused it"));
            }
            Ok(())
        }

        async fn after_create<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(
            &self,
            db: &C,
            entity: &mut LedgerEntry,
        ) -> Result<(), ApiError> {
            // A hook writing a second resource on the connection it is handed.
            <AuditNote as crudcrate::CRUDResource>::create(
                db,
                AuditNoteCreate {
                    subject: entity.name.clone(),
                },
            )
            .await?;
            if entity.name == "doomed" {
                return Err(ApiError::bad_request("the after hook refused it"));
            }
            Ok(())
        }
    }
}

use ledger_entry::{LedgerEntryCreate, LedgerEntryUpdate, LedgerOps};

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(ledger_entry::Entity, audit_note::Entity).await
}

fn create(name: &str) -> LedgerEntryCreate {
    LedgerEntryCreate {
        name: name.to_string(),
    }
}

fn rename(name: &str) -> LedgerEntryUpdate {
    LedgerEntryUpdate {
        name: Some(Some(name.to_string())),
    }
}

#[tokio::test]
#[serial]
async fn a_failing_after_hook_takes_the_insert_with_it() {
    AFTER_BEGIN.store(0, Ordering::SeqCst);
    let db = setup_test_db().await.unwrap();

    LedgerOps
        .create(&db, create("kept"))
        .await
        .expect("a create whose hooks pass commits");

    let err = LedgerOps
        .create(&db, create("doomed"))
        .await
        .expect_err("the after hook refuses it");
    assert!(format!("{err:?}").contains("the after hook refused it"));

    let names: Vec<String> = ledger_entry::Entity::find()
        .all(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.name)
        .collect();
    assert_eq!(
        names,
        vec!["kept".to_string()],
        "the refused row's insert was rolled back with the hook that refused it"
    );
    assert_eq!(
        AFTER_BEGIN.load(Ordering::SeqCst),
        2,
        "after_begin runs once per lifecycle, on the transaction the write happens in"
    );

    let subjects: Vec<String> = audit_note::Entity::find()
        .all(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.subject)
        .collect();
    assert_eq!(
        subjects,
        vec!["kept".to_string()],
        "the hook's own write lands with the row it followed, and goes back with the refused one"
    );
}

/// Scenario: a five-row batch create whose third row the `after_create` hook refuses.
///
/// Expected behaviour: no row from the batch is there afterwards. A batch is one write or none,
/// so a caller reading an error never has to ask which half of it landed.
#[tokio::test]
#[serial]
async fn a_batch_that_fails_partway_leaves_none_of_its_rows() {
    AFTER_BEGIN.store(0, Ordering::SeqCst);
    let db = setup_test_db().await.unwrap();

    let err = LedgerOps
        .create_many(
            &db,
            vec![
                create("first"),
                create("second"),
                create("doomed"),
                create("fourth"),
                create("fifth"),
            ],
        )
        .await
        .expect_err("the after hook refuses the third row");
    assert!(format!("{err:?}").contains("the after hook refused it"));

    assert_eq!(
        ledger_entry::Entity::find().count(&db).await.unwrap(),
        0,
        "the two rows written before the refusal go back with it"
    );
}

/// The same rule for an update: the rows edited before the failure are not left edited.
#[tokio::test]
#[serial]
async fn a_batch_update_that_fails_partway_leaves_no_row_edited() {
    AFTER_BEGIN.store(0, Ordering::SeqCst);
    let db = setup_test_db().await.unwrap();

    let kept = LedgerOps.create(&db, create("kept")).await.unwrap();
    let doomed = LedgerOps.create(&db, create("other")).await.unwrap();

    let err = LedgerOps
        .update_many(
            &db,
            vec![
                (kept.id, rename("renamed")),
                (doomed.id, rename("doomed")),
            ],
        )
        .await
        .expect_err("the after hook refuses the second row");
    assert!(format!("{err:?}").contains("the after hook refused it"));

    let names: Vec<String> = ledger_entry::Entity::find()
        .all(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.name)
        .collect();
    assert!(
        !names.contains(&"renamed".to_string()),
        "the row edited before the refusal is not left edited: {names:?}"
    );
}
