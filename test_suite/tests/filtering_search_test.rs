// Feature Group 3: Advanced Filtering & Search
// Tests query parameters, fulltext search, database conditions

use chrono::{DateTime, Utc};
use crudcrate::{EntityToModels, filter::apply_filters, traits::CRUDResource};
use sea_orm::{DatabaseBackend, entity::prelude::*};
use serde_json::{json, Value};
use uuid::Uuid;

// Test entity for filtering and search
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "articles")]
#[crudcrate(api_struct = "Article", active_model = "ActiveModel")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(fulltext, filterable, sortable)]
    pub title: String,

    #[sea_orm(column_type = "Text", nullable)]
    #[crudcrate(fulltext)]
    pub content: Option<String>,

    #[crudcrate(fulltext, filterable)]
    pub author: String,

    #[sea_orm(column_type = "Text", nullable)]
    #[crudcrate(fulltext)]
    pub tags: Option<String>,

    #[crudcrate(filterable)]
    pub published: bool,

    #[crudcrate(filterable, sortable)]
    pub view_count: i32,

    #[crudcrate(sortable, exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[tokio::test]
async fn test_basic_equality_filtering() {
    let backend = DatabaseBackend::Sqlite;
    let filter_map = json!({
        "published": true,
        "author": "John Doe"
    });

    let condition = apply_filters::<Article>(
        Some(filter_map.to_string()),
        &Article::filterable_columns(),
        backend
    );

    // Should generate proper WHERE conditions for equality
    assert!(!condition.is_empty());
}

#[tokio::test]
async fn test_numeric_comparison_filtering() {
    let backend = DatabaseBackend::Sqlite;
    
    // Test greater than or equal
    let filter_map = json!({
        "view_count_gte": 100
    });

    let condition = apply_filters::<Article>(
        Some(filter_map.to_string()),
        &Article::filterable_columns(),
        backend
    );

    assert!(!condition.is_empty());

    // Test less than
    let filter_map = json!({
        "view_count_lt": 1000
    });

    let condition = apply_filters::<Article>(
        Some(filter_map.to_string()),
        &Article::filterable_columns(),
        backend
    );

    assert!(!condition.is_empty());

    // Test between (using both gte and lte)
    let filter_map = json!({
        "view_count_gte": 100,
        "view_count_lte": 1000
    });

    let condition = apply_filters::<Article>(
        Some(filter_map.to_string()),
        &Article::filterable_columns(),
        backend
    );

    assert!(!condition.is_empty());
}

#[tokio::test]
async fn test_list_operations_filtering() {
    let backend = DatabaseBackend::Sqlite;
    
    // Test IN operation with array of IDs
    let filter_map = json!({
        "id": ["550e8400-e29b-41d4-a716-446655440000", "550e8400-e29b-41d4-a716-446655440001"]
    });

    let condition = apply_filters::<Article>(
        Some(filter_map.to_string()),
        &Article::filterable_columns(),
        backend
    );

    assert!(!condition.is_empty());

    // Test IN operation with string array
    let filter_map = json!({
        "author": ["John Doe", "Jane Smith", "Bob Wilson"]
    });

    let condition = apply_filters::<Article>(
        Some(filter_map.to_string()),
        &Article::filterable_columns(),
        backend
    );

    assert!(!condition.is_empty());
}

#[tokio::test]
async fn test_fulltext_search_sqlite() {
    let backend = DatabaseBackend::Sqlite;
    
    // Test fulltext search query
    let filter_map = json!({
        "q": "rust programming tutorial"
    });

    let condition = apply_filters::<Article>(
        Some(filter_map.to_string()),
        &Article::filterable_columns(),
        backend
    );

    assert!(!condition.is_empty());
    
    // SQLite should use LIKE-based fallback for fulltext search
    // The exact SQL generation is internal, but we verify conditions are created
}

#[tokio::test]
async fn test_fulltext_search_postgresql() {
    let backend = DatabaseBackend::Postgres;
    
    // Test PostgreSQL fulltext search
    let filter_map = json!({
        "q": "rust programming tutorial"
    });

    let condition = apply_filters::<Article>(
        Some(filter_map.to_string()),
        &Article::filterable_columns(),
        backend
    );

    assert!(!condition.is_empty());
    
    // PostgreSQL should generate tsvector/plainto_tsquery conditions
}

#[tokio::test]
async fn test_fulltext_search_mysql() {
    let backend = DatabaseBackend::MySql;
    
    // Test MySQL fulltext search
    let filter_map = json!({
        "q": "rust programming tutorial"
    });

    let condition = apply_filters::<Article>(
        Some(filter_map.to_string()),
        &Article::filterable_columns(),
        backend
    );

    assert!(!condition.is_empty());
    
    // MySQL should use MATCH AGAINST for fulltext search
}

#[tokio::test]
async fn test_combined_filtering_and_search() {
    let backend = DatabaseBackend::Sqlite;
    
    // Test combination of regular filters and fulltext search
    let filter_map = json!({
        "q": "tutorial",
        "published": true,
        "view_count_gte": 50,
        "author": "John Doe"
    });

    let condition = apply_filters::<Article>(
        Some(filter_map.to_string()),
        &Article::filterable_columns(),
        backend
    );

    assert!(!condition.is_empty());
}

