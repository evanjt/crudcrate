use chrono::{DateTime, Utc};
use crudcrate::{traits::CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, EntityToModels, DeriveEntityModel)]
#[sea_orm(table_name = "vehicles")]
#[crudcrate(api_struct = "Vehicle", generate_router)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,
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
    #[crudcrate(sortable, exclude(one, all, create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
    #[crudcrate(sortable, exclude(one, all, create, update), on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,

    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all, depth = 1))]
    pub parts: Vec<super::vehicle_part::VehiclePart>,

    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all, depth = 1))]
    pub maintenance_records: Vec<crate::models::maintenance_record::MaintenanceRecord>,
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
