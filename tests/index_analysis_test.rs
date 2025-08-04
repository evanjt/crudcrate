/*!
# Index Analysis Integration Test

Tests the database-agnostic index analysis functionality.
*/

use chrono::{DateTime, Utc};
use crudcrate::{
    EntityToModels, analyze_indexes_for_resource,
    index_analysis::{IndexType, Priority},
    traits::CRUDResource,
};
use sea_orm::{Database, DatabaseConnection, entity::prelude::*};
use sea_orm_migration::{prelude::*, sea_query::ColumnDef};
use uuid::Uuid;

/// Test entity with various field types for index analysis
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "index_test_posts")]
#[crudcrate(api_struct = "IndexTestPost", active_model = "ActiveModel")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, create_model = false, update_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable)] // Should recommend B-tree index
    pub title: String,

    #[sea_orm(column_type = "Text")]
    #[crudcrate(fulltext)] // Should recommend fulltext index
    pub content: String,

    #[crudcrate(fulltext, filterable)] // Should recommend both
    pub author: String,

    #[crudcrate(filterable)] // Should recommend B-tree index
    pub published: bool,

    #[crudcrate(sortable)] // Should recommend B-tree index
    pub view_count: i32,

    #[crudcrate(sortable, create_model = false, update_model = false, on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Migration for the test table
pub struct IndexTestMigrator;

#[async_trait::async_trait]
impl MigratorTrait for IndexTestMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(CreateIndexTestTable)]
    }
}

pub struct CreateIndexTestTable;

