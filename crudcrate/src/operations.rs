//! Trait-based CRUD customization.
//!
//! [`CRUDOperations`] provides an alternative to per-attribute hooks: implement the trait
//! on a unit struct, override only the methods you need, and wire it in with
//! `#[crudcrate(operations = MyOps)]`.
//!
//! ```rust,ignore
//! pub struct AssetOps;
//!
//! #[async_trait]
//! impl CRUDOperations for AssetOps {
//!     type Resource = Asset;
//!
//!     // `id` is the resource's PK value type: `crudcrate::PrimaryKeyType<Self::Resource>`.
//!     async fn delete<C: ConnectionTrait>(
//!         &self,
//!         db: &C,
//!         id: crudcrate::PrimaryKeyType<Self::Resource>,
//!     ) -> Result<crudcrate::PrimaryKeyType<Self::Resource>, ApiError> {
//!         let asset = Asset::get_one(db, id).await?;
//!         delete_from_s3(&asset.s3_key).await
//!             .map_err(|e| ApiError::internal(format!("S3 cleanup failed: {e}"), None))?;
//!         Asset::delete(db, id).await
//!     }
//! }
//!
//! #[derive(EntityToModels)]
//! #[crudcrate(generate_router, operations = AssetOps)]
//! pub struct Model { /* ... */ }
//! ```

use sea_orm::{Condition, ConnectionTrait, Order, TransactionSession, TransactionTrait};

use crate::ApiError;
use crate::core::CRUDResource;
use crate::core::traits::PrimaryKeyType;

/// The primary-key value type of the resource this operations impl drives.
///
/// Shorthand for [`PrimaryKeyType`] applied to `O::Resource`, keeping the trait
/// method signatures readable while staying generic over the entity's PK type.
/// `PrimaryKeyType<R>` already carries the `CRUDResource` bound on `R`, so
/// `ResourceId<Self>` is well-formed wherever `Self: CRUDOperations`.
type ResourceId<O> = PrimaryKeyType<<O as CRUDOperations>::Resource>;

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
/// async fn before_create<C: ConnectionTrait>(&self, db: &C, data: &CreateModel) -> Result<(), DbErr> {
///     validate(data)?;
///     Ok(())
/// }
///
/// async fn after_get_one<C: ConnectionTrait>(&self, db: &C, entity: &mut Resource) -> Result<(), DbErr> {
///     entity.view_count = get_view_count(db, entity.id).await?;
///     Ok(())
/// }
/// ```
///
/// **Level 2: Core Logic** (custom queries, business logic)
/// ```rust,ignore
/// // `id` is the resource's PK value type: `crudcrate::PrimaryKeyType<Self::Resource>`.
/// async fn fetch_one<C: ConnectionTrait>(
///     &self,
///     db: &C,
///     id: crudcrate::PrimaryKeyType<Self::Resource>,
/// ) -> Result<Self::Resource, ApiError> {
///     // Custom query with joins
///     Entity::find_by_id(id).find_with_related(Related).one(db).await?.ok_or(...)
/// }
/// ```
///
/// **Level 3: Full Override** (complete control)
/// ```rust,ignore
/// // `id` is the resource's PK value type: `crudcrate::PrimaryKeyType<Self::Resource>`.
/// async fn delete<C: ConnectionTrait>(
///     &self,
///     db: &C,
///     id: crudcrate::PrimaryKeyType<Self::Resource>,
/// ) -> Result<crudcrate::PrimaryKeyType<Self::Resource>, ApiError> {
///     // Completely custom implementation
///     cleanup_s3(id).await?;
///     Entity::delete_by_id(id).exec(db).await?;
///     Ok(id)
/// }
/// ```
// The futures are not declared `Send`. Every caller in the stack is concrete by the
// time it awaits one, so the bound is inferred where it is needed; a generic caller
// that spawns one states `+ Send` itself.
#[allow(async_fn_in_trait)]
pub trait CRUDOperations: Send + Sync {
    /// The CRUD resource type this operations implementation works with
    type Resource: CRUDResource;

