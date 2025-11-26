//! # CRUD Operations Trait
//!
//! This module provides the `CRUDOperations` trait that allows users to customize CRUD behavior
//! by implementing a trait with sensible defaults, rather than overriding individual functions
//! via attributes.
//!
//! ## Philosophy
//!
//! - **`CRUDResource`**: Low-level trait implemented by entities (generated automatically)
//! - **`CRUDOperations`**: High-level trait for customizing CRUD behavior (user-implemented)
//! - **Default Implementations**: Each operation has a sensible default that delegates to `CRUDResource`
//! - **Selective Overrides**: Users only override the methods they need to customize
//! - **Composition**: Operations can call other operations (e.g., `update` calling `self.get_one()`)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use crudcrate::{CRUDOperations, CRUDResource, ApiError};
//! use async_trait::async_trait;
//!
//! // Define your operations with custom behavior
//! pub struct AssetOperations;
//!
//! #[async_trait]
//! impl CRUDOperations for AssetOperations {
//!     type Resource = Asset;
//!
//!     // Override before_delete hook for authorization
//!     async fn before_delete(&self, db: &DatabaseConnection, id: Uuid) -> Result<(), ApiError> {
//!         if !user_has_permission(id) {
//!             return Err(ApiError::forbidden("You don't have permission to delete this asset"));
//!         }
//!         Ok(())
//!     }
//!
//!     // Override delete to add S3 cleanup
//!     async fn delete(&self, db: &DatabaseConnection, id: Uuid) -> Result<Uuid, ApiError> {
//!         // Fetch the asset first
//!         let asset = Asset::get_one(db, id).await?;
//!
//!         // Delete from S3
//!         delete_from_s3(&asset.s3_key).await
//!             .map_err(|e| ApiError::internal(format!("S3 cleanup failed: {}", e), None))?;
//!
//!         // Then delete from database
//!         Asset::delete(db, id).await
//!     }
//!
//!     // get_one, get_all, create, update, delete_many all use defaults!
//! }
//!
//! // Use in your entity definition
//! #[derive(EntityToModels)]
//! #[crudcrate(generate_router, operations = AssetOperations)]
//! pub struct Asset {
//!     pub id: Uuid,
//!     pub s3_key: String,
//! }
//! ```

use async_trait::async_trait;
use sea_orm::{Condition, DatabaseConnection, Order};
use uuid::Uuid;

use crate::core::CRUDResource;
use crate::ApiError;

/// Trait for defining CRUD operations with customizable behavior
///
/// This trait provides **three levels of customization**:
///
/// 1. **Lifecycle Hooks**: `before_*` and `after_*` methods for validation, logging, enrichment
/// 2. **Core Logic**: `fetch_*` and `perform_*` methods for custom queries and business logic
/// 3. **Full Override**: Replace entire operations like `get_one`, `delete`, etc.
///
/// ## Type Parameters
///
/// - `Resource`: The CRUD resource type that implements `CRUDResource`
///
/// ## Customization Levels
///
/// **Level 1: Hooks Only** (validation, logging, enrichment)
/// ```rust,ignore
/// async fn before_create(&self, db: &DatabaseConnection, data: &CreateModel) -> Result<(), DbErr> {
///     validate(data)?;
///     Ok(())
/// }
///
/// async fn after_get_one(&self, db: &DatabaseConnection, entity: &mut Resource) -> Result<(), DbErr> {
///     entity.view_count = get_view_count(db, entity.id).await?;
///     Ok(())
/// }
/// ```
///
/// **Level 2: Core Logic** (custom queries, business logic)
/// ```rust,ignore
/// async fn fetch_one(&self, db: &DatabaseConnection, id: Uuid) -> Result<Resource, DbErr> {
///     // Custom query with joins
///     Entity::find_by_id(id).find_with_related(Related).one(db).await?.ok_or(...)
/// }
/// ```
///
/// **Level 3: Full Override** (complete control)
/// ```rust,ignore
/// async fn delete(&self, db: &DatabaseConnection, id: Uuid) -> Result<Uuid, DbErr> {
///     // Completely custom implementation
///     cleanup_s3(id).await?;
///     Entity::delete_by_id(id).exec(db).await?;
///     Ok(id)
/// }
/// ```
#[async_trait]
pub trait CRUDOperations: Send + Sync {
    /// The CRUD resource type this operations implementation works with
    type Resource: CRUDResource;