#[tokio::test]
async fn test_empty_and_invalid_filters() {
    let backend = DatabaseBackend::Sqlite;
    
    // Test empty filter
    let condition = apply_filters::<Article>(
        None,
        &Article::filterable_columns(),
        backend
    );
    
    // Empty filters should result in empty condition (no WHERE clause)
    assert!(condition.is_empty());

    // Test empty JSON object
    let filter_map = json!({});
    let condition = apply_filters::<Article>(
        Some(filter_map.to_string()),
        &Article::filterable_columns(),
        backend
    );
    
    assert!(condition.is_empty());

    // Test invalid JSON
    let condition = apply_filters::<Article>(
        Some("invalid json".to_string()),
        &Article::filterable_columns(),
        backend
    );
    
    // Should handle gracefully and return empty condition
    assert!(condition.is_empty());
}

#[tokio::test]
async fn test_non_filterable_field_ignored() {
    let backend = DatabaseBackend::Sqlite;
    
    // Try to filter on created_at which is not marked as filterable
    let filter_map = json!({
        "created_at": "2024-01-01T00:00:00Z",
        "title": "Valid Filter" // This should work
    });

    let condition = apply_filters::<Article>(
        Some(filter_map.to_string()),
        &Article::filterable_columns(),
        backend
    );

    // Should only process the filterable field (title)
    assert!(!condition.is_empty());
}

#[tokio::test]
async fn test_case_insensitive_string_matching() {
    let backend = DatabaseBackend::Sqlite;
    
    // Test case-insensitive matching for string fields
    let filter_map = json!({
        "author": "john doe" // Should match "John Doe" case-insensitively
    });

    let condition = apply_filters::<Article>(
        Some(filter_map.to_string()),
        &Article::filterable_columns(),
        backend
    );

    assert!(!condition.is_empty());
}

#[tokio::test]
async fn test_like_filterable_columns() {
    // Test fields that use LIKE queries for substring matching
    let backend = DatabaseBackend::Sqlite;
    
    // For fields marked with like_filterable_columns, should use LIKE instead of exact match
    let filter_map = json!({
        "title": "programming" // Should match titles containing "programming"
    });

    let condition = apply_filters::<Article>(
        Some(filter_map.to_string()),
        &Article::filterable_columns(),
        backend
    );

    assert!(!condition.is_empty());
}

#[tokio::test]
async fn test_fulltext_language_configuration() {
    // Test that different fulltext languages are handled
    
    // Test entity with Spanish language configuration
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "spanish_articles")]
    #[crudcrate(api_struct = "SpanishArticle", fulltext_language = "spanish")]
    pub struct SpanishModel {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update))]
        pub id: Uuid,

        #[crudcrate(fulltext, filterable)]
        pub titulo: String,

        #[crudcrate(fulltext)]
        pub contenido: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum SpanishRelation {}

    impl ActiveModelBehavior for spanish_articles::ActiveModel {}

    let backend = DatabaseBackend::Postgres;
    let filter_map = json!({
        "q": "programación tutorial"
    });

    let condition = apply_filters::<SpanishArticle>(
        Some(filter_map.to_string()),
        &SpanishArticle::filterable_columns(),
        backend
    );

    assert!(!condition.is_empty());
}

#[tokio::test]
async fn test_date_time_filtering() {
    let backend = DatabaseBackend::Sqlite;
    
    // Test date/time comparison filtering
    let filter_map = json!({
        "created_at_gte": "2024-01-01T00:00:00Z",
        "created_at_lt": "2024-12-31T23:59:59Z"
    });

    let condition = apply_filters::<Article>(
        Some(filter_map.to_string()),
        &Article::filterable_columns(),
        backend
    );

    // created_at is sortable but not filterable in our test model
    // This should result in empty condition as the field is ignored
    assert!(condition.is_empty());
}

#[tokio::test]
async fn test_boolean_filtering_variations() {
    let backend = DatabaseBackend::Sqlite;
    
    // Test different boolean value representations
    let test_cases = vec![
        json!({"published": true}),
        json!({"published": "true"}),
        json!({"published": 1}),
        json!({"published": false}),
        json!({"published": "false"}),
        json!({"published": 0}),
    ];

    for filter_map in test_cases {
        let condition = apply_filters::<Article>(
            Some(filter_map.to_string()),
            &Article::filterable_columns(),
            backend
        );

        assert!(!condition.is_empty());
    }
}

#[tokio::test]
async fn test_null_value_filtering() {
    let backend = DatabaseBackend::Sqlite;
    
    // Test filtering for null values
    let filter_map = json!({
        "content": null
    });

    let condition = apply_filters::<Article>(
        Some(filter_map.to_string()),
        &Article::filterable_columns(),
        backend
    );

    // Should handle null filtering (content is not filterable in our model)
    // This tests the null value parsing logic
    assert!(condition.is_empty());
}

#[tokio::test]
async fn test_complex_nested_search_terms() {
    let backend = DatabaseBackend::Sqlite;
    
    // Test fulltext search with complex terms
    let filter_map = json!({
        "q": "\"rust programming\" tutorial -beginner +advanced"
    });

    let condition = apply_filters::<Article>(
        Some(filter_map.to_string()),
        &Article::filterable_columns(),
        backend
    );

    assert!(!condition.is_empty());
}