    /// Hook called on the transaction the single-row lifecycle runs in, immediately after `BEGIN`
    /// and before any other hook.
    ///
    /// Use for: session state the write itself needs, which is what `SET LOCAL` is for. It is set
    /// on the transaction rather than on a pooled connection, so it applies to exactly this
    /// lifecycle and is gone when the transaction ends.
    ///
    /// # Errors
    /// Return `ApiError` to abort the operation; the transaction is rolled back and nothing is
    /// written.
    ///
    /// # Example
    /// ```rust,ignore
    /// async fn after_begin<C: ConnectionTrait>(&self, db: &C) -> Result<(), ApiError> {
    ///     db.execute_unprepared("SET LOCAL app.actor = 'alice'").await?;
    ///     Ok(())
    /// }
    /// ```
    async fn after_begin<C: ConnectionTrait>(&self, _db: &C) -> Result<(), ApiError> {
        Ok(()) // Default: no-op
    }

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
    /// // `id` is the resource's PK value type: `crudcrate::PrimaryKeyType<Self::Resource>`.
    /// async fn before_get_one<C: ConnectionTrait>(
    ///     &self,
    ///     _db: &C,
    ///     id: crudcrate::PrimaryKeyType<Self::Resource>,
    /// ) -> Result<(), ApiError> {
    ///     if !has_permission(id) {
    ///         return Err(ApiError::forbidden("Access denied"));
    ///     }
    ///     Ok(())
    /// }
    /// ```
    async fn before_get_one<C: ConnectionTrait>(
        &self,
        _db: &C,
        _id: ResourceId<Self>,
    ) -> Result<(), ApiError> {
        Ok(()) // Default: no-op
    }

    /// Hook called after fetching a single entity
    ///
    /// Use for: enrichment, computed fields, audit logging
    ///
    /// # Errors
    /// Return `ApiError` to abort the operation
    async fn after_get_one<C: ConnectionTrait>(
        &self,
        _db: &C,
        _entity: &mut Self::Resource,
    ) -> Result<(), ApiError> {
        Ok(()) // Default: no-op
    }

    /// Core database fetch logic for a single entity
    ///
    /// Override this to customize the query (e.g., add joins, select specific columns)
    ///
    /// # Errors
    /// Returns `ApiError::NotFound` if entity doesn't exist
    async fn fetch_one<C: ConnectionTrait>(
        &self,
        db: &C,
        id: ResourceId<Self>,
    ) -> Result<Self::Resource, ApiError> {
        crate::core::defaults::get_one::<Self::Resource, _>(db, id).await
    }

    // ==========================================
    // LIFECYCLE HOOKS - GET ALL
    // ==========================================

