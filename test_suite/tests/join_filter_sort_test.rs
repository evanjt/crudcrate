//! Integration tests for join filtering and sorting functionality.
//!
//! This module tests the ability to filter and sort on related entity columns
//! using dot-notation syntax (e.g., `vehicles.year`, `vehicles.make`).

use chrono::{DateTime, Utc};
use crudcrate::{
    traits::CRUDResource, EntityToModels, SortConfig,
    apply_filters_with_joins, parse_sorting_with_joins, parse_dot_notation,
};
use sea_orm::{
    ActiveModelBehavior, DeriveEntityModel, DeriveRelation, EntityTrait, EnumIter,
    entity::prelude::*,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// ============================================================================
// TEST MODELS
// ============================================================================

// NOTE: Due to circular dependency issues with PartialEq derives on join fields,
// we define Vehicle first so Customer can reference it

/// Vehicle entity - child of customer
pub mod vehicle {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, EntityToModels, Serialize, Deserialize, ToSchema)]
    #[sea_orm(table_name = "vehicles")]
    #[crudcrate(
        api_struct = "Vehicle",
        name_singular = "vehicle",
        name_plural = "vehicles",
        derive_partial_eq,
        derive_eq
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        pub customer_id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub make: String,

        #[crudcrate(filterable, sortable)]
        pub model: String,

        #[crudcrate(filterable, sortable)]
        pub year: i32,

        #[crudcrate(filterable, sortable)]
        pub color: String,

        #[crudcrate(filterable, sortable)]
        pub mileage: i32,
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

/// Customer entity - parent with join to vehicles
pub mod customer {
    use super::*;

    #[derive(Clone, Debug, DeriveEntityModel, EntityToModels, Serialize, Deserialize, ToSchema)]
    #[sea_orm(table_name = "customers")]
    #[crudcrate(
        api_struct = "Customer",
        name_singular = "customer",
        name_plural = "customers",
        description = "Customer with vehicles relationship",
        no_partial_eq,
        no_eq
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[crudcrate(filterable)]
        pub email: String,

        #[crudcrate(sortable, exclude(create, update), on_create = Utc::now())]
        pub created_at: DateTime<Utc>,

        /// Vehicles relationship - with join_filterable and join_sortable
        #[sea_orm(ignore)]
        #[crudcrate(
            non_db_attr,
            join(one, all, depth = 1),
            join_filterable("make", "year", "color"),
            join_sortable("year", "mileage")
        )]
        pub vehicles: Vec<super::vehicle::Vehicle>,
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

// ============================================================================
// UNIT TESTS FOR DOT-NOTATION PARSING
// ============================================================================

#[cfg(test)]
mod dot_notation_tests {
    use super::*;
    use crudcrate::FilterOperator;

    #[test]
    fn test_parse_simple_dot_notation() {
        let result = parse_dot_notation("vehicles.make");
        assert!(result.is_some());
        let (join_field, column, op) = result.unwrap();
        assert_eq!(join_field, "vehicles");
        assert_eq!(column, "make");
        assert_eq!(op, FilterOperator::Eq);
    }

    #[test]
    fn test_parse_dot_notation_with_gte_operator() {
        let result = parse_dot_notation("vehicles.year_gte");
        assert!(result.is_some());
        let (join_field, column, op) = result.unwrap();
        assert_eq!(join_field, "vehicles");
        assert_eq!(column, "year");
        assert_eq!(op, FilterOperator::Gte);
    }

    #[test]
    fn test_parse_dot_notation_with_lte_operator() {
        let result = parse_dot_notation("vehicles.mileage_lte");
        assert!(result.is_some());
        let (join_field, column, op) = result.unwrap();
        assert_eq!(join_field, "vehicles");
        assert_eq!(column, "mileage");
        assert_eq!(op, FilterOperator::Lte);
    }

    #[test]
    fn test_parse_dot_notation_with_neq_operator() {
        let result = parse_dot_notation("vehicles.color_neq");
        assert!(result.is_some());
        let (join_field, column, op) = result.unwrap();
        assert_eq!(join_field, "vehicles");
        assert_eq!(column, "color");
        assert_eq!(op, FilterOperator::Neq);
    }

    #[test]
    fn test_parse_no_dot_returns_none() {
        let result = parse_dot_notation("name");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_nested_dots() {
        // Should split on first dot only
        let result = parse_dot_notation("a.b.c");
        assert!(result.is_some());
        let (join_field, column, _) = result.unwrap();
        assert_eq!(join_field, "a");
        assert_eq!(column, "b.c");
    }
}

// ============================================================================
// TESTS FOR GENERATED TRAIT METHODS
// ============================================================================

#[cfg(test)]
mod trait_method_tests {
    use super::*;
    use customer::Customer;