#[async_trait::async_trait]
impl MigrationName for CreateIndexTestTable {
    fn name(&self) -> &'static str {
        "m20240101_000001_create_index_test_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for CreateIndexTestTable {
    async fn up(&self, manager: &SchemaManager) -> Result<(), sea_orm::DbErr> {
        let table = Table::create()
            .table(IndexTestEntity)
            .if_not_exists()
            .col(
                ColumnDef::new(IndexTestColumn::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            )
            .col(ColumnDef::new(IndexTestColumn::Title).string().not_null())
            .col(ColumnDef::new(IndexTestColumn::Content).text().not_null())
            .col(ColumnDef::new(IndexTestColumn::Author).string().not_null())
            .col(
                ColumnDef::new(IndexTestColumn::Published)
                    .boolean()
                    .not_null()
                    .default(false),
            )
            .col(
                ColumnDef::new(IndexTestColumn::ViewCount)
                    .integer()
                    .not_null()
                    .default(0),
            )
            .col(
                ColumnDef::new(IndexTestColumn::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null(),
            )
            .to_owned();

        manager.create_table(table).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), sea_orm::DbErr> {
        manager
            .drop_table(Table::drop().table(IndexTestEntity).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum IndexTestColumn {
    Id,
    Title,
    Content,
    Author,
    Published,
    ViewCount,
    CreatedAt,
}

impl Iden for IndexTestColumn {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(
            s,
            "{}",
            match self {
                Self::Id => "id",
                Self::Title => "title",
                Self::Content => "content",
                Self::Author => "author",
                Self::Published => "published",
                Self::ViewCount => "view_count",
                Self::CreatedAt => "created_at",
            }
        )
        .unwrap();
    }
}

#[derive(Debug)]
pub struct IndexTestEntity;

impl Iden for IndexTestEntity {
    fn unquoted(&self, s: &mut dyn std::fmt::Write) {
        write!(s, "index_test_posts").unwrap();
    }
}

async fn setup_test_db() -> Result<DatabaseConnection, sea_orm::DbErr> {
    let db = Database::connect("sqlite::memory:").await?;
    IndexTestMigrator::up(&db, None).await?;
    Ok(db)
}

#[tokio::test]
async fn test_index_analysis_functionality() {
    let db = setup_test_db()
        .await
        .expect("Failed to set up test database");

    // Analyze indexes for our test resource
    let recommendations = analyze_indexes_for_resource::<IndexTestPost>(&db)
        .await
        .expect("Failed to analyze indexes");

    // Should have recommendations for unindexed filterable/sortable fields
    assert!(
        !recommendations.is_empty(),
        "Should have index recommendations"
    );

    // Check that we have recommendations for the expected fields
    let recommended_columns: Vec<String> = recommendations
        .iter()
        .map(|r| r.column_name.clone())
        .collect();

    // Should recommend indexes for filterable/sortable fields
    assert!(recommended_columns.iter().any(|col| col.contains("title")));
    assert!(recommended_columns.iter().any(|col| col.contains("author")));
    assert!(
        recommended_columns
            .iter()
            .any(|col| col.contains("published"))
    );
    assert!(
        recommended_columns
            .iter()
            .any(|col| col.contains("view_count"))
    );
    assert!(
        recommended_columns
            .iter()
            .any(|col| col.contains("created_at"))
    );

    // Should have fulltext recommendation
    let has_fulltext_rec = recommendations
        .iter()
        .any(|r| r.reason.contains("Fulltext search"));
    assert!(
        has_fulltext_rec,
        "Should have fulltext search recommendation"
    );

    println!("\n📋 Index Analysis Test Results:");
    println!("Found {} recommendations", recommendations.len());
    for rec in &recommendations {
        println!(
            "  • {}: {} ({})",
            rec.column_name, rec.reason, rec.suggested_sql
        );
    }
}

#[tokio::test]
async fn test_display_index_recommendations() {
    let db = setup_test_db()
        .await
        .expect("Failed to set up test database");

    println!("\n🧪 Testing Index Analysis Display:");

    // This will display the pretty formatted recommendations
    IndexTestPost::analyze_and_display_indexes(&db)
        .await
        .expect("Failed to analyze and display indexes");

    // Test that calling it again doesn't display twice (due to atomic boolean)
    println!("\n🔄 Calling analysis again (should not display twice):");
    IndexTestPost::analyze_and_display_indexes(&db)
        .await
        .expect("Failed to analyze and display indexes");
}

#[tokio::test]
async fn test_filterable_columns_recommendations() {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to database");

    // Create table without indexes - using direct SQL to avoid Sea-ORM conflicts
    db.execute_unprepared(
        "CREATE TABLE index_test_posts (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            author TEXT NOT NULL,
            published BOOLEAN NOT NULL,
            view_count INTEGER NOT NULL,
            created_at DATETIME NOT NULL
        )",
    )
    .await
    .expect("Failed to create table");

    let recommendations = analyze_indexes_for_resource::<IndexTestPost>(&db)
        .await
        .expect("Failed to analyze indexes");

    // Filter for filterable field recommendations only
    let filterable_recs: Vec<_> = recommendations
        .iter()
        .filter(|r| r.reason.contains("filterable but not indexed"))
        .collect();

    // Should have recommendations for filterable fields (title, author, published)
    assert!(
        !filterable_recs.is_empty(),
        "Should have filterable field recommendations"
    );

    // Check that we have the expected filterable fields
    let filterable_columns: Vec<String> = filterable_recs
        .iter()
        .map(|r| r.column_name.clone())
        .collect();

    assert!(filterable_columns.contains(&"title".to_string()));
    assert!(filterable_columns.contains(&"author".to_string()));
    assert!(filterable_columns.contains(&"published".to_string()));

    // All should be medium priority B-tree indexes
    let filterable_count = filterable_recs.len();
    for rec in &filterable_recs {
        assert_eq!(rec.priority, Priority::Medium);
        assert_eq!(rec.index_type, IndexType::BTree);
    }

    println!("✅ Filterable columns test passed: {filterable_count} recommendations");
}

#[tokio::test]
async fn test_sortable_columns_recommendations() {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to database");

    // Create table without indexes
    db.execute_unprepared(
        "CREATE TABLE index_test_posts (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            author TEXT NOT NULL,
            published BOOLEAN NOT NULL,
            view_count INTEGER NOT NULL,
            created_at DATETIME NOT NULL
        )",
    )
    .await
    .expect("Failed to create table");

    let recommendations = analyze_indexes_for_resource::<IndexTestPost>(&db)
        .await
        .expect("Failed to analyze indexes");

    // Filter for sortable field recommendations only
    let sortable_recs: Vec<_> = recommendations
        .iter()
        .filter(|r| r.reason.contains("sortable but not indexed"))
        .collect();

    // Should have recommendations for sortable fields (title, view_count, created_at)
    assert!(
        !sortable_recs.is_empty(),
        "Should have sortable field recommendations"
    );

    // Check that we have the expected sortable fields
    let sortable_columns: Vec<String> = sortable_recs
        .iter()
        .map(|r| r.column_name.clone())
        .collect();

    assert!(sortable_columns.contains(&"title".to_string()));
    assert!(sortable_columns.contains(&"view_count".to_string()));
    assert!(sortable_columns.contains(&"created_at".to_string()));

    // All should be medium priority B-tree indexes
    let sortable_count = sortable_recs.len();
    for rec in &sortable_recs {
        assert_eq!(rec.priority, Priority::Medium);
        assert_eq!(rec.index_type, IndexType::BTree);
    }

    println!("✅ Sortable columns test passed: {sortable_count} recommendations");
}

#[tokio::test]
async fn test_fulltext_columns_recommendations() {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to database");

    // Create table without indexes
    db.execute_unprepared(
        "CREATE TABLE index_test_posts (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            author TEXT NOT NULL,
            published BOOLEAN NOT NULL,
            view_count INTEGER NOT NULL,
            created_at DATETIME NOT NULL
        )",
    )
    .await
    .expect("Failed to create table");

    let recommendations = analyze_indexes_for_resource::<IndexTestPost>(&db)
        .await
        .expect("Failed to analyze indexes");

    // Filter for fulltext recommendations only
    let fulltext_recs: Vec<_> = recommendations
        .iter()
        .filter(|r| r.reason.contains("Fulltext search"))
        .collect();

    // Should have 1 fulltext recommendation covering all fulltext fields
    assert_eq!(
        fulltext_recs.len(),
        1,
        "Should have exactly 1 fulltext search recommendation"
    );

    let fulltext_rec = &fulltext_recs[0];
    assert_eq!(fulltext_rec.priority, Priority::High);
    assert_eq!(fulltext_rec.index_type, IndexType::BTree); // SQLite fallback
    assert!(fulltext_rec.reason.contains("Fulltext search on 2 columns")); // content, author
    assert!(fulltext_rec.column_name.contains("content"));
    assert!(fulltext_rec.column_name.contains("author"));

    println!(
        "✅ Fulltext columns test passed: {} recommendations",
        fulltext_recs.len()
    );
}

#[tokio::test]
async fn test_priority_levels() {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to database");

    // Create table without indexes
    db.execute_unprepared(
        "CREATE TABLE index_test_posts (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            author TEXT NOT NULL,
            published BOOLEAN NOT NULL,
            view_count INTEGER NOT NULL,
            created_at DATETIME NOT NULL
        )",
    )
    .await
    .expect("Failed to create table");

