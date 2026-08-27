use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

pub mod vehicle {
    use super::{DeriveEntityModel, EntityToModels, Uuid};
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "vehicles")]
    #[crudcrate(
        api_struct = "Vehicle",
        name_singular = "vehicle",
        name_plural = "vehicles",
        generate_router,
        derive_partial_eq,
        derive_eq
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,
        #[crudcrate(filterable)]
        pub customer_id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub make: String,
        #[crudcrate(filterable, sortable)]
        pub year: i32,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::customer::Entity",
            from = "Column::CustomerId",
            to = "super::customer::Column::Id"
        )]
        Customer,
    }

    impl Related<super::customer::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Customer.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod customer {
    use super::vehicle::Vehicle;
    use super::{DeriveEntityModel, EntityToModels, Uuid};
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "customers")]
    #[crudcrate(
        api_struct = "Customer",
        name_singular = "customer",
        name_plural = "customers",
        generate_router,
        derive_partial_eq
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,
        #[crudcrate(filterable, sortable)]
        pub name: String,
        #[crudcrate(filterable)]
        pub email: String,

        /// Joined filterable columns declared here enable
        /// `?filter={"vehicles.make":"BMW"}` on the customer endpoint.
        #[sea_orm(ignore)]
        #[crudcrate(
            non_db_attr,
            exclude(create, update),
            join(one, all, depth = 1, filterable("make", "year"))
        )]
        pub vehicles: Vec<Vehicle>,
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
}

pub async fn setup_database(
    database_url: &str,
) -> Result<sea_orm::DatabaseConnection, Box<dyn std::error::Error>> {
    use sea_orm::Database;
    let db = Database::connect(database_url).await?;

    db.execute_raw(sea_orm::Statement::from_string(
        db.get_database_backend(),
        r"CREATE TABLE IF NOT EXISTS customers (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            email TEXT NOT NULL
        );"
        .to_owned(),
    ))
    .await?;

    db.execute_raw(sea_orm::Statement::from_string(
        db.get_database_backend(),
        r"CREATE TABLE IF NOT EXISTS vehicles (
            id TEXT PRIMARY KEY NOT NULL,
            customer_id TEXT NOT NULL,
            make TEXT NOT NULL,
            year INTEGER NOT NULL,
            FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE
        );"
        .to_owned(),
    ))
    .await?;

    Ok(db)
}
