// Comprehensive integration tests for crudcrate-derive macros
//
// Tests cover all major functionality:
// - ToCreateModel derive macro
// - ToUpdateModel derive macro
// - ActiveModel conversion and merging
// - Serialization/deserialization
// - OpenAPI schema generation
// - Field exclusions and auto-generation
use chrono::{DateTime, Utc};
use crudcrate::{ToCreateModel, ToUpdateModel, traits::MergeIntoActiveModel};
use sea_orm::{ActiveValue, entity::prelude::*};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// Mock Sea-ORM entity
mod test_entity {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "test_items")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub name: String,
        pub description: Option<String>,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// Simple test model
#[derive(ToSchema, Serialize, Deserialize, ToCreateModel, ToUpdateModel)]
#[active_model = "test_entity::ActiveModel"]
pub struct TestItem {
    #[crudcrate(create_model = false, update_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    #[crudcrate(create_model = false, update_model = false, on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
    #[crudcrate(create_model = false, update_model = false, on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,
}

impl From<test_entity::Model> for TestItem {
    fn from(model: test_entity::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            description: model.description,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

// Additional test entities for extended functionality
mod extended_test_entity {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "extended_test_items")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub name: String,
        pub description: Option<String>,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
        pub active: bool,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// Test model with defaults for ToCreateModel
#[derive(ToSchema, Serialize, Deserialize, ToCreateModel, ToUpdateModel)]
#[active_model = "extended_test_entity::ActiveModel"]
pub struct TestItemWithDefaults {
    #[crudcrate(create_model = false, update_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,
    #[crudcrate(on_create = "default_name".to_string())]
    pub name: String,
    pub description: Option<String>,
    #[crudcrate(create_model = false, update_model = false, on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
    #[crudcrate(create_model = false, update_model = false, on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,
    #[crudcrate(on_create = true)]
    pub active: bool,
}

// Test model with non-database attributes
#[derive(ToSchema, Serialize, Deserialize, ToCreateModel, ToUpdateModel)]
#[active_model = "test_entity::ActiveModel"]
pub struct TestItemWithNonDbAttrs {
    #[crudcrate(create_model = false, update_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    #[crudcrate(create_model = false, update_model = false, on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
    #[crudcrate(create_model = false, update_model = false, on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,

    // Non-database attributes
    #[crudcrate(non_db_attr = true)]
    pub computed_field: String,

    #[crudcrate(non_db_attr = true, default = "default_value".to_string())]
    pub default_field: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::ActiveValue::*;

    #[test]
    fn test_create_model_basic() {
        let create = TestItemCreate {
            name: "Test Item".to_string(),
            description: Some("A test item".to_string()),
        };

        // Convert to ActiveModel
        let active_model: test_entity::ActiveModel = create.into();

        // Verify conversion
        assert_eq!(active_model.name, Set("Test Item".to_string()));
        assert_eq!(
            active_model.description,
            Set(Some("A test item".to_string()))
        );

        // Auto-generated fields should be set
        assert!(matches!(active_model.id, Set(_)));
        assert!(matches!(active_model.created_at, Set(_)));
        assert!(matches!(active_model.updated_at, Set(_)));
    }

    #[test]
    fn test_create_model_with_none() {
        let create = TestItemCreate {
            name: "Test Item".to_string(),
            description: None,
        };

        let active_model: test_entity::ActiveModel = create.into();

        assert_eq!(active_model.name, Set("Test Item".to_string()));
        assert_eq!(active_model.description, Set(None));
    }

    #[test]
    fn test_update_model_basic() {
        // Test double Option pattern
        let update = TestItemUpdate {
            name: Some(Some("Updated Name".to_string())),
            description: Some(Some("Updated description".to_string())),
        };

        let existing = test_entity::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set("Original Name".to_string()),
            description: Set(Some("Original description".to_string())),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };

        let merged = update.merge_into_activemodel(existing).unwrap();

        assert_eq!(merged.name, Set("Updated Name".to_string()));
        assert_eq!(
            merged.description,
            Set(Some("Updated description".to_string()))
        );
        assert!(matches!(merged.updated_at, Set(_)));
    }

    #[test]
    fn test_update_model_partial() {
        let update = TestItemUpdate {
            name: Some(Some("Updated Name".to_string())),
            description: None, // Don't update this field
        };

        let existing = test_entity::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set("Original Name".to_string()),
            description: Set(Some("Keep this".to_string())),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };

        let merged = update.merge_into_activemodel(existing).unwrap();

        assert_eq!(merged.name, Set("Updated Name".to_string()));
        assert!(matches!(merged.description, NotSet)); // Field not provided, so NotSet
    }

