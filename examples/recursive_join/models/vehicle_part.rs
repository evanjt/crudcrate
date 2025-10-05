use chrono::{DateTime, Utc};
use crudcrate::{EntityToModels, traits::CRUDResource};
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "vehicle_parts")]
#[crudcrate(api_struct = "VehiclePart", generate_router)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    // Foreign key to vehicle
    #[crudcrate(filterable)]
    pub vehicle_id: Uuid,

    #[crudcrate(filterable, sortable, fulltext)]
    pub name: String,

    #[crudcrate(filterable)]
    pub part_number: String,

    #[crudcrate(filterable, sortable)]
    pub category: String,

    // #[crudcrate(sortable)]
    // pub price: Option<Decimal>,  // Temporarily disabled for debugging

    #[crudcrate(filterable)]
    pub in_stock: bool,

    #[crudcrate(sortable, exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,

    #[crudcrate(sortable, exclude(create, update), on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::vehicle::Entity",
        from = "Column::VehicleId",
        to = "super::vehicle::Column::Id"
    )]
    Vehicle,
}

impl Related<super::vehicle::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Vehicle.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}