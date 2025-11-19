use async_trait::async_trait;
use crudcrate::{ApiError, CRUDOperations, CRUDResource, EntityToModels};
use sea_orm::{Condition, DatabaseConnection, Order, entity::prelude::*};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "blog_posts")]
#[crudcrate(
    api_struct = "BlogPost",
    generate_router,
    operations = BlogPostOperations  // ← Use our custom operations with hooks
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub title: String,

    #[crudcrate(filterable)]
    pub content: String,

    #[crudcrate(filterable)]
    pub published: bool,

    #[crudcrate(filterable)]
    pub image_s3_key: Option<String>,

    // This field will be populated by after_get_one hook
    #[crudcrate(exclude(create, update))]
    pub view_count: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

//
// Custom operations with lifecycle hooks
//

pub struct BlogPostOperations;

#[async_trait]
impl CRUDOperations for BlogPostOperations {
    type Resource = BlogPost;

    // ===========================================
    // LEVEL 1: LIFECYCLE HOOKS
    // ===========================================

    /// Before creating: Validate content length and sanitize
    async fn before_create(
        &self,
        _db: &DatabaseConnection,
        data: &BlogPostCreate,
    ) -> Result<(), ApiError> {
        println!("🔍 [HOOK] Validating blog post before creation...");

        // Validation: Content must be at least 100 characters
        if data.content.len() < 100 {
            return Err(ApiError::bad_request(format!(
                "Blog post content too short: {} chars (minimum 100 required)",
                data.content.len()
            )));
        }

        // Validation: Title must not be empty
        if data.title.trim().is_empty() {
            return Err(ApiError::bad_request("Blog post title cannot be empty"));
        }

        println!("   ✓ Validation passed");
        Ok(())
    }

    /// After creating: Log and send notification
    async fn after_create(
        &self,
        _db: &DatabaseConnection,
        entity: &mut BlogPost,
    ) -> Result<(), ApiError> {
        println!("📢 [HOOK] Blog post created: \"{}\"", entity.title);
        Ok(())
    }

    async fn before_get_one(&self, _db: &DatabaseConnection, id: Uuid) -> Result<(), ApiError> {
        println!("🔍 [HOOK] Fetching blog post {id}...");
        Err(ApiError::internal("Failed to fetch blog post", None))
    }

    /// After fetching one: Enrich with view count and increment it
    async fn after_get_one(
        &self,
        db: &DatabaseConnection,
        entity: &mut BlogPost,
    ) -> Result<(), ApiError> {
        // Fetch view count from analytics (simulated)
        entity.view_count = get_view_count(db, entity.id).await?;

        // Increment view count
        increment_view_count(db, entity.id).await?;
        println!(
            "Hook 🔢 View count for post {} is now {}",
            entity.id,
            entity.view_count + 1
        );

        Ok(())
    }

    /// Before deleting: Check permissions and log
    async fn before_delete(&self, _db: &DatabaseConnection, id: Uuid) -> Result<(), ApiError> {
        println!("🔐 [HOOK] Deleting blog post {id}...");
        Ok(())
    }

    // ===========================================
    // LEVEL 2: CORE LOGIC CUSTOMIZATION
    // ===========================================

    /// Custom fetch_all: Only return published posts by default
    async fn fetch_all(
        &self,
        db: &DatabaseConnection,
        condition: &Condition,
        order_column: <Self::Resource as CRUDResource>::ColumnType,
        order_direction: Order,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<<Self::Resource as CRUDResource>::ListModel>, ApiError> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

        // Add published=true filter to the condition
        let mut custom_condition = condition.clone();
        custom_condition = custom_condition.add(Column::Published.eq(true));

        let models = <Self::Resource as CRUDResource>::EntityType::find()
            .filter(custom_condition)
            .order_by(order_column, order_direction)
            .offset(offset)
            .limit(limit)
            .all(db)
            .await
            .map_err(ApiError::database)?;

        Ok(models
            .into_iter()
            .map(|model| {
                <Self::Resource as CRUDResource>::ListModel::from(Self::Resource::from(model))
            })
            .collect())
    }

    // ===========================================
    // LEVEL 3: FULL OPERATION OVERRIDE
    // ===========================================

    /// Complete override: Custom delete with S3 cleanup
    async fn delete(&self, db: &DatabaseConnection, id: Uuid) -> Result<Uuid, ApiError> {
        println!("🗑️  [FULL OVERRIDE] Custom delete with S3 cleanup");

        // 1. Fetch the post first (using default fetch_one)
        let post = self.fetch_one(db, id).await?;

        // 2. Delete S3 image if exists
        if let Some(s3_key) = &post.image_s3_key {
            println!("   🗑️  Deleting S3 image: {}", s3_key);
            delete_from_s3(s3_key)
                .await
                .map_err(|e| ApiError::internal(format!("S3 cleanup failed: {}", e), None))?;
        }

        // 3. Delete from database (using default perform_delete)
        let deleted_id = self.perform_delete(db, id).await?;

        println!("   ✓ Blog post deleted successfully");
        Ok(deleted_id)
    }
}

//
// Simulated helper functions
//

/// Simulated view count fetch from analytics service
async fn get_view_count(_db: &DatabaseConnection, _id: Uuid) -> Result<i32, ApiError> {
    // In real code: query analytics database or cache
    Ok(42)
}

/// Simulated view count increment
async fn increment_view_count(_db: &DatabaseConnection, _id: Uuid) -> Result<(), ApiError> {
    // In real code: increment counter in analytics service
    Ok(())
}

/// Simulated S3 deletion function
async fn delete_from_s3(s3_key: &str) -> Result<(), String> {
    // In real code: s3_client.delete_object().send().await?
    println!("      [S3] Deleted: {}", s3_key);
    Ok(())
}
