// Comprehensive tests for EntityToModels macro functionality
//
// Tests cover:
// - Create, Update, and List model generation
// - Field exclusions and inclusions
// - use_target_models functionality
// - Complex entity relationships
// - Macro attribute validation

use chrono::{DateTime, Utc};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::Set;
use uuid::Uuid;

// Test entity similar to Tray - represents a plate used in laboratory experiments
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "plate")]
#[crudcrate(api_struct = "Plate")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, update_model = false, create_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(sortable, filterable, list_model = false)]
    pub plate_configuration_id: Uuid,

    #[crudcrate(sortable, filterable)]
    pub order_sequence: i32,

    #[crudcrate(sortable, filterable)]
    pub rotation_degrees: i32,

    #[crudcrate(sortable, filterable, fulltext)]
    pub name: Option<String>,

    #[crudcrate(sortable, filterable)]
    pub qty_x_axis: Option<i32>,

    #[crudcrate(sortable, filterable)]
    pub qty_y_axis: Option<i32>,

    #[crudcrate(sortable, filterable)]
    pub well_relative_diameter: Option<i32>, // Use i32 instead of Decimal for simplicity

    #[crudcrate(update_model = false, create_model = false, on_create = chrono::Utc::now(), sortable, list_model = false)]
    pub created_at: DateTime<Utc>,

    #[crudcrate(update_model = false, create_model = false, on_update = chrono::Utc::now(), on_create = chrono::Utc::now(), sortable, list_model = false)]
    pub last_updated: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Entity that uses target models - put in module to avoid conflicts