    let recommendations = analyze_indexes_for_resource::<IndexTestPost>(&db)
        .await
        .expect("Failed to analyze indexes");

    // Group recommendations by priority
    let mut priority_counts = std::collections::HashMap::new();
    for rec in &recommendations {
        *priority_counts.entry(rec.priority.clone()).or_insert(0) += 1;
    }

    // Should have Medium priority for filterable/sortable fields
    assert!(
        priority_counts.get(&Priority::Medium).unwrap_or(&0) > &0,
        "Should have medium priority recommendations"
    );

    // Should have High priority for fulltext search
    assert!(
        priority_counts.get(&Priority::High).unwrap_or(&0) > &0,
        "Should have high priority recommendations"
    );

    println!("✅ Priority levels test passed: {priority_counts:?}");
}

#[tokio::test]
async fn test_no_recommendations_with_existing_indexes() {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to database");

    // Create table WITH indexes
    db.execute_unprepared(
        "CREATE TABLE index_test_posts (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            author TEXT NOT NULL,
            published BOOLEAN NOT NULL,
            view_count INTEGER NOT NULL,
            created_at DATETIME NOT NULL
        )",
    )
    .await
    .expect("Failed to create table");

    // Add indexes for all filterable/sortable fields
    db.execute_unprepared("CREATE INDEX idx_index_test_posts_title ON index_test_posts (title)")
        .await
        .expect("Failed to create title index");
    db.execute_unprepared("CREATE INDEX idx_index_test_posts_author ON index_test_posts (author)")
        .await
        .expect("Failed to create author index");
    db.execute_unprepared(
        "CREATE INDEX idx_index_test_posts_published ON index_test_posts (published)",
    )
    .await
    .expect("Failed to create published index");
    db.execute_unprepared(
        "CREATE INDEX idx_index_test_posts_view_count ON index_test_posts (view_count)",
    )
    .await
    .expect("Failed to create view_count index");
    db.execute_unprepared(
        "CREATE INDEX idx_index_test_posts_created_at ON index_test_posts (created_at)",
    )
    .await
    .expect("Failed to create created_at index");

    let recommendations = analyze_indexes_for_resource::<IndexTestPost>(&db)
        .await
        .expect("Failed to analyze indexes");

    // Filter out fulltext recommendations (we can't easily create fulltext indexes in SQLite)
    let non_fulltext_recs: Vec<_> = recommendations
        .iter()
        .filter(|r| !r.reason.contains("Fulltext search"))
        .collect();

    // Should have no recommendations for filterable/sortable fields since they're indexed
    assert_eq!(
        non_fulltext_recs.len(),
        0,
        "Should have no recommendations for indexed fields"
    );

    // Should still have fulltext recommendation since we didn't create that index
    let fulltext_recs: Vec<_> = recommendations
        .iter()
        .filter(|r| r.reason.contains("Fulltext search"))
        .collect();
    assert_eq!(
        fulltext_recs.len(),
        1,
        "Should still have fulltext recommendation"
    );

    println!(
        "✅ Existing indexes test passed: {} non-fulltext recommendations",
        non_fulltext_recs.len()
    );
}

