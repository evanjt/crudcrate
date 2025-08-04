use chrono::{DateTime, Utc};
use crudcrate::{EntityToModels, filter::apply_filters, traits::CRUDResource};
use sea_orm::{DatabaseBackend, entity::prelude::*};
use uuid::Uuid;

// Use EntityToModels to generate fulltext search capabilities
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "articles")]
#[crudcrate(api_struct = "Article", active_model = "ActiveModel")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, create_model = false, update_model = false, on_create = Uuid::new_v4())]
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

    #[crudcrate(fulltext)]
    pub view_count: i32,

    #[crudcrate(sortable, create_model = false, update_model = false, on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[tokio::test]
async fn test_fulltext_search_postgres_simulation() {
    // Test PostgreSQL fulltext search functionality
    let backend = DatabaseBackend::Postgres;

    // Create a test query
    let filter_json = r#"{"q": "rust programming"}"#;

    let condition = apply_filters::<Article>(
        Some(filter_json.to_string()),
        &Article::filterable_columns(),
        backend,
    );

    // The condition should be built (we can't easily test the exact SQL without a real DB)
    // But we can verify that the function doesn't panic and returns a valid condition
    assert!(!condition.is_empty());
}

#[tokio::test]
async fn test_fulltext_search_sqlite_fallback() {
    // Test SQLite fallback fulltext search functionality
    let backend = DatabaseBackend::Sqlite;

    // Create a test query
    let filter_json = r#"{"q": "rust programming"}"#;

    let condition = apply_filters::<Article>(
        Some(filter_json.to_string()),
        &Article::filterable_columns(),
        backend,
    );

    // The condition should be built with fallback LIKE query
    assert!(!condition.is_empty());
}

#[tokio::test]
async fn test_fulltext_searchable_columns_generation() {
    // Test that the EntityToModels macro correctly generated fulltext_searchable_columns
    let fulltext_columns = Article::fulltext_searchable_columns();

    // Should have title, content, author, tags, and view_count
    assert_eq!(fulltext_columns.len(), 5);

    let column_names: Vec<&str> = fulltext_columns.iter().map(|(name, _)| *name).collect();
    assert!(column_names.contains(&"title"));
    assert!(column_names.contains(&"content"));
    assert!(column_names.contains(&"author"));
    assert!(column_names.contains(&"tags"));
    assert!(column_names.contains(&"view_count"));
}

#[tokio::test]
async fn test_fulltext_search_with_different_data_types() {
    // Test that fulltext search works with string, optional string, and numeric types
    let backend = DatabaseBackend::Sqlite;

    // Test with various search terms
    let search_terms = vec![
        "programming",
        "rust lang",
        "web development",
        "tutorial guide",
        "123", // Should work with numeric content too
    ];

    for term in search_terms {
        let filter_json = format!(r#"{{"q": "{}"}}"#, term);
        let condition =
            apply_filters::<Article>(Some(filter_json), &Article::filterable_columns(), backend);

        assert!(
            !condition.is_empty(),
            "Condition should not be empty for term: {}",
            term
        );
    }
}

#[tokio::test]
async fn test_empty_fulltext_query() {
    let backend = DatabaseBackend::Sqlite;

    // Test with empty search query
    let filter_json = r#"{"q": ""}"#;
    let condition = apply_filters::<Article>(
        Some(filter_json.to_string()),
        &Article::filterable_columns(),
        backend,
    );

    // Should handle empty queries gracefully
    assert!(!condition.is_empty());
}

#[tokio::test]
async fn test_no_fulltext_columns_fallback() {
    // Create a test entity with NO fulltext columns to test the fallback behavior
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "simple_items")]
    #[crudcrate(api_struct = "SimpleItem", active_model = "ActiveModel")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, create_model = false, update_model = false, on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable)] // Only filterable, NOT fulltext
        pub name: String,

        #[crudcrate(filterable)]
        pub description: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}

    let backend = DatabaseBackend::Sqlite;
    let filter_json = r#"{"q": "test search"}"#;

    // Test with entity that has NO fulltext columns - should fallback to regular filterable columns
    let condition = apply_filters::<SimpleItem>(
        Some(filter_json.to_string()),
        &SimpleItem::filterable_columns(),
        backend,
    );

    // Should create a valid condition using filterable columns as fallback
    assert!(!condition.is_empty());

    // Verify that this entity indeed has no fulltext columns
    assert_eq!(
        SimpleItem::fulltext_searchable_columns().len(),
        0,
        "SimpleItem should have no fulltext columns to test fallback behavior"
    );
}
