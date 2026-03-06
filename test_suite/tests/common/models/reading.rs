//! Composite-PK entity with aggregate-only (no CRUDResource).
//!
//! This mimics a TimescaleDB hypertable with (parameter_id, time) composite PK.
//! Only aggregate support is generated — no CRUD endpoints or models.

use chrono::{DateTime, Utc};
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "readings")]
#[crudcrate(
    api_struct = "ReadingApi",
    // No generate_router → aggregate-only mode
    aggregate(
        time_column = "time",
        intervals("1 hour", "1 day", "1 week", "1 month"),
        metrics("value"),
        group_by("parameter_id"),
    )
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(filterable)]
    pub parameter_id: Uuid,

    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(filterable, sortable)]
    pub time: DateTime<Utc>,

    #[crudcrate(filterable)]
    pub value: f64,

    pub logged: Option<bool>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