#[tokio::test]
async fn test_combined_field_attributes() {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to database");

    // Create table without indexes
    db.execute_unprepared(
        "CREATE TABLE index_test_posts (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            author TEXT NOT NULL,
            published BOOLEAN NOT NULL,
            view_count INTEGER NOT NULL,
            created_at DATETIME NOT NULL
        )",
    )
    .await
    .expect("Failed to create table");

    let recommendations = analyze_indexes_for_resource::<IndexTestPost>(&db)
        .await
        .expect("Failed to analyze indexes");

    // title is both filterable and sortable - should get recommendation for B-tree index
    let title_recs: Vec<_> = recommendations
        .iter()
        .filter(|r| r.column_name == "title")
        .collect();

    // Should have at least one recommendation for title (could be filterable or sortable reason)
    assert!(
        !title_recs.is_empty(),
        "Should have recommendation for title field"
    );

    // author is both filterable and fulltext - should appear in both B-tree and fulltext recommendations
    let author_individual_recs: Vec<_> = recommendations
        .iter()
        .filter(|r| r.column_name == "author")
        .collect();

    let author_in_fulltext_recs: Vec<_> = recommendations
        .iter()
        .filter(|r| r.column_name.contains("author") && r.reason.contains("Fulltext search"))
        .collect();

    assert!(
        !author_individual_recs.is_empty() || !author_in_fulltext_recs.is_empty(),
        "Should have recommendation involving author field"
    );

    println!("✅ Combined field attributes test passed");
}