    #[test]
    fn test_joined_filterable_columns_generated() {
        let columns = Customer::joined_filterable_columns();

        // Should have 3 filterable columns: make, year, color
        assert_eq!(columns.len(), 3);

        // Check each column is present
        let column_names: Vec<&str> = columns.iter().map(|c| c.full_path).collect();
        assert!(column_names.contains(&"vehicles.make"));
        assert!(column_names.contains(&"vehicles.year"));
        assert!(column_names.contains(&"vehicles.color"));
    }

    #[test]
    fn test_joined_sortable_columns_generated() {
        let columns = Customer::joined_sortable_columns();

        // Should have 2 sortable columns: year, mileage
        assert_eq!(columns.len(), 2);

        // Check each column is present
        let column_names: Vec<&str> = columns.iter().map(|c| c.full_path).collect();
        assert!(column_names.contains(&"vehicles.year"));
        assert!(column_names.contains(&"vehicles.mileage"));
    }

    #[test]
    fn test_joined_column_def_structure() {
        let columns = Customer::joined_filterable_columns();

        // Find the make column
        let make_col = columns.iter().find(|c| c.column_name == "make").unwrap();

        assert_eq!(make_col.join_field, "vehicles");
        assert_eq!(make_col.column_name, "make");
        assert_eq!(make_col.full_path, "vehicles.make");
    }
}

// ============================================================================
// TESTS FOR FILTER PARSING WITH JOINS
// ============================================================================

#[cfg(test)]
mod filter_parsing_tests {
    use super::*;
    use customer::Customer;
    use sea_orm::DatabaseBackend;