    #[test]
    fn test_update_model_set_to_null() {
        let update = TestItemUpdate {
            name: None,
            description: Some(None), // Explicitly set to null
        };

        let existing = test_entity::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set("Original Name".to_string()),
            description: Set(Some("Original description".to_string())),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };

        let merged = update.merge_into_activemodel(existing).unwrap();

        assert!(matches!(merged.name, NotSet)); // Field not provided, so NotSet
        assert_eq!(merged.description, Set(None)); // Should be set to null
    }

    #[test]
    fn test_serialization() {
        let create = TestItemCreate {
            name: "Test Item".to_string(),
            description: Some("Description".to_string()),
        };

        let json = serde_json::to_string(&create).unwrap();
        assert!(json.contains("\"name\":\"Test Item\""));
        assert!(json.contains("\"description\":\"Description\""));

        let deserialized: TestItemCreate = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "Test Item");
        assert_eq!(deserialized.description, Some("Description".to_string()));
    }

    #[test]
    fn test_update_serialization() {
        let update = TestItemUpdate {
            name: Some(Some("Updated".to_string())),
            description: Some(None),
        };

        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("\"name\":\"Updated\""));
        assert!(json.contains("\"description\":null"));
    }

    #[test]
    fn test_merge_into_activemodel() {
        // Test the merge_into_activemodel functionality
        let update = TestItemUpdate {
            name: Some(Some("Merged Name".to_string())),
            description: None, // Should not change
        };

        let existing = test_entity::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set("Original Name".to_string()),
            description: Set(Some("Keep this".to_string())),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };

        let merged = update.merge_into_activemodel(existing).unwrap();

        assert_eq!(merged.name, Set("Merged Name".to_string()));
        assert!(matches!(merged.description, NotSet)); // Field not provided, so NotSet
        assert!(matches!(merged.updated_at, Set(_))); // Should be auto-updated
    }

    #[test]
    fn test_openapi_schema() {
        use utoipa::OpenApi;

        #[derive(OpenApi)]
        #[openapi(components(schemas(TestItem, TestItemCreate, TestItemUpdate)))]
        struct ApiDoc;

        let openapi = ApiDoc::openapi();
        let components = openapi.components.unwrap();

        assert!(components.schemas.contains_key("TestItem"));
        assert!(components.schemas.contains_key("TestItemCreate"));
        assert!(components.schemas.contains_key("TestItemUpdate"));
    }

    // Tests for models with defaults
    #[test]
    fn test_create_model_with_defaults() {
        let create = TestItemWithDefaultsCreate {
            name: None, // Should use default
            description: Some("Custom description".to_string()),
            active: None, // Should use default
        };

        let active_model: extended_test_entity::ActiveModel = create.into();

        // Should use defaults
        assert_eq!(active_model.name, Set("default_name".to_string()));
        assert_eq!(active_model.active, Set(true));

        // Should use provided value
        assert_eq!(
            active_model.description,
            Set(Some("Custom description".to_string()))
        );

        // Auto-generated fields
        assert!(matches!(active_model.id, Set(_)));
        assert!(matches!(active_model.created_at, Set(_)));
        assert!(matches!(active_model.updated_at, Set(_)));
    }

    #[test]
    fn test_create_model_override_defaults() {
        let create = TestItemWithDefaultsCreate {
            name: Some("Custom Name".to_string()),
            description: None,
            active: Some(false),
        };

        let active_model: extended_test_entity::ActiveModel = create.into();

        // Should use provided values, not defaults
        assert_eq!(active_model.name, Set("Custom Name".to_string()));
        assert_eq!(active_model.description, Set(None));
        assert_eq!(active_model.active, Set(false));
    }

    // Tests for non-database attributes
    #[test]
    fn test_non_db_attributes_in_create_model() {
        let create = TestItemWithNonDbAttrsCreate {
            name: "Test Item".to_string(),
            description: Some("A test item".to_string()),
            computed_field: "computed_value".to_string(),
            default_field: None, // Should use default
        };

        // Non-db attributes should be accessible in Create model
        assert_eq!(create.computed_field, "computed_value");
        assert_eq!(create.default_field, None);

        // Convert to ActiveModel - only database fields should be included
        let active_model: test_entity::ActiveModel = create.into();
        assert_eq!(active_model.name, Set("Test Item".to_string()));
        assert_eq!(
            active_model.description,
            Set(Some("A test item".to_string()))
        );

        // Auto-generated fields should be set
        assert!(matches!(active_model.id, Set(_)));
        assert!(matches!(active_model.created_at, Set(_)));
        assert!(matches!(active_model.updated_at, Set(_)));

        // Non-db fields should not be in ActiveModel (this is compile-time enforced)
    }

    #[test]
    fn test_non_db_attributes_in_update_model() {
        let update = TestItemWithNonDbAttrsUpdate {
            name: Some(Some("Updated Item".to_string())),
            description: None, // Don't update
            computed_field: "updated_computed".to_string(),
            default_field: Some("custom_value".to_string()),
        };

        // Non-db attributes should be accessible in Update model
        assert_eq!(update.computed_field, "updated_computed");
        assert_eq!(update.default_field, Some("custom_value".to_string()));

        // Test merge into ActiveModel - only database fields should be merged
        let existing = test_entity::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set("Original".to_string()),
            description: Set(Some("original_description".to_string())),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };

        let merged = update.merge_into_activemodel(existing).unwrap();

        // Only database fields should be merged
        assert_eq!(merged.name, Set("Updated Item".to_string()));
        assert!(matches!(merged.description, NotSet)); // Not provided
        assert!(matches!(merged.updated_at, Set(_))); // Auto-updated
    }

    #[test]
    fn test_non_db_attributes_serialization() {
        let create = TestItemWithNonDbAttrsCreate {
            name: "Serialize Test".to_string(),
            description: Some("Description".to_string()),
            computed_field: "computed".to_string(),
            default_field: Some("custom".to_string()),
        };

        let json = serde_json::to_string(&create).unwrap();
        assert!(json.contains("\"name\":\"Serialize Test\""));
        assert!(json.contains("\"computed_field\":\"computed\""));
        assert!(json.contains("\"default_field\":\"custom\""));

        let deserialized: TestItemWithNonDbAttrsCreate = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.computed_field, "computed");
        assert_eq!(deserialized.default_field, Some("custom".to_string()));
    }

    #[test]
    fn test_update_model_double_option_pattern() {
        // Test that Update model uses Option<Option<T>> for nullable fields
        let update = TestItemUpdate {
            name: Some(Some("Updated Name".to_string())),
            description: Some(Some("Updated description".to_string())),
        };

        // Verify the double Option pattern
        assert_eq!(update.name, Some(Some("Updated Name".to_string())));
        assert_eq!(
            update.description,
            Some(Some("Updated description".to_string()))
        );
    }

    #[test]
    fn test_update_model_json_serialization() {
        let update = TestItemUpdate {
            name: Some(Some("JSON Name".to_string())),
            description: Some(None), // Explicitly null
        };

        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("\"name\":\"JSON Name\""));
        assert!(json.contains("\"description\":null"));

        let deserialized: TestItemUpdate = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, Some(Some("JSON Name".to_string())));
        assert_eq!(deserialized.description, Some(None));
    }

    #[test]
    fn test_update_model_json_deserialization_partial() {
        let json = r#"{"name": "Partial Update"}"#;

        let update: TestItemUpdate = serde_json::from_str(json).unwrap();

        assert_eq!(update.name, Some(Some("Partial Update".to_string())));
        assert_eq!(update.description, None); // Not provided in JSON
    }

    #[test]
    fn test_comprehensive_openapi_schema() {
        use utoipa::OpenApi;

        #[derive(OpenApi)]
        #[openapi(components(schemas(
            TestItem,
            TestItemCreate,
            TestItemUpdate,
            TestItemWithDefaults,
            TestItemWithDefaultsCreate,
            TestItemWithNonDbAttrs,
            TestItemWithNonDbAttrsCreate,
            TestItemWithNonDbAttrsUpdate
        )))]
        struct ApiDoc;

        let openapi = ApiDoc::openapi();
        let components = openapi.components.unwrap();

        // Verify all schemas are present
        assert!(components.schemas.contains_key("TestItem"));
        assert!(components.schemas.contains_key("TestItemCreate"));
        assert!(components.schemas.contains_key("TestItemUpdate"));
        assert!(components.schemas.contains_key("TestItemWithDefaults"));
        assert!(
            components
                .schemas
                .contains_key("TestItemWithDefaultsCreate")
        );
        assert!(components.schemas.contains_key("TestItemWithNonDbAttrs"));
        assert!(
            components
                .schemas
                .contains_key("TestItemWithNonDbAttrsCreate")
        );
        assert!(
            components
                .schemas
                .contains_key("TestItemWithNonDbAttrsUpdate")
        );

        // Verify that non-db attributes are included in schemas
        let create_schema = &components.schemas["TestItemWithNonDbAttrsCreate"];
        let schema_str = serde_json::to_string(&create_schema).unwrap();
        assert!(schema_str.contains("computed_field"));
        assert!(schema_str.contains("default_field"));
    }
}