//! Test that a `#[schema(...)]` attribute on an entity field reaches the generated models, so a
//! jsonb column can declare the shape it actually holds.

use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use utoipa::PartialSchema;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "events")]
#[crudcrate(api_struct = "Event")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[schema(value_type = Vec<String>)]
    pub errors: Option<serde_json::Value>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

fn main() {
    let schema = serde_json::to_value(Event::schema()).expect("the schema serialises");
    let errors = &schema["properties"]["errors"];
    assert_eq!(
        errors["type"], "array",
        "errors declares the array the column holds: {schema}"
    );
    assert_eq!(
        errors["items"]["type"], "string",
        "of strings: {schema}"
    );

    let list = serde_json::to_value(EventList::schema()).expect("the list schema serialises");
    assert_eq!(
        list["properties"]["errors"]["type"], "array",
        "the list a client reads says the same: {list}"
    );
}
