//! Sensor reading entity with explicit aggregates including first/last.

use chrono::{DateTime, Utc};
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "sensor_readings_ext")]
#[crudcrate(
    api_struct = "SensorReadingExt",
    // No generate_router → aggregate-only mode
    aggregate(
        time_column = "recorded_at",
        intervals("1 hour", "1 day"),
        metrics("value", "temperature"),
        group_by("site_id"),
        aggregates(avg, min, max, first, last),
    )
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(filterable)]
    pub site_id: Uuid,

    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(filterable, sortable)]
    pub recorded_at: DateTime<Utc>,

    #[crudcrate(filterable)]
    pub value: f64,

    pub temperature: f64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
