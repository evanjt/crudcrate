//! Registration under a key that is unique only where a predicate holds.
//!
//! Scenario: an episode table is unique on its slot only while the episode is open
//! (`UNIQUE (slot) WHERE resolved_at IS NULL`). A resolved row sits outside that index, so the
//! slot is free again and the next breach is a new episode rather than a reopening of the old
//! one.
//!
//! Expected behaviour: the registration's find is narrowed by the same predicate the index
//! carries, so it never resolves a key to a row the index does not cover.

use chrono::{DateTime, Utc};
use crudcrate::{EntityToModels, UpsertStatus, upsert};
use sea_orm::entity::prelude::*;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, DbErr, EntityTrait, QueryOrder, Set,
};

pub mod episode {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "partial_key_episodes")]
    #[crudcrate(
        api_struct = "Episode",
        upsert_key(slot),
        upsert_where = Column::ResolvedAt.is_null()
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, exclude(create, update))]
        pub slot: String,
        pub severity: i32,
        #[crudcrate(filterable)]
        pub resolved_at: Option<DateTime<Utc>>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

use episode::{Episode, EpisodeCreate};

/// The table plus the partial unique index `create_table_from_entity` cannot express. MySQL has
/// no partial indexes, so there is nothing for the predicate to mirror there and the tests
/// return early.
async fn setup_test_db() -> Result<Option<DatabaseConnection>, DbErr> {
    let db = test_suite::reset_db!(episode::Entity).await?;
    if db.get_database_backend() == DbBackend::MySql {
        return Ok(None);
    }
    db.execute_unprepared(
        "CREATE UNIQUE INDEX IF NOT EXISTS partial_key_episodes_open_uniq
             ON partial_key_episodes (slot) WHERE resolved_at IS NULL",
    )
    .await?;
    Ok(Some(db))
}

fn breach(severity: i32) -> episode::ActiveModel {
    let mut active: episode::ActiveModel = EpisodeCreate {
        severity,
        resolved_at: None,
    }
    .into();
    active.slot = Set("martigny:temperature".to_string());
    active
}

async fn rows(db: &DatabaseConnection) -> Vec<episode::Model> {
    episode::Entity::find()
        .order_by_asc(episode::Column::Severity)
        .all(db)
        .await
        .expect("rows")
}

#[tokio::test]
async fn a_resolved_row_leaves_the_key_free_for_the_next_episode() {
    let Some(db) = setup_test_db().await.expect("db") else {
        return;
    };

    let (opened, status) = upsert::<Episode, _>(&db, breach(1)).await.expect("open");
    assert_eq!(status, UpsertStatus::Created);

    let (same, status) = upsert::<Episode, _>(&db, breach(2)).await.expect("worsen");
    assert_eq!(
        status,
        UpsertStatus::Updated,
        "the episode is still open, so the same row takes the new severity"
    );
    assert_eq!(same.id, opened.id);

    let resolved = episode::ActiveModel {
        id: Set(same.id),
        resolved_at: Set(Some(Utc::now())),
        ..Default::default()
    }
    .update(&db)
    .await
    .expect("resolve");

    let (reopened, status) = upsert::<Episode, _>(&db, breach(1))
        .await
        .expect("re-raise");
    assert_eq!(
        status,
        UpsertStatus::Created,
        "the resolved row is outside the index, so the slot registers a second episode"
    );
    assert_ne!(reopened.id, resolved.id);

    let stored = rows(&db).await;
    assert_eq!(stored.len(), 2, "both episodes are kept: {stored:?}");
    assert!(
        stored
            .iter()
            .any(|e| e.id == resolved.id && e.resolved_at.is_some() && e.severity == 2),
        "the resolved episode keeps the severity it closed at: {stored:?}"
    );
}

#[tokio::test]
async fn the_predicate_narrows_the_find_rather_than_the_write() {
    let Some(db) = setup_test_db().await.expect("db") else {
        return;
    };

    // A row the index does not cover, stored without going through a registration.
    let closed = episode::ActiveModel {
        id: Set(Uuid::new_v4()),
        slot: Set("martigny:temperature".to_string()),
        severity: Set(9),
        resolved_at: Set(Some(Utc::now())),
    }
    .insert(&db)
    .await
    .expect("a closed episode");

    let (opened, status) = upsert::<Episode, _>(&db, breach(1)).await.expect("open");
    assert_eq!(
        status,
        UpsertStatus::Created,
        "the find skips the closed row, and the partial index does not refuse the insert"
    );
    assert_ne!(opened.id, closed.id);
    assert_eq!(rows(&db).await.len(), 2);
}

/// The precondition the registration's savepoint path depends on: while a row is open, the index
/// is what refuses a second one, which is the refusal `upsert` re-reads against.
#[tokio::test]
async fn the_index_refuses_a_second_open_row_for_one_slot() {
    let Some(db) = setup_test_db().await.expect("db") else {
        return;
    };
    upsert::<Episode, _>(&db, breach(1)).await.expect("open");

    let second = episode::ActiveModel {
        id: Set(Uuid::new_v4()),
        slot: Set("martigny:temperature".to_string()),
        severity: Set(2),
        resolved_at: Set(None),
    }
    .insert(&db)
    .await;
    assert!(
        second.is_err(),
        "two open episodes for one slot must not be storable"
    );
}
