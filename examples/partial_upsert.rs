//! # Registration under a partial unique index
//!
//! An alarm episode is unique on its slot only while it is open:
//!
//! ```sql
//! CREATE UNIQUE INDEX episodes_open_uniq
//!     ON episodes (site, parameter) WHERE resolved_at IS NULL;
//! ```
//!
//! `upsert_where` tells the registration the same thing, so its find is narrowed to the rows
//! that index covers. An open episode registers against itself; once it is resolved it leaves
//! the index and the next breach of the slot opens a second episode, with the first kept whole.
//!
//! Run with: `cargo run --example partial_upsert`

use chrono::{DateTime, Utc};
use crudcrate::{EntityToModels, upsert};
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbErr, EntityTrait,
    QueryOrder, Set, Statement, entity::prelude::*,
};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "episodes")]
#[crudcrate(
    api_struct = "Episode",
    upsert_key(site, parameter),
    upsert_where = Column::ResolvedAt.is_null()
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, exclude(create, update))]
    pub site: String,
    #[crudcrate(filterable, exclude(create, update))]
    pub parameter: String,

    pub severity: i32,
    #[crudcrate(filterable)]
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

/// One breach of the Martigny temperature slot, as a source system would send it.
fn breach(severity: i32) -> ActiveModel {
    let mut active: ActiveModel = EpisodeCreate {
        severity,
        resolved_at: None,
    }
    .into();
    active.site = Set("martigny".to_string());
    active.parameter = Set("temperature".to_string());
    active
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::connect("sqlite::memory:").await?;
    create_schema(&db).await?;

    let (first, status) = upsert::<Episode, _>(&db, breach(1)).await?;
    println!("first breach:    {status:?} {}", first.id);

    let (again, status) = upsert::<Episode, _>(&db, breach(2)).await?;
    println!(
        "it worsens:      {status:?} {} (the same episode)",
        again.id
    );

    ActiveModel {
        id: Set(again.id),
        resolved_at: Set(Some(Utc::now())),
        ..Default::default()
    }
    .update(&db)
    .await?;
    println!("value recovers:  the episode is resolved and leaves the index");

    let (reraised, status) = upsert::<Episode, _>(&db, breach(1)).await?;
    println!(
        "it breaches again: {status:?} {} (a new episode)",
        reraised.id
    );

    for episode in Entity::find()
        .order_by_asc(Column::Severity)
        .all(&db)
        .await?
    {
        println!(
            "  {} severity {} resolved {:?}",
            episode.id, episode.severity, episode.resolved_at
        );
    }
    Ok(())
}

async fn create_schema(db: &DatabaseConnection) -> Result<(), DbErr> {
    db.execute_raw(Statement::from_string(
        sea_orm::DatabaseBackend::Sqlite,
        r"
        CREATE TABLE IF NOT EXISTS episodes (
            id TEXT PRIMARY KEY NOT NULL,
            site TEXT NOT NULL,
            parameter TEXT NOT NULL,
            severity INTEGER NOT NULL,
            resolved_at TEXT
        )
        ",
    ))
    .await?;
    // The index the registration's predicate mirrors. Without it two open episodes for one slot
    // could be stored by two callers racing; without the predicate the registration would never
    // try.
    db.execute_raw(Statement::from_string(
        sea_orm::DatabaseBackend::Sqlite,
        r"
        CREATE UNIQUE INDEX IF NOT EXISTS episodes_open_uniq
            ON episodes (site, parameter) WHERE resolved_at IS NULL
        ",
    ))
    .await?;
    Ok(())
}
