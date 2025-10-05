// This file should trigger a compile-time error due to cyclic dependency
// Customer -> vehicles -> customer creates infinite recursion

use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "customers")]
#[crudcrate(api_struct = "Customer", generate_router)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub name: String,

    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr = true, exclude(create, update), join(all))]
    pub vehicles: Vec<vehicle::Model>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "vehicle::Entity")]
    Vehicles,
}

impl Related<vehicle::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Vehicles.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

mod vehicle {
    use super::*;
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
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

        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, exclude(create, update), join(one))]
        pub customer: Option<super::Model>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::Entity",
            from = "Column::CustomerId",
            to = "super::Column::Id"
        )]
        Customer,
    }

    impl Related<super::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Customer.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}