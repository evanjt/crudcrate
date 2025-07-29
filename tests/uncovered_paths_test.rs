/// Tests specifically targeting uncovered code paths to reach 80%+ coverage
use crudcrate::sort::generic_sort;
use crudcrate::models::FilterOptions;
use sea_orm::sea_query::Order;

// Use the existing todo entity from common module for testing
mod common;
use common::todo_entity::{Column as TodoColumn};

/// Test the generic_sort function directly to improve sort.rs coverage
#[test]
fn test_generic_sort_with_valid_json() {
    let sortable_columns = [
        ("title", TodoColumn::Title),
        ("completed", TodoColumn::Completed),
    ];
    
    // Test valid JSON sort array
    let (_column, order) = generic_sort(
        Some(r#"["title", "DESC"]"#.to_string()),
        &sortable_columns,
        TodoColumn::Id,
    );
    
    // We can't compare columns due to PartialEq not being implemented
    // but we can test the order 
    assert_eq!(order, Order::Desc);
}

#[test]
fn test_generic_sort_with_invalid_json() {
    let sortable_columns = [
        ("title", TodoColumn::Title),
        ("completed", TodoColumn::Completed),
    ];
    
    // Test invalid JSON - should fallback to defaults
    let (_column, order) = generic_sort(
        Some("invalid json".to_string()),
        &sortable_columns,
        TodoColumn::Id,
    );
    
    // Test that it doesn't crash and returns valid order
    assert_eq!(order, Order::Asc); // default order
}

#[test]
fn test_generic_sort_with_empty_array() {
    let sortable_columns = [
        ("title", TodoColumn::Title),
        ("completed", TodoColumn::Completed),
    ];
    
    // Test empty JSON array - should fallback to defaults
    let (_column, order) = generic_sort(
        Some("[]".to_string()),
        &sortable_columns,
        TodoColumn::Id,
    );
    
    // Test that it doesn't crash and returns valid order
    assert_eq!(order, Order::Asc); // default order
}

#[test]
fn test_generic_sort_with_single_element_array() {
    let sortable_columns = [
        ("title", TodoColumn::Title),
        ("completed", TodoColumn::Completed),
    ];
    
    // Test single element array - should use default order
    let (_column, order) = generic_sort(
        Some(r#"["title"]"#.to_string()),
        &sortable_columns,
        TodoColumn::Id,
    );
    
    // Test that it doesn't crash and returns valid order
    assert_eq!(order, Order::Asc); // default order when missing
}

#[test]
fn test_generic_sort_with_unknown_column() {
    let sortable_columns = [
        ("title", TodoColumn::Title),
        ("completed", TodoColumn::Completed),
    ];
    
    // Test unknown column - should fallback to default
    let (_column, order) = generic_sort(
        Some(r#"["unknown_column", "DESC"]"#.to_string()),
        &sortable_columns,
        TodoColumn::Id,
    );
    
    // Test that it doesn't crash and keeps the order
    assert_eq!(order, Order::Desc); // but keep the order
}

#[test]
fn test_generic_sort_with_none_sort() {
    let sortable_columns = [
        ("title", TodoColumn::Title),
        ("completed", TodoColumn::Completed),
    ];
    
    // Test None - should use defaults
    let (_column, order) = generic_sort(
        None,
        &sortable_columns,
        TodoColumn::Id,
    );
    
    // Test that it doesn't crash and returns valid order
    assert_eq!(order, Order::Asc); // default order
}

#[test]
fn test_generic_sort_case_sensitivity() {
    let sortable_columns = [
        ("title", TodoColumn::Title),
        ("completed", TodoColumn::Completed),
    ];
    
    // Test ASC vs asc (case sensitivity)
    let (_column, order) = generic_sort(
        Some(r#"["title", "asc"]"#.to_string()),
        &sortable_columns,
        TodoColumn::Id,
    );
    
    // generic_sort should be case sensitive for ASC, so "asc" != "ASC" 
    // Should fallback to DESC for non-"ASC" values
    assert_eq!(order, Order::Desc);
}

/// Test OpenAPI schema generation to improve models.rs coverage
#[test] 
fn test_filter_options_schema_generation() {
    use utoipa::{ToSchema, OpenApi};
    
    // This exercises the ToSchema derive by using it in an OpenAPI context
    #[derive(OpenApi)]
    #[openapi(components(schemas(FilterOptions)))]
    struct ApiDoc;
    
    // Generate the OpenAPI spec which exercises ToSchema
    let spec = ApiDoc::openapi();
    
    // Verify FilterOptions is in the schema components
    assert!(spec.components.is_some());
    let components = spec.components.unwrap();
    assert!(components.schemas.contains_key("FilterOptions"));
    
    // Test that the struct can be instantiated with schema-related traits
    let _default_options = FilterOptions::default();
    
    // Test that it has the expected fields by creating a full struct
    let filter_options = FilterOptions {
        filter: Some(r#"{"title": "test"}"#.to_string()),
        range: Some("[0,9]".to_string()),
        page: Some(1),
        per_page: Some(10),
        sort: Some(r#"["title", "ASC"]"#.to_string()),
        sort_by: Some("title".to_string()),
        order: Some("ASC".to_string()),
    };
    
    // Verify it can be deserialized (which exercises various trait implementations)
    let json_str = r#"{
        "filter": "{\"title\": \"test\"}",
        "range": "[0,9]",
        "page": 1,
        "per_page": 10,
        "sort": "[\"title\", \"ASC\"]",
        "sort_by": "title",
        "order": "ASC"
    }"#;
    
    let deserialized: FilterOptions = serde_json::from_str(json_str).expect("Failed to deserialize");
    assert_eq!(deserialized.filter, filter_options.filter);
    assert_eq!(deserialized.range, filter_options.range);
    assert_eq!(deserialized.page, filter_options.page);
    assert_eq!(deserialized.per_page, filter_options.per_page);
    assert_eq!(deserialized.sort, filter_options.sort);
    assert_eq!(deserialized.sort_by, filter_options.sort_by);
    assert_eq!(deserialized.order, filter_options.order);
}

/// Test schema generation with empty/default values
#[test]
fn test_filter_options_schema_defaults() {
    // Test default constructor (exercises Default derive)
    let default_options = FilterOptions::default();
    
    assert_eq!(default_options.filter, None);
    assert_eq!(default_options.range, None);
    assert_eq!(default_options.page, None);
    assert_eq!(default_options.per_page, None);
    assert_eq!(default_options.sort, None);
    assert_eq!(default_options.sort_by, None);
    assert_eq!(default_options.order, None);
    
    // Test that schema can handle empty structs
    let empty_json = "{}";
    let empty_options: FilterOptions = serde_json::from_str(empty_json).expect("Failed to deserialize empty JSON");
    
    // Should be equivalent to default
    assert_eq!(empty_options.filter, default_options.filter);
    assert_eq!(empty_options.range, default_options.range);
}

/// Test OpenAPI parameter generation (IntoParams derive)
#[test]
fn test_filter_options_into_params() {
    use utoipa::IntoParams;
    
    // This exercises the IntoParams derive that generates OpenAPI parameter schemas
    let params = FilterOptions::into_params(|| None);
    
    // Verify we have the expected number of parameters (7 fields)
    assert_eq!(params.len(), 7);
    
    // Verify parameter names match struct fields
    let param_names: Vec<&str> = params.iter().map(|p| p.name.as_str()).collect();
    assert!(param_names.contains(&"filter"));
    assert!(param_names.contains(&"range"));
    assert!(param_names.contains(&"page"));
    assert!(param_names.contains(&"per_page")); 
    assert!(param_names.contains(&"sort"));
    assert!(param_names.contains(&"sort_by"));
    assert!(param_names.contains(&"order"));
}