    /// Hook called before fetching multiple entities
    async fn before_get_all<C: ConnectionTrait>(
        &self,
        _db: &C,
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
    async fn after_get_all<C: ConnectionTrait>(
        &self,
        _db: &C,
        _entities: &mut Vec<<Self::Resource as CRUDResource>::ListModel>,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    /// Core database fetch logic for multiple entities.
    ///
    /// The primary key is appended as a secondary sort key when the requested sort
    /// column is not the primary key, keeping `OFFSET`/`LIMIT` paging stable across
    /// rows that tie on the sort column.
    async fn fetch_all<C: ConnectionTrait>(
        &self,
        db: &C,
        condition: &Condition,
        order_column: <Self::Resource as CRUDResource>::ColumnType,
        order_direction: Order,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<<Self::Resource as CRUDResource>::ListModel>, ApiError> {
        crate::core::defaults::get_all::<Self::Resource, _>(
            db,
            condition,
            order_column,
            order_direction,
            offset,
            limit,
        )
        .await
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
    /// async fn before_create<C: ConnectionTrait>(&self, db: &C, data: &CreateModel) -> Result<(), ApiError> {
    ///     if data.price <= 0 {
    ///         return Err(ApiError::bad_request("Price must be greater than 0"));
    ///     }
    ///     Ok(())
    /// }
    /// ```
    async fn before_create<C: ConnectionTrait>(
        &self,
        _db: &C,
        _data: &<Self::Resource as CRUDResource>::CreateModel,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    /// Hook called after creating an entity
    ///
    /// Use for: sending notifications, logging, cache invalidation
    async fn after_create<C: ConnectionTrait>(
        &self,
        _db: &C,
        _entity: &mut Self::Resource,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    /// Core database insert logic
    async fn perform_create<C: ConnectionTrait>(
        &self,
        db: &C,
        data: <Self::Resource as CRUDResource>::CreateModel,
    ) -> Result<Self::Resource, ApiError> {
        crate::core::defaults::create::<Self::Resource, _>(db, data).await
    }

    // ==========================================
    // LIFECYCLE HOOKS - UPDATE
    // ==========================================

    /// Hook called before updating an entity
    async fn before_update<C: ConnectionTrait>(
        &self,
        _db: &C,
        _id: ResourceId<Self>,
        _data: &<Self::Resource as CRUDResource>::UpdateModel,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    /// Hook called after updating an entity
    async fn after_update<C: ConnectionTrait>(
        &self,
        _db: &C,
        _entity: &mut Self::Resource,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    /// Core database update logic
    async fn perform_update<C: ConnectionTrait>(
        &self,
        db: &C,
        id: ResourceId<Self>,
        data: <Self::Resource as CRUDResource>::UpdateModel,
    ) -> Result<Self::Resource, ApiError> {
        crate::core::defaults::update::<Self::Resource, _>(db, id, data).await
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
    /// // `id` is the resource's PK value type: `crudcrate::PrimaryKeyType<Self::Resource>`.
    /// async fn before_delete<C: ConnectionTrait>(
    ///     &self,
    ///     db: &C,
    ///     id: crudcrate::PrimaryKeyType<Self::Resource>,
    /// ) -> Result<(), ApiError> {
    ///     if !user_can_delete(id) {
    ///         return Err(ApiError::forbidden("You don't have permission to delete this resource"));
    ///     }
    ///     Ok(())
    /// }
    /// ```
    async fn before_delete<C: ConnectionTrait>(
        &self,
        _db: &C,
        _id: ResourceId<Self>,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    /// Hook called after deleting an entity
    ///
    /// Use for: cache invalidation, notifications, audit logging
    async fn after_delete<C: ConnectionTrait>(
        &self,
        _db: &C,
        _id: ResourceId<Self>,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    /// Core database delete logic
    async fn perform_delete<C: ConnectionTrait>(
        &self,
        db: &C,
        id: ResourceId<Self>,
    ) -> Result<ResourceId<Self>, ApiError> {
        crate::core::defaults::delete::<Self::Resource, _>(db, id).await
    }

    // ==========================================
    // LIFECYCLE HOOKS - DELETE MANY
    // ==========================================

    /// Hook called before batch deleting entities
    async fn before_delete_many<C: ConnectionTrait>(
        &self,
        _db: &C,
        _ids: &[ResourceId<Self>],
    ) -> Result<(), ApiError> {
        Ok(())
    }

    /// Hook called after batch deleting entities
    async fn after_delete_many<C: ConnectionTrait>(
        &self,
        _db: &C,
        _ids: &[ResourceId<Self>],
    ) -> Result<(), ApiError> {
        Ok(())
    }

    /// Core batch delete logic: the single-row [`Self::delete`] lifecycle per id, so
    /// `before_delete` and `after_delete` fire for every row. An id that is not there is skipped,
    /// which also de-duplicates a repeated one, so the return is the ids that existed, in input
    /// order. Override this to delete the batch in one statement, and take the hooks with it.
    ///
    /// # Errors
    ///
    /// Returns `ApiError` if any hook or delete fails for a row that exists
    async fn perform_delete_many<C: ConnectionTrait + TransactionTrait>(
        &self,
        db: &C,
        ids: Vec<ResourceId<Self>>,
    ) -> Result<Vec<ResourceId<Self>>, ApiError> {
        let mut deleted = Vec::with_capacity(ids.len());
        for id in ids {
            match self.delete(db, id).await {
                Ok(id) => deleted.push(id),
                Err(ApiError::NotFound { .. }) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(deleted)
    }

    // ==========================================
    // MAIN OPERATIONS (orchestrate hooks + core logic)
    // ==========================================

    /// Fetch a single entity by ID
    ///
    /// Orchestrates the full `get_one` lifecycle:
    /// 1. `before_get_one` - validation, auth, logging
    /// 2. `fetch_one` - database query
    /// 3. `after_get_one` - enrichment, computed fields
    ///
    /// # Errors
    ///
    /// Returns `ApiError::NotFound` if the entity doesn't exist
    /// Returns `ApiError` if any hook or core logic fails
    async fn get_one<C: ConnectionTrait>(
        &self,
        db: &C,
        id: ResourceId<Self>,
    ) -> Result<Self::Resource, ApiError> {
        // 1. Before hook
        self.before_get_one(db, id.clone()).await?;

        // 2. Core logic (fetch)
        let mut entity = self.fetch_one(db, id).await?;

        // 3. After hook
        self.after_get_one(db, &mut entity).await?;

        Ok(entity)
    }

    /// Fetch multiple entities with filtering, sorting, and pagination
    ///
    /// Orchestrates the full `get_all` lifecycle:
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
    async fn get_all<C: ConnectionTrait>(
        &self,
        db: &C,
        condition: &Condition,
        order_column: <Self::Resource as CRUDResource>::ColumnType,
        order_direction: Order,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<<Self::Resource as CRUDResource>::ListModel>, ApiError> {
        // 1. Before hook
        self.before_get_all(db, condition, order_column, &order_direction, offset, limit)
            .await?;

        // 2. Core logic (fetch)
        let mut entities = self
            .fetch_all(db, condition, order_column, order_direction, offset, limit)
            .await?;

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
    async fn create<C: ConnectionTrait + TransactionTrait>(
        &self,
        db: &C,
        data: <Self::Resource as CRUDResource>::CreateModel,
    ) -> Result<Self::Resource, ApiError> {
        let txn = db.begin().await.map_err(ApiError::database)?;

        // 1. Session state for the write, on the transaction it happens in
        self.after_begin(&txn).await?;

        // 2. Before hook
        self.before_create(&txn, &data).await?;

        // 3. Core logic (insert)
        let mut entity = self.perform_create(&txn, data).await?;

        // 4. After hook
        self.after_create(&txn, &mut entity).await?;

        txn.commit().await.map_err(ApiError::database)?;
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
    async fn update<C: ConnectionTrait + TransactionTrait>(
        &self,
        db: &C,
        id: ResourceId<Self>,
        data: <Self::Resource as CRUDResource>::UpdateModel,
    ) -> Result<Self::Resource, ApiError> {
        let txn = db.begin().await.map_err(ApiError::database)?;

        // 1. Session state for the write, on the transaction it happens in
        self.after_begin(&txn).await?;

        // 2. Before hook
        self.before_update(&txn, id.clone(), &data).await?;

        // 3. Core logic (update)
        let mut entity = self.perform_update(&txn, id, data).await?;

        // 4. After hook
        self.after_update(&txn, &mut entity).await?;

        txn.commit().await.map_err(ApiError::database)?;
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
    async fn delete<C: ConnectionTrait + TransactionTrait>(
        &self,
        db: &C,
        id: ResourceId<Self>,
    ) -> Result<ResourceId<Self>, ApiError> {
        let txn = db.begin().await.map_err(ApiError::database)?;

        // 1. Session state for the write, on the transaction it happens in
        self.after_begin(&txn).await?;

        // 2. Before hook
        self.before_delete(&txn, id.clone()).await?;

        // 3. Core logic (delete)
        let deleted_id = self.perform_delete(&txn, id).await?;

        // 4. After hook
        self.after_delete(&txn, deleted_id.clone()).await?;

        txn.commit().await.map_err(ApiError::database)?;
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
    async fn delete_many<C: ConnectionTrait + TransactionTrait>(
        &self,
        db: &C,
        ids: Vec<ResourceId<Self>>,
    ) -> Result<Vec<ResourceId<Self>>, ApiError> {
        let txn = db.begin().await.map_err(ApiError::database)?;

        // 1. Before hook
        self.before_delete_many(&txn, &ids).await?;

        // 2. Core logic (batch delete)
        let deleted_ids = self.perform_delete_many(&txn, ids).await?;

        // 3. After hook
        self.after_delete_many(&txn, &deleted_ids).await?;

        txn.commit().await.map_err(ApiError::database)?;
        Ok(deleted_ids)
    }

    /// Create multiple entities in a batch
    ///
    /// Runs the single-row [`Self::create`] lifecycle per item, so `before_create` and
    /// `after_create` fire for every row, inside one transaction the batch opens: a failure at any
    /// row leaves none of them, so a caller reading an error never has to ask which half landed.
    /// Each row's own `begin` is a savepoint within it. Override this to insert the batch in one
    /// statement, and take the hooks with it.
    ///
    /// **Security**: Limited to `batch_limit()` items (100 by default) to prevent `DoS`.
    ///
    /// # Errors
    ///
    /// Returns `ApiError` if the batch size exceeds the security limit (default: 100)
    /// Returns `ApiError` if any validation or database insertion fails
    async fn create_many<C: ConnectionTrait + TransactionTrait>(
        &self,
        db: &C,
        data: Vec<<Self::Resource as CRUDResource>::CreateModel>,
    ) -> Result<Vec<Self::Resource>, ApiError> {
        if data.len() > Self::Resource::batch_limit() {
            return Err(ApiError::bad_request(format!(
                "Batch create limited to {} items. Received {} items.",
                Self::Resource::batch_limit(),
                data.len()
            )));
        }
        let txn = db.begin().await.map_err(ApiError::database)?;
        let mut created = Vec::with_capacity(data.len());
        for item in data {
            created.push(self.create(&txn, item).await?);
        }
        txn.commit().await.map_err(ApiError::database)?;
        Ok(created)
    }

    /// Update multiple entities in a batch
    ///
    /// Runs the single-row [`Self::update`] lifecycle per item, so `before_update` and
    /// `after_update` fire for every row, inside one transaction the batch opens: a failure at any
    /// row leaves none of them edited. Each row's own `begin` is a savepoint within it. Override
    /// this to update the batch in one statement, and take the hooks with it.
    ///
    /// **Security**: Limited to `batch_limit()` items (100 by default) to prevent `DoS`.
    ///
    /// # Errors
    ///
    /// Returns `ApiError` if the batch size exceeds the security limit (default: 100)
    /// Returns `ApiError` if any validation or database update fails
    async fn update_many<C: ConnectionTrait + TransactionTrait>(
        &self,
        db: &C,
        updates: Vec<(
            ResourceId<Self>,
            <Self::Resource as CRUDResource>::UpdateModel,
        )>,
    ) -> Result<Vec<Self::Resource>, ApiError> {
        if updates.len() > Self::Resource::batch_limit() {
            return Err(ApiError::bad_request(format!(
                "Batch update limited to {} items. Received {} items.",
                Self::Resource::batch_limit(),
                updates.len()
            )));
        }
        let txn = db.begin().await.map_err(ApiError::database)?;
        let mut updated = Vec::with_capacity(updates.len());
        for (id, data) in updates {
            updated.push(self.update(&txn, id, data).await?);
        }
        txn.commit().await.map_err(ApiError::database)?;
        Ok(updated)
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
#[deprecated(
    note = "never constructed by generated code; implement CRUDOperations on your own type; removed in the next breaking release"
)]
pub struct DefaultCRUDOperations<T: CRUDResource> {
    _phantom: std::marker::PhantomData<T>,
}

#[allow(deprecated)]
impl<T: CRUDResource> DefaultCRUDOperations<T> {
    /// Create a new default operations instance
    #[must_use]
    pub const fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

#[allow(deprecated)]
impl<T: CRUDResource> Default for DefaultCRUDOperations<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(deprecated)]
impl<T: CRUDResource> CRUDOperations for DefaultCRUDOperations<T> {
    type Resource = T;

    // All methods use default implementations from the trait
    // No overrides needed - delegates to T::method() automatically
}