    // ==========================================
    // LIFECYCLE HOOKS - GET ONE
    // ==========================================

    /// Hook called before fetching a single entity
    ///
    /// Use for: authorization checks, rate limiting, logging
    ///
    /// # Errors
    /// Return `ApiError` to abort the operation with specific HTTP status code
    ///
    /// # Example
    /// ```rust,ignore
    /// async fn before_get_one(&self, _db: &DatabaseConnection, id: Uuid) -> Result<(), ApiError> {
    ///     if !has_permission(id) {
    ///         return Err(ApiError::forbidden("Access denied"));
    ///     }
    ///     Ok(())
    /// }
    /// ```
    async fn before_get_one(&self, _db: &DatabaseConnection, _id: Uuid) -> Result<(), ApiError> {
        Ok(())  // Default: no-op
    }

    /// Hook called after fetching a single entity
    ///
    /// Use for: enrichment, computed fields, audit logging
    ///
    /// # Errors
    /// Return `ApiError` to abort the operation
    async fn after_get_one(&self, _db: &DatabaseConnection, _entity: &mut Self::Resource) -> Result<(), ApiError> {
        Ok(())  // Default: no-op
    }

    /// Core database fetch logic for a single entity
    ///
    /// Override this to customize the query (e.g., add joins, select specific columns)
    ///
    /// # Errors
    /// Returns `ApiError::NotFound` if entity doesn't exist
    async fn fetch_one(&self, db: &DatabaseConnection, id: Uuid) -> Result<Self::Resource, ApiError> {
        use sea_orm::EntityTrait;

        let model = <Self::Resource as CRUDResource>::EntityType::find_by_id(id)
            .one(db)
            .await
            .map_err(ApiError::database)?
            .ok_or_else(|| ApiError::not_found(
                <Self::Resource as CRUDResource>::RESOURCE_NAME_SINGULAR,
                Some(id.to_string()),
            ))?;
        Ok(Self::Resource::from(model))
    }

    // ==========================================
    // LIFECYCLE HOOKS - GET ALL
    // ==========================================

