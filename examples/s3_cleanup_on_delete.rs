//! S3 Cleanup on Delete Pattern
//!
//! Demonstrates using `fn_delete` and `fn_delete_many` to clean up external
//! resources (S3 objects) before deleting database records.
//!
//! Based on production pattern from drop4crop-api and spice-api.

use chrono::{DateTime, Utc};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::{Database, DatabaseConnection, entity::prelude::*, EntityTrait};
use uuid::Uuid;

// ============================================================================
// Asset Entity with S3 Storage
// ============================================================================

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "assets")]
#[crudcrate(
    api_struct = "Asset",
    name_singular = "asset",
    name_plural = "assets",
    description = "File assets stored in S3",
    generate_router,
    fn_delete = delete_asset,
    fn_delete_many = delete_many_assets,
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable)]
    pub filename: String,

    #[crudcrate(filterable)]
    pub s3_key: String,  // Path in S3 bucket

    #[crudcrate(filterable)]
    pub bucket: String,

    #[crudcrate(sortable)]
    pub size_bytes: i64,

    #[crudcrate(exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

// ============================================================================
// Mock S3 Client (Use aws-sdk-s3 in production)
// ============================================================================

async fn delete_s3_object(bucket: &str, key: &str) -> Result<(), String> {
    // In production: use AWS SDK to delete from S3
    println!("   🗑️  Deleted S3 object: s3://{}/{}", bucket, key);
    Ok(())
}

// ============================================================================
// Custom Delete Functions with S3 Cleanup
// ============================================================================

/// Delete single asset with S3 cleanup
async fn delete_asset(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), DbErr> {
    // 1. Fetch asset to get S3 key
    let asset = Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("Asset not found".to_string()))?;

    // 2. Delete from S3 first (fail fast if S3 is down)
    delete_s3_object(&asset.bucket, &asset.s3_key)
        .await
        .map_err(|e| DbErr::Custom(format!("S3 deletion failed: {}", e)))?;

    // 3. Delete from database
    Entity::delete_by_id(id).exec(db).await?;

    Ok(())
}

/// Delete multiple assets with S3 cleanup
/// Returns IDs that were successfully deleted
async fn delete_many_assets(
    db: &DatabaseConnection,
    ids: Vec<Uuid>,
) -> Result<Vec<Uuid>, DbErr> {
    let mut deleted_ids = Vec::new();

    for id in &ids {
        // Fetch asset
        let asset = match Entity::find_by_id(*id).one(db).await? {
            Some(a) => a,
            None => continue, // Skip if not found
        };

        // Try to delete from S3 (graceful failure)
        let s3_result = delete_s3_object(&asset.bucket, &asset.s3_key).await;

        if s3_result.is_err() {
            eprintln!("⚠️  S3 deletion failed for {}, skipping DB deletion", id);
            continue;
        }

        // Delete from database
        if Entity::delete_by_id(*id).exec(db).await.is_ok() {
            deleted_ids.push(*id);
        }
    }

    Ok(deleted_ids)
}

// ============================================================================
// Example Usage
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🪣 S3 Cleanup on Delete Example\n");

    let db = Database::connect("sqlite::memory:").await?;

    db.execute(sea_orm::Statement::from_string(
        db.get_database_backend(),
        r#"CREATE TABLE assets (
            id TEXT PRIMARY KEY,
            filename TEXT NOT NULL,
            s3_key TEXT NOT NULL,
            bucket TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            created_at TEXT NOT NULL
        )"#.to_owned(),
    )).await?;

    // Create test assets
    println!("1️⃣  Creating test assets...");
    let asset1 = Asset::create(&db, AssetCreate {
        filename: "report.pdf".to_string(),
        s3_key: "uploads/2024/report.pdf".to_string(),
        bucket: "my-bucket".to_string(),
        size_bytes: 1024000,
    }).await?;

    let asset2 = Asset::create(&db, AssetCreate {
        filename: "image.jpg".to_string(),
        s3_key: "uploads/2024/image.jpg".to_string(),
        bucket: "my-bucket".to_string(),
        size_bytes: 512000,
    }).await?;

    println!("   ✅ Created 2 assets\n");

    // Delete single asset (with S3 cleanup)
    println!("2️⃣  Deleting single asset (with S3 cleanup)...");
    Asset::delete(&db, asset1.id).await?;
    println!("   ✅ Deleted asset {} and its S3 object\n", asset1.id);

    // Delete multiple assets
    println!("3️⃣  Deleting multiple assets...");
    let deleted = Asset::delete_many(&db, vec![asset2.id]).await?;
    println!("   ✅ Deleted {} assets with S3 cleanup\n", deleted.len());

    println!("✅ Example complete!");
    println!("\n💡 Production Tips:");
    println!("   • Use aws-sdk-s3 for real S3 operations");
    println!("   • Consider queuing S3 deletes for async processing");
    println!("   • Log failed S3 deletes for manual cleanup");
    println!("   • Handle partial failures gracefully in delete_many");

    Ok(())
}