    #[test]
    fn test_filter_with_main_columns_only() {
        let filter_str = Some(r#"{"name":"John"}"#.to_string());
        let filterable_columns = Customer::filterable_columns();

        let parsed = apply_filters_with_joins::<Customer>(
            filter_str,
            &filterable_columns,
            DatabaseBackend::Sqlite,
        );

        // Should have no joined filters
        assert!(!parsed.has_joined_filters);
        assert!(parsed.joined_filters.is_empty());
    }

    #[test]
    fn test_filter_with_joined_columns() {
        let filter_str = Some(r#"{"vehicles.make":"BMW"}"#.to_string());
        let filterable_columns = Customer::filterable_columns();

        let parsed = apply_filters_with_joins::<Customer>(
            filter_str,
            &filterable_columns,
            DatabaseBackend::Sqlite,
        );

        // Should have joined filters
        assert!(parsed.has_joined_filters);
        assert_eq!(parsed.joined_filters.len(), 1);

        let filter = &parsed.joined_filters[0];
        assert_eq!(filter.join_field, "vehicles");
        assert_eq!(filter.column, "make");
    }

    #[test]
    fn test_filter_with_mixed_columns() {
        let filter_str = Some(r#"{"name":"John","vehicles.year_gte":2020}"#.to_string());
        let filterable_columns = Customer::filterable_columns();

        let parsed = apply_filters_with_joins::<Customer>(
            filter_str,
            &filterable_columns,
            DatabaseBackend::Sqlite,
        );

        // Should have joined filters
        assert!(parsed.has_joined_filters);
        assert_eq!(parsed.joined_filters.len(), 1);

        // Main condition should be set (not just default)
        // Can't easily test the condition content without DB, but it should exist
    }

    #[test]
    fn test_filter_with_invalid_joined_column_ignored() {
        // Try to filter on a column not in join_filterable
        let filter_str = Some(r#"{"vehicles.model":"Sedan"}"#.to_string());
        let filterable_columns = Customer::filterable_columns();

        let parsed = apply_filters_with_joins::<Customer>(
            filter_str,
            &filterable_columns,
            DatabaseBackend::Sqlite,
        );

        // Should NOT have joined filters (model is not in join_filterable)
        assert!(!parsed.has_joined_filters);
        assert!(parsed.joined_filters.is_empty());
    }

    #[test]
    fn test_filter_with_multiple_joined_filters() {
        let filter_str = Some(r#"{"vehicles.make":"BMW","vehicles.year_gte":2020,"vehicles.color":"Black"}"#.to_string());
        let filterable_columns = Customer::filterable_columns();

        let parsed = apply_filters_with_joins::<Customer>(
            filter_str,
            &filterable_columns,
            DatabaseBackend::Sqlite,
        );

        // Should have 3 joined filters
        assert!(parsed.has_joined_filters);
        assert_eq!(parsed.joined_filters.len(), 3);
    }
}

// ============================================================================
// TESTS FOR SORT PARSING WITH JOINS
// ============================================================================

#[cfg(test)]
mod sort_parsing_tests {
    use super::*;
    use customer::Customer;
    use crudcrate::FilterOptions;

    #[test]
    fn test_sort_by_main_column() {
        let params = FilterOptions {
            sort: Some(r#"["name", "DESC"]"#.to_string()),
            ..Default::default()
        };

        let sortable_columns = Customer::sortable_columns();
        let default_column = Customer::default_index_column();

        let sort_config = parse_sorting_with_joins::<Customer, _>(
            &params,
            &sortable_columns,
            default_column,
        );

        // Should be a Column sort
        match sort_config {
            SortConfig::Column { direction, .. } => {
                assert_eq!(direction, sea_orm::Order::Desc);
            }
            SortConfig::Joined { .. } => {
                panic!("Expected Column sort, got Joined");
            }
        }
    }

    #[test]
    fn test_sort_by_joined_column() {
        let params = FilterOptions {
            sort: Some(r#"["vehicles.year", "DESC"]"#.to_string()),
            ..Default::default()
        };

        let sortable_columns = Customer::sortable_columns();
        let default_column = Customer::default_index_column();

        let sort_config = parse_sorting_with_joins::<Customer, _>(
            &params,
            &sortable_columns,
            default_column,
        );

        // Should be a Joined sort
        match sort_config {
            SortConfig::Joined { join_field, column, direction } => {
                assert_eq!(join_field, "vehicles");
                assert_eq!(column, "year");
                assert_eq!(direction, sea_orm::Order::Desc);
            }
            SortConfig::Column { .. } => {
                panic!("Expected Joined sort, got Column");
            }
        }
    }

    #[test]
    fn test_sort_by_invalid_joined_column_falls_back() {
        // Try to sort by a column not in join_sortable (e.g., model)
        let params = FilterOptions {
            sort: Some(r#"["vehicles.model", "DESC"]"#.to_string()),
            ..Default::default()
        };

        let sortable_columns = Customer::sortable_columns();
        let default_column = Customer::default_index_column();

        let sort_config = parse_sorting_with_joins::<Customer, _>(
            &params,
            &sortable_columns,
            default_column,
        );

        // Should fall back to Column sort (invalid joined sort)
        match sort_config {
            SortConfig::Column { .. } => {
                // Expected - falls back to default
            }
            SortConfig::Joined { .. } => {
                panic!("Should have fallen back to Column sort");
            }
        }
    }

    #[test]
    fn test_sort_config_is_joined() {
        let params = FilterOptions {
            sort: Some(r#"["vehicles.mileage", "ASC"]"#.to_string()),
            ..Default::default()
        };

        let sortable_columns = Customer::sortable_columns();
        let default_column = Customer::default_index_column();

        let sort_config = parse_sorting_with_joins::<Customer, _>(
            &params,
            &sortable_columns,
            default_column,
        );

        assert!(sort_config.is_joined());
    }

    #[test]
    fn test_sort_config_direction() {
        let params = FilterOptions {
            sort: Some(r#"["vehicles.year", "ASC"]"#.to_string()),
            ..Default::default()
        };

        let sortable_columns = Customer::sortable_columns();
        let default_column = Customer::default_index_column();

        let sort_config = parse_sorting_with_joins::<Customer, _>(
            &params,
            &sortable_columns,
            default_column,
        );

        assert_eq!(sort_config.direction(), sea_orm::Order::Asc);
    }
}

// ============================================================================
// TESTS FOR SECURITY (WHITELIST VALIDATION)
// ============================================================================

#[cfg(test)]
mod security_tests {
    use super::*;
    use customer::Customer;
    use sea_orm::DatabaseBackend;

    #[test]
    fn test_sql_injection_in_dot_notation_rejected() {
        // Attempt SQL injection via dot-notation
        let filter_str = Some(r#"{"vehicles.make; DROP TABLE customers--":"test"}"#.to_string());
        let filterable_columns = Customer::filterable_columns();

        let parsed = apply_filters_with_joins::<Customer>(
            filter_str,
            &filterable_columns,
            DatabaseBackend::Sqlite,
        );

        // Should be rejected (not a valid joined column)
        assert!(!parsed.has_joined_filters);
    }

    #[test]
    fn test_invalid_join_field_rejected() {
        // Try to filter on a non-existent join field
        let filter_str = Some(r#"{"fake_relation.column":"value"}"#.to_string());
        let filterable_columns = Customer::filterable_columns();

        let parsed = apply_filters_with_joins::<Customer>(
            filter_str,
            &filterable_columns,
            DatabaseBackend::Sqlite,
        );

        // Should be rejected
        assert!(!parsed.has_joined_filters);
    }

    #[test]
    fn test_only_whitelisted_columns_accepted() {
        // model is NOT in join_filterable, so should be rejected
        let filter_str = Some(r#"{"vehicles.model":"Civic"}"#.to_string());
        let filterable_columns = Customer::filterable_columns();

        let parsed = apply_filters_with_joins::<Customer>(
            filter_str,
            &filterable_columns,
            DatabaseBackend::Sqlite,
        );

        assert!(!parsed.has_joined_filters);

        // But make IS in join_filterable, so should be accepted
        let filter_str = Some(r#"{"vehicles.make":"Honda"}"#.to_string());

        let parsed = apply_filters_with_joins::<Customer>(
            filter_str,
            &filterable_columns,
            DatabaseBackend::Sqlite,
        );

        assert!(parsed.has_joined_filters);
    }
}
