/*!
# Index Analysis Integration Test

Tests the database-agnostic index analysis functionality.
*/

use chrono::{DateTime, Utc};
use crudcrate::{analyze_indexes_for_resource, display_index_recommendations, traits::CRUDResource};
use crudcrate_derive::EntityToModels;
use sea_orm::{entity::prelude::*, Database, DatabaseConnection};
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
    
    #[crudcrate(filterable, sortable)]  // Should recommend B-tree index
    pub title: String,
    
    #[sea_orm(column_type = "Text")]
    #[crudcrate(fulltext)]  // Should recommend fulltext index
    pub content: String,
    
    #[crudcrate(fulltext, filterable)]  // Should recommend both
    pub author: String,
    
    #[crudcrate(filterable)]  // Should recommend B-tree index
    pub published: bool,
    
    #[crudcrate(sortable)]  // Should recommend B-tree index
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
            .col(ColumnDef::new(IndexTestColumn::Id).uuid().not_null().primary_key())
            .col(ColumnDef::new(IndexTestColumn::Title).string().not_null())
            .col(ColumnDef::new(IndexTestColumn::Content).text().not_null())
            .col(ColumnDef::new(IndexTestColumn::Author).string().not_null())
            .col(ColumnDef::new(IndexTestColumn::Published).boolean().not_null().default(false))
            .col(ColumnDef::new(IndexTestColumn::ViewCount).integer().not_null().default(0))
            .col(ColumnDef::new(IndexTestColumn::CreatedAt).timestamp_with_time_zone().not_null())
            .to_owned();

        manager.create_table(table).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), sea_orm::DbErr> {
        manager.drop_table(Table::drop().table(IndexTestEntity).to_owned()).await?;
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
        ).unwrap();
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
    let db = setup_test_db().await.expect("Failed to set up test database");
    
    // Analyze indexes for our test resource
    let recommendations = analyze_indexes_for_resource::<IndexTestPost>(&db)
        .await
        .expect("Failed to analyze indexes");
    
    // Should have recommendations for unindexed filterable/sortable fields
    assert!(!recommendations.is_empty(), "Should have index recommendations");
    
    // Check that we have recommendations for the expected fields
    let recommended_columns: Vec<String> = recommendations
        .iter()
        .map(|r| r.column_name.clone())
        .collect();
    
    // Should recommend indexes for filterable/sortable fields
    assert!(recommended_columns.iter().any(|col| col.contains("title")));
    assert!(recommended_columns.iter().any(|col| col.contains("author")));
    assert!(recommended_columns.iter().any(|col| col.contains("published")));
    assert!(recommended_columns.iter().any(|col| col.contains("view_count")));
    assert!(recommended_columns.iter().any(|col| col.contains("created_at")));
    
    // Should have fulltext recommendation
    let has_fulltext_rec = recommendations
        .iter()
        .any(|r| r.reason.contains("Fulltext search"));
    assert!(has_fulltext_rec, "Should have fulltext search recommendation");
    
    println!("\n📋 Index Analysis Test Results:");
    println!("Found {} recommendations", recommendations.len());
    for rec in &recommendations {
        println!("  • {}: {} ({})", rec.column_name, rec.reason, rec.suggested_sql);
    }
}

#[tokio::test]
async fn test_display_index_recommendations() {
    let db = setup_test_db().await.expect("Failed to set up test database");
    
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