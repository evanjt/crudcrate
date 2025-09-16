use chrono::{DateTime, Utc};
use crudcrate::{EntityToModels, traits::CRUDResource};
use sea_orm::entity::prelude::*;
use uuid::Uuid;

use super::vehicle_part_entity::VehiclePart;
use super::maintenance_record_entity::MaintenanceRecord;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "vehicles")]
#[crudcrate(api_struct = "Vehicle", generate_router)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, create_model = false, update_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,

    // Foreign key to customer
    #[crudcrate(filterable)]
    pub customer_id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub make: String,

    #[crudcrate(filterable, sortable)]
    pub model: String,

    #[crudcrate(filterable, sortable)]
    pub year: i32,

    #[crudcrate(filterable)]
    pub vin: String,

    #[crudcrate(sortable, create_model = false, update_model = false, on_create = Utc::now())]
    pub created_at: DateTime<Utc>,

    #[crudcrate(sortable, create_model = false, update_model = false, on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,

    // Join fields for recursive loading (depth 1 since we don't want infinite recursion)
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr = true, join(all, depth = 1))]
    pub parts: Vec<VehiclePart>,

    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr = true, join(all, depth = 1))]
    pub maintenance_records: Vec<MaintenanceRecord>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::customer_entity::Entity",
        from = "Column::CustomerId",
        to = "super::customer_entity::Column::Id"
    )]
    Customer,

    #[sea_orm(has_many = "super::vehicle_part_entity::Entity")]
    Parts,

    #[sea_orm(has_many = "super::maintenance_record_entity::Entity")]
    MaintenanceRecords,
}

impl Related<super::customer_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Customer.def()
    }
}

impl Related<super::vehicle_part_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Parts.def()
    }
}

impl Related<super::maintenance_record_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MaintenanceRecords.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// Type alias for easier importing
pub type VehicleEntity = Entity;