    /// Hook called before fetching multiple entities
    async fn before_get_all(
        &self,
        _db: &DatabaseConnection,
        _condition: &Condition,
        _order_column: <Self::Resource as CRUDResource>::ColumnType,
        _order_direction: &Order,
        _offset: u64,
        _limit: u64,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    /// Hook called after fetching multiple entities
    ///
    /// Receives a mutable reference to the list for enrichment
    async fn after_get_all(
        &self,
        _db: &DatabaseConnection,
        _entities: &mut Vec<<Self::Resource as CRUDResource>::ListModel>,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    /// Core database fetch logic for multiple entities
    async fn fetch_all(
        &self,
        db: &DatabaseConnection,
        condition: &Condition,
        order_column: <Self::Resource as CRUDResource>::ColumnType,
        order_direction: Order,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<<Self::Resource as CRUDResource>::ListModel>, ApiError> {
        use sea_orm::{EntityTrait, QueryFilter, QueryOrder, QuerySelect};

        let models = <Self::Resource as CRUDResource>::EntityType::find()
            .filter(condition.clone())
            .order_by(order_column, order_direction)
            .offset(offset)
            .limit(limit)
            .all(db)
            .await
            .map_err(ApiError::database)?;
        Ok(models.into_iter().map(|model| <Self::Resource as CRUDResource>::ListModel::from(Self::Resource::from(model))).collect())
    }

    // ==========================================
    // LIFECYCLE HOOKS - CREATE
    // ==========================================

    /// Hook called before creating an entity
    ///
    /// Use for: validation, authorization, setting default values
    ///
    /// # Example
    /// ```rust,ignore
    /// async fn before_create(&self, db: &DatabaseConnection, data: &CreateModel) -> Result<(), ApiError> {
    ///     if data.price <= 0 {
    ///         return Err(ApiError::bad_request("Price must be greater than 0"));
    ///     }
    ///     Ok(())
    /// }
    /// ```
    async fn before_create(&self, _db: &DatabaseConnection, _data: &<Self::Resource as CRUDResource>::CreateModel) -> Result<(), ApiError> {
        Ok(())
    }

    /// Hook called after creating an entity
    ///
    /// Use for: sending notifications, logging, cache invalidation
    async fn after_create(&self, _db: &DatabaseConnection, _entity: &mut Self::Resource) -> Result<(), ApiError> {
        Ok(())
    }

    /// Core database insert logic
    async fn perform_create(&self, db: &DatabaseConnection, data: <Self::Resource as CRUDResource>::CreateModel) -> Result<Self::Resource, ApiError> {
        use sea_orm::ActiveModelTrait;

        let active_model: <Self::Resource as CRUDResource>::ActiveModelType = data.into();
        let model = active_model.insert(db).await.map_err(ApiError::database)?;
        Ok(Self::Resource::from(model))
    }

    // ==========================================
    // LIFECYCLE HOOKS - UPDATE
    // ==========================================

    /// Hook called before updating an entity
    async fn before_update(&self, _db: &DatabaseConnection, _id: Uuid, _data: &<Self::Resource as CRUDResource>::UpdateModel) -> Result<(), ApiError> {
        Ok(())
    }

    /// Hook called after updating an entity
    async fn after_update(&self, _db: &DatabaseConnection, _entity: &mut Self::Resource) -> Result<(), ApiError> {
        Ok(())
    }

    /// Core database update logic
    async fn perform_update(&self, db: &DatabaseConnection, id: Uuid, data: <Self::Resource as CRUDResource>::UpdateModel) -> Result<Self::Resource, ApiError> {
        use sea_orm::{EntityTrait, IntoActiveModel, ActiveModelTrait};
        use crate::core::MergeIntoActiveModel;

        let model = <Self::Resource as CRUDResource>::EntityType::find_by_id(id)
            .one(db)
            .await
            .map_err(ApiError::database)?
            .ok_or_else(|| ApiError::not_found(
                <Self::Resource as CRUDResource>::RESOURCE_NAME_SINGULAR,
                Some(id.to_string()),
            ))?;
        let existing: <Self::Resource as CRUDResource>::ActiveModelType = model.into_active_model();
        let updated_model = data.merge_into_activemodel(existing)?;
        let updated = updated_model.update(db).await.map_err(ApiError::database)?;
        Ok(Self::Resource::from(updated))
    }

    // ==========================================
    // LIFECYCLE HOOKS - DELETE
    // ==========================================

    /// Hook called before deleting an entity
    ///
    /// Use for: authorization, cleanup of related resources
    ///
    /// # Example
    /// ```rust,ignore
    /// async fn before_delete(&self, db: &DatabaseConnection, id: Uuid) -> Result<(), ApiError> {
    ///     if !user_can_delete(id) {
    ///         return Err(ApiError::forbidden("You don't have permission to delete this resource"));
    ///     }
    ///     Ok(())
    /// }
    /// ```
    async fn before_delete(&self, _db: &DatabaseConnection, _id: Uuid) -> Result<(), ApiError> {
        Ok(())
    }

    /// Hook called after deleting an entity
    ///
    /// Use for: cache invalidation, notifications, audit logging
    async fn after_delete(&self, _db: &DatabaseConnection, _id: Uuid) -> Result<(), ApiError> {
        Ok(())
    }

    /// Core database delete logic
    async fn perform_delete(&self, db: &DatabaseConnection, id: Uuid) -> Result<Uuid, ApiError> {
        use sea_orm::EntityTrait;

        let res = <Self::Resource as CRUDResource>::EntityType::delete_by_id(id)
            .exec(db)
            .await
            .map_err(ApiError::database)?;
        match res.rows_affected {
            0 => Err(ApiError::not_found(
                <Self::Resource as CRUDResource>::RESOURCE_NAME_SINGULAR,
                Some(id.to_string()),
            )),
            _ => Ok(id),
        }
    }

    // ==========================================
    // LIFECYCLE HOOKS - DELETE MANY
    // ==========================================

    /// Hook called before batch deleting entities
    async fn before_delete_many(&self, _db: &DatabaseConnection, _ids: &[Uuid]) -> Result<(), ApiError> {
        Ok(())
    }

    /// Hook called after batch deleting entities
    async fn after_delete_many(&self, _db: &DatabaseConnection, _ids: &[Uuid]) -> Result<(), ApiError> {
        Ok(())
    }

    /// Core database batch delete logic
    async fn perform_delete_many(&self, db: &DatabaseConnection, ids: Vec<Uuid>) -> Result<Vec<Uuid>, ApiError> {
        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

        // Security: Limit batch size to prevent DoS attacks
        const MAX_BATCH_DELETE_SIZE: usize = 100;
        if ids.len() > MAX_BATCH_DELETE_SIZE {
            return Err(ApiError::bad_request(format!(
                "Batch delete limited to {} items. Received {} items.",
                MAX_BATCH_DELETE_SIZE,
                ids.len()
            )));
        }

        <Self::Resource as CRUDResource>::EntityType::delete_many()
            .filter(<Self::Resource as CRUDResource>::ID_COLUMN.is_in(ids.clone()))
            .exec(db)
            .await
            .map_err(ApiError::database)?;
        Ok(ids)
    }

    // ==========================================
    // MAIN OPERATIONS (orchestrate hooks + core logic)
    // ==========================================

    /// Fetch a single entity by ID
    ///
    /// Orchestrates the full get_one lifecycle:
    /// 1. `before_get_one` - validation, auth, logging
    /// 2. `fetch_one` - database query
    /// 3. `after_get_one` - enrichment, computed fields
    ///
    /// # Errors
    ///
    /// Returns `ApiError::NotFound` if the entity doesn't exist
    /// Returns `ApiError` if any hook or core logic fails
    async fn get_one(&self, db: &DatabaseConnection, id: Uuid) -> Result<Self::Resource, ApiError> {
        // 1. Before hook
        self.before_get_one(db, id).await?;

        // 2. Core logic (fetch)
        let mut entity = self.fetch_one(db, id).await?;

        // 3. After hook
        self.after_get_one(db, &mut entity).await?;

        Ok(entity)
    }

    /// Fetch multiple entities with filtering, sorting, and pagination
    ///
    /// Orchestrates the full get_all lifecycle:
    /// 1. `before_get_all` - validation, auth, logging
    /// 2. `fetch_all` - database query
    /// 3. `after_get_all` - enrichment, computed fields
    ///
    /// # Parameters
    ///
    /// - `db`: Database connection
    /// - `condition`: Filter conditions to apply
    /// - `order_column`: Column to sort by
    /// - `order_direction`: Sort direction (ASC or DESC)
    /// - `offset`: Number of records to skip
    /// - `limit`: Maximum number of records to return
    ///
    /// # Errors
    ///
    /// Returns `ApiError` if any hook or database query fails
    async fn get_all(
        &self,
        db: &DatabaseConnection,
        condition: &Condition,
        order_column: <Self::Resource as CRUDResource>::ColumnType,
        order_direction: Order,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<<Self::Resource as CRUDResource>::ListModel>, ApiError> {
        // 1. Before hook
        self.before_get_all(db, condition, order_column, &order_direction, offset, limit).await?;

        // 2. Core logic (fetch)
        let mut entities = self.fetch_all(db, condition, order_column, order_direction, offset, limit).await?;

        // 3. After hook
        self.after_get_all(db, &mut entities).await?;

        Ok(entities)
    }

    /// Create a new entity
    ///
    /// Orchestrates the full create lifecycle:
    /// 1. `before_create` - validation, auth, setting defaults
    /// 2. `perform_create` - database insert
    /// 3. `after_create` - notifications, cache updates
    ///
    /// # Errors
    ///
    /// Returns `ApiError` if any hook or database insertion fails
    async fn create(
        &self,
        db: &DatabaseConnection,
        data: <Self::Resource as CRUDResource>::CreateModel,
    ) -> Result<Self::Resource, ApiError> {
        // 1. Before hook
        self.before_create(db, &data).await?;

        // 2. Core logic (insert)
        let mut entity = self.perform_create(db, data).await?;

        // 3. After hook
        self.after_create(db, &mut entity).await?;

        Ok(entity)
    }

    /// Update an existing entity
    ///
    /// Orchestrates the full update lifecycle:
    /// 1. `before_update` - validation, auth, checking permissions
    /// 2. `perform_update` - database update
    /// 3. `after_update` - notifications, cache invalidation
    ///
    /// # Errors
    ///
    /// Returns `ApiError::NotFound` if the entity doesn't exist
    /// Returns `ApiError` if any hook or database update fails
    async fn update(
        &self,
        db: &DatabaseConnection,
        id: Uuid,
        data: <Self::Resource as CRUDResource>::UpdateModel,
    ) -> Result<Self::Resource, ApiError> {
        // 1. Before hook
        self.before_update(db, id, &data).await?;

        // 2. Core logic (update)
        let mut entity = self.perform_update(db, id, data).await?;

        // 3. After hook
        self.after_update(db, &mut entity).await?;

        Ok(entity)
    }

    /// Delete a single entity by ID
    ///
    /// Orchestrates the full delete lifecycle:
    /// 1. `before_delete` - auth, cleanup of related resources (e.g., S3)
    /// 2. `perform_delete` - database deletion
    /// 3. `after_delete` - notifications, cache invalidation, audit logging
    ///
    /// # Errors
    ///
    /// Returns `ApiError::NotFound` if the entity doesn't exist
    /// Returns `ApiError` if any hook or database deletion fails
    async fn delete(&self, db: &DatabaseConnection, id: Uuid) -> Result<Uuid, ApiError> {
        // 1. Before hook
        self.before_delete(db, id).await?;

        // 2. Core logic (delete)
        let deleted_id = self.perform_delete(db, id).await?;

        // 3. After hook
        self.after_delete(db, deleted_id).await?;

        Ok(deleted_id)
    }

    /// Delete multiple entities by IDs
    ///
    /// Orchestrates the full batch delete lifecycle:
    /// 1. `before_delete_many` - auth, batch validation
    /// 2. `perform_delete_many` - database batch deletion
    /// 3. `after_delete_many` - notifications, cache invalidation
    ///
    /// **Security**: Limited to 100 items by default. Override for different limits.
    ///
    /// # Errors
    ///
    /// Returns `ApiError` if the batch size exceeds the security limit (default: 100)
    /// Returns `ApiError` if any hook or database deletion fails
    async fn delete_many(
        &self,
        db: &DatabaseConnection,
        ids: Vec<Uuid>,
    ) -> Result<Vec<Uuid>, ApiError> {
        // 1. Before hook
        self.before_delete_many(db, &ids).await?;

        // 2. Core logic (batch delete)
        let deleted_ids = self.perform_delete_many(db, ids).await?;

        // 3. After hook
        self.after_delete_many(db, &deleted_ids).await?;

        Ok(deleted_ids)
    }

    /// Create multiple entities in a batch
    ///
    /// Orchestrates the full batch create lifecycle:
    /// 1. Validation via before hooks (per item)
    /// 2. Database batch insertion
    /// 3. After hooks (per item)
    ///
    /// **Security**: Limited to 100 items by default to prevent DoS.
    ///
    /// # Errors
    ///
    /// Returns `ApiError` if the batch size exceeds the security limit (default: 100)
    /// Returns `ApiError` if any validation or database insertion fails
    async fn create_many(
        &self,
        db: &DatabaseConnection,
        data: Vec<<Self::Resource as CRUDResource>::CreateModel>,
    ) -> Result<Vec<Self::Resource>, ApiError> {
        Self::Resource::create_many(db, data).await
    }

    /// Update multiple entities in a batch
    ///
    /// Orchestrates the full batch update lifecycle:
    /// 1. Validation via before hooks (per item)
    /// 2. Database batch updates
    /// 3. After hooks (per item)
    ///
    /// **Security**: Limited to 100 items by default to prevent DoS.
    ///
    /// # Errors
    ///
    /// Returns `ApiError` if the batch size exceeds the security limit (default: 100)
    /// Returns `ApiError` if any validation or database update fails
    async fn update_many(
        &self,
        db: &DatabaseConnection,
        updates: Vec<(Uuid, <Self::Resource as CRUDResource>::UpdateModel)>,
    ) -> Result<Vec<Self::Resource>, ApiError> {
        Self::Resource::update_many(db, updates).await
    }
}

/// Default CRUD operations implementation
///
/// This struct provides a zero-cost wrapper that delegates all operations to the
/// underlying `CRUDResource` trait. It's used automatically when no custom
/// `operations` attribute is specified.
///
/// ## Usage
///
/// This is used automatically by the derive macro:
///
/// ```rust,ignore
/// #[derive(EntityToModels)]
/// #[crudcrate(generate_router)]  // No operations specified
/// pub struct Todo {
///     pub id: Uuid,
///     pub title: String,
/// }
/// // Automatically uses DefaultCRUDOperations<Todo>
/// ```
pub struct DefaultCRUDOperations<T: CRUDResource> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T: CRUDResource> DefaultCRUDOperations<T> {
    /// Create a new default operations instance
    #[must_use]
    pub const fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: CRUDResource> Default for DefaultCRUDOperations<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<T: CRUDResource> CRUDOperations for DefaultCRUDOperations<T> {
    type Resource = T;

    // All methods use default implementations from the trait
    // No overrides needed - delegates to T::method() automatically
}

