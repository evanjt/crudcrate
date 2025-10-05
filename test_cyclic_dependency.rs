use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "customers")]
#[crudcrate(api_struct = "Customer", generate_router)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update))]
    pub id: i32,

    #[crudcrate(filterable, sortable)]
    pub name: String,

    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr = true, exclude(create, update), join(all))]  // No depth parameter
    pub vehicles: Vec<Vehicle>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "Vehicle")]
    Vehicles,
}

impl Related<Vehicle> for Entity {
    fn to() -> RelationDef {
        Relation::Vehicles.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "vehicles")]
#[crudcrate(api_struct = "Vehicle", generate_router)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update))]
    pub id: i32,

    #[crudcrate(filterable)]
    pub customer_id: i32,

    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr = true, exclude(create, update), join(one))]  // No depth parameter
    pub customer: Option<Customer>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::Customer",
        from = "Column::CustomerId",
        to = "super::Customer::Column::Id"
    )]
    Customer,
}

impl Related<super::Customer> for Entity {
    fn to() -> RelationDef {
        Relation::Customer.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

fn main() {
    println!("This example demonstrates cyclic dependency detection.");
}