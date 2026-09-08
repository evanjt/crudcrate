//! A response model's `Option` is required in the document; a request's is not.
//!
//! utoipa reads every `Option<T>` as not-required and nullable, so a value the API always sends as
//! null and one a client may omit are described the same way, and a generated client has to handle
//! an absence that never happens. The serialization models say which is which.

use sea_orm::{DatabaseConnection, entity::prelude::*};
use utoipa::OpenApi;

mod catalogue {
    use crudcrate::EntityToModels;
    use sea_orm::entity::prelude::*;
    use uuid::Uuid;

    #[derive(
        Clone,
        Debug,
        PartialEq,
        DeriveEntityModel,
        serde::Serialize,
        serde::Deserialize,
        EntityToModels,
    )]
    #[sea_orm(table_name = "catalogue_items")]
    #[crudcrate(
        api_struct = "CatalogueItem",
        name_singular = "catalogue_item",
        name_plural = "catalogue_items",
        generate_router
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(sortable, filterable)]
        pub name: String,

        pub note: Option<String>,

        #[serde(skip_serializing_if = "Option::is_none")]
        pub omitted: Option<String>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

use catalogue::CatalogueItem;

async fn setup_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(catalogue::Entity).await
}

fn required(document: &serde_json::Value, schema: &str) -> Vec<String> {
    document["components"]["schemas"][schema]["required"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|v| v.as_str().map(ToString::to_string))
        .collect()
}

#[tokio::test]
async fn test_a_response_option_is_required_and_a_request_option_is_not() {
    #[derive(OpenApi)]
    #[openapi(components(schemas(CatalogueItem)))]
    struct Api;
    let (_, api) = utoipa_axum::router::OpenApiRouter::with_openapi(Api::openapi())
        .nest(
            "/catalogue_items",
            CatalogueItem::router(&setup_db().await.expect("db")),
        )
        .split_for_parts();
    let document = serde_json::to_value(&api).expect("the document serialises");

    for schema in [
        "CatalogueItem",
        "CatalogueItemList",
        "CatalogueItemResponse",
    ] {
        assert!(
            required(&document, schema).contains(&"note".to_string()),
            "{schema} always sends note, null when empty: {document}"
        );
        assert!(
            !required(&document, schema).contains(&"omitted".to_string()),
            "{schema} omits a skip_serializing_if field: {document}"
        );
    }

    for schema in ["CatalogueItemCreate", "CatalogueItemUpdate"] {
        assert!(
            !required(&document, schema).contains(&"note".to_string()),
            "{schema} is a request, where an Option is the client's to omit: {document}"
        );
    }
}
