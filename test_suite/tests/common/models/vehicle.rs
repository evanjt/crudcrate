use chrono::{DateTime, Utc};
use crudcrate::{EntityToModels, traits::CRUDResource};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::customer::Customer;
use super::maintenance_record::MaintenanceRecord;
use super::vehicle_part::VehiclePart;

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    utoipa::ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum FuelType {
    #[sea_orm(string_value = "Gasoline")]
    Gasoline,
    #[sea_orm(string_value = "Diesel")]
    Diesel,
    #[sea_orm(string_value = "Electric")]
    Electric,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    utoipa::ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum Transmission {
    #[sea_orm(string_value = "Manual")]
    Manual,
    #[sea_orm(string_value = "Automatic")]
    Automatic,
    #[sea_orm(string_value = "CVT")]
    Cvt,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "vehicles")]
#[crudcrate(api_struct = "Vehicle", generate_router, derive_partial_eq)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,
    #[crudcrate(filterable)]
    pub customer_id: Uuid,
    #[crudcrate(filterable, sortable)]
    pub make: String,
    #[crudcrate(filterable, sortable)]
    #[allow(clippy::struct_field_names)]
    pub model: String,
    #[crudcrate(filterable, sortable)]
    pub year: i32,
    #[crudcrate(filterable)]
    pub vin: String,
    #[crudcrate(filterable, sortable)]
    pub fuel_type: Option<FuelType>,
    #[crudcrate(filterable, sortable)]
    pub transmission: Option<Transmission>,
    #[crudcrate(sortable, exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
    #[crudcrate(sortable, exclude(create, update), on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,
    #[crudcrate(filterable, exclude(scoped, create), on_create = false)]
    pub is_private: bool,
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr = true, exclude(create, update), join(one, depth = 1))]
    pub customer: Option<Customer>,
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr = true, exclude(create, update), join(one, all, depth = 3))]
    pub parts: Vec<VehiclePart>,
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr = true, exclude(create, update), join(one, all, depth = 3))]
    pub maintenance_records: Vec<MaintenanceRecord>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::customer::Entity",
        from = "Column::CustomerId",
        to = "super::customer::Column::Id"
    )]
    Customer,

    #[sea_orm(has_many = "super::vehicle_part::Entity")]
    Parts,

    #[sea_orm(has_many = "super::maintenance_record::Entity")]
    MaintenanceRecords,
}

impl Related<super::customer::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Customer.def()
    }
}

impl Related<super::vehicle_part::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Parts.def()
    }
}

impl Related<super::maintenance_record::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MaintenanceRecords.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
