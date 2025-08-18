// Test to verify that fields marked with create_model=false are properly excluded
// from generated Create structs.

use chrono::{DateTime, Utc};
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "test_entity")]
#[crudcrate(api_struct = "TestEntity")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, create_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable)]
    pub name: String,

    #[crudcrate(create_model = false, on_create = chrono::Utc::now())]
    pub created_at: DateTime<Utc>,

    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr = true, default = vec![], create_model = false)]
    pub excluded_non_db_field: Vec<String>,

    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr = true, default = vec![])]
    pub included_non_db_field: Vec<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Set;

    #[test]
    fn test_create_model_only_includes_allowed_fields() {
        // This test creates a TestEntityCreate struct and verifies that:
        // 1. Fields with create_model=false are NOT included
        // 2. Fields without create_model=false are included
        // 3. This test will COMPILE SUCCESSFULLY ONLY when the bug is fixed

        // If the bug exists, this test will fail to compile because excluded fields are included
        // If the bug is fixed, this test will compile and pass

        let create_data = TestEntityCreate {
            name: "Test Entity".to_string(),
            included_non_db_field: vec!["allowed".to_string()],
        };

        assert_eq!(create_data.name, "Test Entity");
        assert_eq!(create_data.included_non_db_field, vec!["allowed".to_string()]);
    }

    #[test]
    fn test_create_to_active_model_conversion() {
        // Verify that the conversion from Create model to ActiveModel works correctly
        // and that excluded fields are still set via on_create expressions
        
        let create_data = TestEntityCreate {
            name: "Test Conversion".to_string(),
            included_non_db_field: vec!["test".to_string()],
        };

        let active_model: ActiveModel = create_data.into();

        // Verify that fields from the Create struct are set
        assert_eq!(active_model.name, Set("Test Conversion".to_string()));

        // Verify that excluded fields are automatically set via on_create expressions
        assert!(matches!(active_model.id, Set(_)));
        assert!(matches!(active_model.created_at, Set(_)));
    }

}