mod plate_configuration {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "plate_configurations")]
    #[crudcrate(
        generate_router,
        api_struct = "PlateConfiguration",
        name_singular = "plate_configuration",
        name_plural = "plate_configurations",
        description = "This endpoint manages plate configurations for laboratory experiments."
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, update_model = false, create_model = false, on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[sea_orm(column_type = "Text", nullable, unique)]
        #[crudcrate(sortable, filterable, fulltext)]
        pub name: Option<String>,

        #[crudcrate(sortable, filterable)]
        pub experiment_default: bool,

        #[crudcrate(update_model = false, create_model = false, on_create = chrono::Utc::now(), sortable, list_model = false)]
        pub created_at: DateTime<Utc>,

        #[crudcrate(update_model = false, create_model = false, on_update = chrono::Utc::now(), on_create = chrono::Utc::now(), sortable, list_model = false)]
        pub last_updated: DateTime<Utc>,

        // This field should use PlateCreate in the generated PlateConfigurationCreate model
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, default = vec![], use_target_models)]
        pub plates: Vec<super::Plate>,

        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, default = vec![], list_model = false)]
        pub associated_experiments: Vec<String>, // Simplified for testing
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// Simple entity for testing basic model generation
mod simple_item {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "simple_item")]
    #[crudcrate(api_struct = "SimpleItem")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, create_model = false, update_model = false, on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[crudcrate(filterable)]
        pub description: Option<String>,

        #[crudcrate(filterable)]
        pub active: bool,

        #[crudcrate(create_model = false, update_model = false, on_create = chrono::Utc::now())]
        pub created_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crudcrate::traits::MergeIntoActiveModel;
    use plate_configuration::{
        PlateConfiguration, PlateConfigurationCreate, PlateConfigurationUpdate,
    };
    use simple_item::{
        ActiveModel as SimpleItemActiveModel, SimpleItem, SimpleItemCreate, SimpleItemUpdate,
    };

    #[test]
    fn test_plate_create_model_generated() {
        // Test that PlateCreate model is generated with correct fields
        let create = PlateCreate {
            plate_configuration_id: Uuid::new_v4(),
            order_sequence: 1,
            rotation_degrees: 90,
            name: Some("Test Plate".to_string()),
            qty_x_axis: Some(8),
            qty_y_axis: Some(12),
            well_relative_diameter: Some(500), // 5.00 as int
        };

        // Verify all fields are accessible
        assert!(!create.plate_configuration_id.to_string().is_empty());
        assert_eq!(create.order_sequence, 1);
        assert_eq!(create.rotation_degrees, 90);
        assert_eq!(create.name, Some("Test Plate".to_string()));
        assert_eq!(create.qty_x_axis, Some(8));
        assert_eq!(create.qty_y_axis, Some(12));
        assert_eq!(create.well_relative_diameter, Some(500));

        // Test conversion to ActiveModel
        let active_model: ActiveModel = create.into();

        // Verify database fields are set correctly
        assert!(matches!(active_model.plate_configuration_id, Set(_)));
        assert_eq!(active_model.order_sequence, Set(1));
        assert_eq!(active_model.rotation_degrees, Set(90));
        assert_eq!(active_model.name, Set(Some("Test Plate".to_string())));
        assert_eq!(active_model.qty_x_axis, Set(Some(8)));
        assert_eq!(active_model.qty_y_axis, Set(Some(12)));
        assert_eq!(active_model.well_relative_diameter, Set(Some(500)));

        // Auto-generated fields should be set
        assert!(matches!(active_model.id, Set(_)));
        assert!(matches!(active_model.created_at, Set(_)));
        assert!(matches!(active_model.last_updated, Set(_)));
    }

    #[test]
    fn test_plate_update_model_generated() {
        // Test that PlateUpdate model is generated with double-option pattern
        let update = PlateUpdate {
            plate_configuration_id: Some(Some(Uuid::new_v4())),
            order_sequence: Some(Some(2)),
            rotation_degrees: Some(Some(180)),
            name: Some(Some("Updated Plate".to_string())),
            qty_x_axis: Some(Some(10)),
            qty_y_axis: Some(Some(16)),
            well_relative_diameter: Some(Some(750)), // 7.50 as int
        };

        // Test merging with existing ActiveModel
        let existing = ActiveModel {
            id: Set(Uuid::new_v4()),
            plate_configuration_id: Set(Uuid::new_v4()),
            order_sequence: Set(1),
            rotation_degrees: Set(90),
            name: Set(Some("Original Plate".to_string())),
            qty_x_axis: Set(Some(8)),
            qty_y_axis: Set(Some(12)),
            well_relative_diameter: Set(Some(500)),
            created_at: Set(Utc::now()),
            last_updated: Set(Utc::now()),
        };

        let merged = update.merge_into_activemodel(existing).unwrap();

        // Verify updated fields
        assert!(matches!(merged.plate_configuration_id, Set(_)));
        assert_eq!(merged.order_sequence, Set(2));
        assert_eq!(merged.rotation_degrees, Set(180));
        assert_eq!(merged.name, Set(Some("Updated Plate".to_string())));
        assert_eq!(merged.qty_x_axis, Set(Some(10)));
        assert_eq!(merged.qty_y_axis, Set(Some(16)));
        assert_eq!(merged.well_relative_diameter, Set(Some(750)));

        // Auto-updated fields should be set
        assert!(matches!(merged.last_updated, Set(_)));
    }

    #[test]
    fn test_plate_list_model_generated() {
        // Test that PlateList model is generated with correct field exclusions
        let plate = Plate {
            id: Uuid::new_v4(),
            plate_configuration_id: Uuid::new_v4(),
            order_sequence: 1,
            rotation_degrees: 90,
            name: Some("Test Plate".to_string()),
            qty_x_axis: Some(8),
            qty_y_axis: Some(12),
            well_relative_diameter: Some(500),
            created_at: Utc::now(),
            last_updated: Utc::now(),
        };

        let list_model = PlateList::from(plate);

        // Verify included fields
        assert!(!list_model.id.to_string().is_empty());
        assert_eq!(list_model.order_sequence, 1);
        assert_eq!(list_model.rotation_degrees, 90);
        assert_eq!(list_model.name, Some("Test Plate".to_string()));
        assert_eq!(list_model.qty_x_axis, Some(8));
        assert_eq!(list_model.qty_y_axis, Some(12));
        assert_eq!(list_model.well_relative_diameter, Some(500));

        // Fields with list_model = false should be excluded (compile-time check)
        // The following would cause compile errors if uncommented:
        // let _ = list_model.plate_configuration_id; // Should not exist
        // let _ = list_model.created_at; // Should not exist
        // let _ = list_model.last_updated; // Should not exist
    }

    #[test]
    fn test_simple_item_models_generation() {
        // Test basic model generation without complex features
        let create = SimpleItemCreate {
            name: "Simple Item".to_string(),
            description: Some("A simple test item".to_string()),
            active: true,
        };

        // Test Create model
        assert_eq!(create.name, "Simple Item");
        assert_eq!(create.description, Some("A simple test item".to_string()));
        assert_eq!(create.active, true);

        // Test conversion to ActiveModel
        let active_model: SimpleItemActiveModel = create.into();
        assert_eq!(active_model.name, Set("Simple Item".to_string()));
        assert_eq!(
            active_model.description,
            Set(Some("A simple test item".to_string()))
        );
        assert_eq!(active_model.active, Set(true));
        assert!(matches!(active_model.id, Set(_)));
        assert!(matches!(active_model.created_at, Set(_)));

        // Test Update model
        let update = SimpleItemUpdate {
            name: Some(Some("Updated Simple Item".to_string())),
            description: Some(None), // Set to null
            active: Some(Some(false)),
        };

        let existing = SimpleItemActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set("Original".to_string()),
            description: Set(Some("Original desc".to_string())),
            active: Set(true),
            created_at: Set(Utc::now()),
        };

        let merged = update.merge_into_activemodel(existing).unwrap();
        assert_eq!(merged.name, Set("Updated Simple Item".to_string()));
        assert_eq!(merged.description, Set(None)); // Should be set to null
        assert_eq!(merged.active, Set(false));
    }

    #[test]
    fn test_field_exclusions_and_auto_generation() {
        // Test that excluded fields are not in Create/Update models
        // and auto-generated fields work correctly

        let create = PlateCreate {
            plate_configuration_id: Uuid::new_v4(),
            order_sequence: 1,
            rotation_degrees: 0,
            name: Some("Test".to_string()),
            qty_x_axis: None,
            qty_y_axis: None,
            well_relative_diameter: None,
        };

        // Fields with create_model = false should not be accessible
        // The following would cause compile errors if uncommented:
        // let _ = create.id; // Should not exist
        // let _ = create.created_at; // Should not exist
        // let _ = create.last_updated; // Should not exist

        let active_model: ActiveModel = create.into();

        // Auto-generated fields should be set even though they're not in Create model
        assert!(matches!(active_model.id, Set(_)));
        assert!(matches!(active_model.created_at, Set(_)));
        assert!(matches!(active_model.last_updated, Set(_)));
    }

    #[test]
    fn test_optional_field_handling() {
        // Test that optional fields work correctly in all model types
        let create_with_none = PlateCreate {
            plate_configuration_id: Uuid::new_v4(),
            order_sequence: 1,
            rotation_degrees: 0,
            name: None,
            qty_x_axis: None,
            qty_y_axis: None,
            well_relative_diameter: None,
        };

        let active_model: ActiveModel = create_with_none.into();
        assert_eq!(active_model.name, Set(None));
        assert_eq!(active_model.qty_x_axis, Set(None));
        assert_eq!(active_model.qty_y_axis, Set(None));
        assert_eq!(active_model.well_relative_diameter, Set(None));

        // Test update with Some(None) to explicitly set to null
        let update = PlateUpdate {
            plate_configuration_id: None,
            order_sequence: None,
            rotation_degrees: None,
            name: Some(None),                   // Explicitly set to null
            qty_x_axis: Some(Some(5)),          // Set to Some value
            qty_y_axis: None,                   // Don't update
            well_relative_diameter: Some(None), // Explicitly set to null
        };

        let existing = ActiveModel {
            id: Set(Uuid::new_v4()),
            plate_configuration_id: Set(Uuid::new_v4()),
            order_sequence: Set(1),
            rotation_degrees: Set(0),
            name: Set(Some("Original".to_string())),
            qty_x_axis: Set(Some(8)),
            qty_y_axis: Set(Some(12)),
            well_relative_diameter: Set(Some(500)),
            created_at: Set(Utc::now()),
            last_updated: Set(Utc::now()),
        };

        let merged = update.merge_into_activemodel(existing).unwrap();
        assert_eq!(merged.name, Set(None)); // Should be null
        assert_eq!(merged.qty_x_axis, Set(Some(5))); // Should be updated
        assert!(matches!(merged.qty_y_axis, sea_orm::ActiveValue::NotSet)); // Should not be updated
        assert_eq!(merged.well_relative_diameter, Set(None)); // Should be null
    }

    #[test]
    fn test_crud_resource_trait_implementation() {
        // Test that the generated models properly implement CRUDResource
        // This is an indirect test that the macro generates the correct associated types

        // These type assertions will fail at compile time if the macro doesn't generate correctly
        fn assert_crud_resource<T>()
        where
            T: CRUDResource,
            T::CreateModel: Send,
            T::UpdateModel: Send + Sync,
            T::ListModel: Send + Sync,
        {
            // Type constraints ensure the macro generated the correct associated types
        }

        assert_crud_resource::<Plate>();
        assert_crud_resource::<PlateConfiguration>();
        assert_crud_resource::<SimpleItem>();

        // Test the constants are generated correctly
        assert_eq!(Plate::RESOURCE_NAME_SINGULAR, "plate");
        assert_eq!(Plate::RESOURCE_NAME_PLURAL, "plates");

        assert_eq!(
            PlateConfiguration::RESOURCE_NAME_SINGULAR,
            "plate_configuration"
        );
        assert_eq!(
            PlateConfiguration::RESOURCE_NAME_PLURAL,
            "plate_configurations"
        );

        assert_eq!(SimpleItem::RESOURCE_NAME_SINGULAR, "simple_item");
        assert_eq!(SimpleItem::RESOURCE_NAME_PLURAL, "simple_items");
    }

    #[test]
    fn test_serialization_and_deserialization() {
        // Test that generated models can be serialized/deserialized
        let create = PlateCreate {
            plate_configuration_id: Uuid::new_v4(),
            order_sequence: 1,
            rotation_degrees: 90,
            name: Some("Serializable Plate".to_string()),
            qty_x_axis: Some(8),
            qty_y_axis: Some(12),
            well_relative_diameter: Some(500),
        };

        // Test serialization
        let json = serde_json::to_string(&create).unwrap();
        assert!(json.contains("Serializable Plate"));
        assert!(json.contains("\"order_sequence\":1"));

        // Test deserialization
        let deserialized: PlateCreate = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, Some("Serializable Plate".to_string()));
        assert_eq!(deserialized.order_sequence, 1);
        assert_eq!(deserialized.rotation_degrees, 90);

        // Test update model serialization with double-option pattern
        let update = PlateUpdate {
            plate_configuration_id: None,
            order_sequence: Some(Some(2)),
            rotation_degrees: Some(Some(180)),
            name: Some(Some("Updated".to_string())),
            qty_x_axis: Some(None), // Explicitly null
            qty_y_axis: None,       // Not provided
            well_relative_diameter: Some(Some(750)),
        };

        let update_json = serde_json::to_string(&update).unwrap();
        assert!(update_json.contains("\"name\":\"Updated\""));
        assert!(update_json.contains("\"qty_x_axis\":null"));

        let deserialized_update: PlateUpdate = serde_json::from_str(&update_json).unwrap();
        assert_eq!(deserialized_update.name, Some(Some("Updated".to_string())));
        assert_eq!(deserialized_update.qty_x_axis, Some(None));
        assert_eq!(deserialized_update.qty_y_axis, None);
    }

    #[test]
    fn test_create_and_update_models_exist() {
        // Compile-time verification that the expected models are generated

        // These type assertions ensure the macro generated all expected types
        let _plate_create: PlateCreate = PlateCreate {
            plate_configuration_id: Uuid::new_v4(),
            order_sequence: 1,
            rotation_degrees: 0,
            name: None,
            qty_x_axis: None,
            qty_y_axis: None,
            well_relative_diameter: None,
        };

        let _plate_update: PlateUpdate = PlateUpdate {
            plate_configuration_id: None,
            order_sequence: None,
            rotation_degrees: None,
            name: None,
            qty_x_axis: None,
            qty_y_axis: None,
            well_relative_diameter: None,
        };

        let _plate_list: PlateList = PlateList {
            id: Uuid::new_v4(),
            order_sequence: 1,
            rotation_degrees: 0,
            name: None,
            qty_x_axis: None,
            qty_y_axis: None,
            well_relative_diameter: None,
        };

        let _simple_create: SimpleItemCreate = SimpleItemCreate {
            name: "test".to_string(),
            description: None,
            active: false,
        };

        let _simple_update: SimpleItemUpdate = SimpleItemUpdate {
            name: None,
            description: None,
            active: None,
        };

        // If we get here, all the expected model types were generated successfully
        assert!(true, "All expected model types are generated by the macro");
    }

    #[test]
    fn test_macro_field_attributes() {
        // Test various combinations of field attributes work correctly

        // Test that primary key fields are excluded from Create/Update models
        let create = PlateCreate {
            plate_configuration_id: Uuid::new_v4(),
            order_sequence: 1,
            rotation_degrees: 0,
            name: Some("Attribute Test".to_string()),
            qty_x_axis: Some(10),
            qty_y_axis: Some(10),
            well_relative_diameter: Some(100),
        };

        // Test that list_model = false excludes fields from List model
        let plate = Plate {
            id: Uuid::new_v4(),
            plate_configuration_id: Uuid::new_v4(), // This should be excluded from PlateList
            order_sequence: 1,
            rotation_degrees: 0,
            name: Some("List Test".to_string()),
            qty_x_axis: Some(10),
            qty_y_axis: Some(10),
            well_relative_diameter: Some(100),
            created_at: Utc::now(),
            last_updated: Utc::now(),
        };

        let list_model = PlateList::from(plate);

        // Verify that fields marked with list_model = false are excluded
        assert_eq!(list_model.order_sequence, 1);
        assert_eq!(list_model.name, Some("List Test".to_string()));

        // The following should cause compilation errors if uncommented (fields excluded from list):
        // let _ = list_model.plate_configuration_id; // excluded by list_model = false
        // let _ = list_model.created_at; // excluded by list_model = false
        // let _ = list_model.last_updated; // excluded by list_model = false

        // Test that auto-generation works
        let active_model: ActiveModel = create.into();
        assert!(matches!(active_model.id, Set(_))); // Should be auto-generated
        assert!(matches!(active_model.created_at, Set(_))); // Should be auto-generated
        assert!(matches!(active_model.last_updated, Set(_))); // Should be auto-generated
    }

    #[test]
    fn test_use_target_models_functionality() {
        // This test validates that use_target_models properly converts
        // Vec<Plate> declarations to Vec<PlateCreate> in generated Create models
        
        let plate_create = PlateCreate {
            plate_configuration_id: Uuid::new_v4(),
            order_sequence: 1,
            rotation_degrees: 0,
            name: Some("Test Plate".to_string()),
            qty_x_axis: Some(8),
            qty_y_axis: Some(12),
            well_relative_diameter: Some(500),
        };

        // This should work when use_target_models is implemented:
        // PlateConfigurationCreate.plates should accept Vec<PlateCreate>
        let config_create = PlateConfigurationCreate {
            name: Some("Test Configuration".to_string()),
            experiment_default: true,
            plates: vec![plate_create], // Should accept PlateCreate when use_target_models is implemented
            associated_experiments: vec!["exp1".to_string()],
        };

        assert_eq!(config_create.plates.len(), 1);
        assert_eq!(config_create.plates[0].name, Some("Test Plate".to_string()));
        assert_eq!(config_create.plates[0].order_sequence, 1);
    }

    #[test]
    fn test_use_target_models_for_update_models() {
        // Test that use_target_models also works for Update models
        let plate_update = PlateUpdate {
            plate_configuration_id: Some(Some(Uuid::new_v4())),
            order_sequence: Some(Some(2)),
            rotation_degrees: Some(Some(180)),
            name: Some(Some("Updated Plate".to_string())),
            qty_x_axis: Some(Some(10)),
            qty_y_axis: Some(Some(16)),
            well_relative_diameter: Some(Some(750)),
        };

        let config_update = PlateConfigurationUpdate {
            name: Some(Some("Updated Configuration".to_string())),
            experiment_default: Some(Some(false)),
            plates: vec![plate_update], // Should accept PlateUpdate when use_target_models is implemented
            associated_experiments: vec!["updated_experiment".to_string()],
        };

        assert_eq!(config_update.plates.len(), 1);
        assert_eq!(config_update.plates[0].name, Some(Some("Updated Plate".to_string())));
    }

    #[test]
    fn test_plate_configuration_basic_functionality() {
        // Test basic PlateConfiguration functionality without use_target_models complexity

        // Test PlateConfigurationCreate without nested models for now
        let config_create = PlateConfigurationCreate {
            name: Some("Basic Configuration".to_string()),
            experiment_default: false,
            plates: vec![], // Empty for now until use_target_models is implemented
            associated_experiments: vec!["experiment1".to_string()],
        };

        assert_eq!(config_create.name, Some("Basic Configuration".to_string()));
        assert_eq!(config_create.experiment_default, false);
        assert_eq!(config_create.plates.len(), 0);
        assert_eq!(config_create.associated_experiments.len(), 1);

        // Test PlateConfigurationUpdate
        let config_update = PlateConfigurationUpdate {
            name: Some(Some("Updated Configuration".to_string())),
            experiment_default: Some(Some(true)),
            plates: vec![], // Empty for now
            associated_experiments: vec!["updated_experiment".to_string()],
        };

        assert_eq!(
            config_update.name,
            Some(Some("Updated Configuration".to_string()))
        );
        assert_eq!(config_update.experiment_default, Some(Some(true)));
        assert_eq!(config_update.plates.len(), 0);
    }

    // NOTE: use_target_models tests are commented out until the feature is fully implemented
    // The tests above demonstrate that use_target_models is not yet working as expected
    // When use_target_models is implemented, PlateConfigurationCreate.plates should expect Vec<PlateCreate>
    // instead of Vec<Plate>

    /* TODO: Uncomment and fix these tests when use_target_models is implemented

    #[test]
    fn test_use_target_models_implementation() {
        // This test will validate that use_target_models properly converts
        // Vec<Plate> declarations to Vec<PlateCreate> in generated Create models

        let plate_create = PlateCreate {
            plate_configuration_id: Uuid::new_v4(),
            order_sequence: 1,
            rotation_degrees: 0,
            name: Some("Target Model Plate".to_string()),
            qty_x_axis: Some(8),
            qty_y_axis: Some(12),
            well_relative_diameter: Some(500),
        };

        // This should work when use_target_models is implemented
        let config_create = PlateConfigurationCreate {
            name: Some("Target Models Config".to_string()),
            experiment_default: true,
            plates: vec![plate_create], // Should accept PlateCreate when implemented
            associated_experiments: vec!["test".to_string()],
        };

        assert_eq!(config_create.plates.len(), 1);
    }

    */
}
