use chrono::{DateTime, Utc};
use crudcrate::{traits::CRUDResource, EntityToModels, JoinField};
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, EntityToModels, DeriveEntityModel)]
#[sea_orm(table_name = "customers")]
#[crudcrate(api_struct = "Customer", generate_router)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,
    #[crudcrate(filterable, sortable)]
    pub name: String,
    #[crudcrate(filterable)]
    pub email: String,
    #[crudcrate(sortable, exclude(all,one), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
    #[crudcrate(sortable, exclude(one,all), on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,

    // Join field for vehicles - automatically loaded with join(one, all)
    // Using JoinField wrapper in source Model, unwrapped to Vec in API struct with #[schema(no_recursion)]
    // Using module path to reference the vehicle Model (not API struct)
    // depth=1 means: Customer -> Vehicles (1 level deep, vehicles won't load their nested parts)
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all, depth = 2))]
    pub vehicles: JoinField<Vec<super::vehicle::Model>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::vehicle::Entity")]
    Vehicles,
}

impl Related<super::vehicle::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Vehicles.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
