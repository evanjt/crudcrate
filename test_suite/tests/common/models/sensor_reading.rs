use chrono::{DateTime, Utc};
use crudcrate::{EntityToModels, traits::CRUDResource};
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "sensor_readings")]
#[crudcrate(
    api_struct = "SensorReading",
    generate_router,
    aggregate(
        time_column = "recorded_at",
        intervals("1 hour", "1 day", "1 week", "1 month"),
        metrics("value"),
        group_by("site_id"),
    )
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable)]
    pub site_id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub recorded_at: DateTime<Utc>,

    #[crudcrate(filterable, sortable)]
    pub value: f64,

    #[crudcrate(sortable, exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
