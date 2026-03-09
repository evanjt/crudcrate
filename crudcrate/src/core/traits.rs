use async_trait::async_trait;
use sea_orm::{
    Condition, DatabaseConnection, EntityTrait, IntoActiveModel, Order, PaginatorTrait, QueryOrder,
    QuerySelect, entity::prelude::*,
};
use uuid::Uuid;

use crate::ApiError;

/// Helper for extracting UUID PKs in batch queries.
/// Used by `delete_many` to verify which IDs actually existed.
#[derive(Debug, sea_orm::FromQueryResult)]
pub struct UuidIdResult {
    pub id: Uuid,
}

pub trait MergeIntoActiveModel<ActiveModelType> {
    /// Merge this update model into an existing active model
    ///
    /// # Errors
    ///
    /// Returns an `ApiError` if the merge operation fails due to data conversion issues.
    fn merge_into_activemodel(self, existing: ActiveModelType)
    -> Result<ActiveModelType, ApiError>;
}

#[async_trait]
pub trait CRUDResource: Sized + Send + Sync
where
    Self::EntityType: EntityTrait + Sync,
    Self::ActiveModelType: ActiveModelTrait + ActiveModelBehavior + Send + Sync,
    <Self::EntityType as EntityTrait>::Model: Sync + IntoActiveModel<Self::ActiveModelType>,
    <<Self::EntityType as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType: From<Uuid>,
    <<Self::EntityType as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType: Into<Uuid>,
    <<Self::EntityType as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType: Into<Uuid>,
    Self: From<<Self::EntityType as EntityTrait>::Model>,
{
    type EntityType: EntityTrait + Sync;
    type ColumnType: ColumnTrait + std::fmt::Debug;
    type ActiveModelType: ActiveModelTrait<Entity = Self::EntityType>;
    type CreateModel: Into<Self::ActiveModelType> + Send;
    type UpdateModel: Send + Sync + MergeIntoActiveModel<Self::ActiveModelType>;
    type ListModel: From<Self> + Send + Sync;

    const ID_COLUMN: Self::ColumnType;
    const RESOURCE_NAME_SINGULAR: &str;
    const RESOURCE_NAME_PLURAL: &str;
    const TABLE_NAME: &'static str;
    const RESOURCE_DESCRIPTION: &'static str = "";
    const FULLTEXT_LANGUAGE: &'static str = "english";

    /// Maximum number of items allowed in batch create/update/delete operations.
    /// Override with `#[crudcrate(batch_limit = 500)]` on your struct, or implement
    /// manually for runtime logic (env vars, config, etc.).
    fn batch_limit() -> usize {
        100
    }

    /// Maximum page size for pagination.
    /// Override with `#[crudcrate(max_page_size = 500)]` on your struct, or implement
    /// manually for runtime logic (env vars, config, etc.).
    fn max_page_size() -> u64 {
        1000
    }

    async fn get_all(
        db: &DatabaseConnection,
        condition: &Condition,
        order_column: Self::ColumnType,
        order_direction: Order,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Self::ListModel>, ApiError> {
        let models = Self::EntityType::find()
            .filter(condition.clone())
            .order_by(order_column, order_direction)
            .offset(offset)
            .limit(limit)
            .all(db)
            .await
            .map_err(ApiError::database)?;
        Ok(models
            .into_iter()
            .map(|model| Self::ListModel::from(Self::from(model)))
            .collect())
    }

    async fn get_one(db: &DatabaseConnection, id: Uuid) -> Result<Self, ApiError> {
        let model = Self::EntityType::find_by_id(id)
            .one(db)
            .await
            .map_err(ApiError::database)?
            .ok_or_else(|| {
                ApiError::not_found(Self::RESOURCE_NAME_SINGULAR, Some(id.to_string()))
            })?;
        Ok(Self::from(model))
    }

    async fn create(
        db: &DatabaseConnection,
        create_model: Self::CreateModel,
    ) -> Result<Self, ApiError> {
        use sea_orm::ActiveModelTrait;
        let active_model: Self::ActiveModelType = create_model.into();

        // Use insert and return the model directly
        // This works across all databases unlike last_insert_id for UUIDs
        let model = active_model.insert(db).await.map_err(ApiError::database)?;

        // Convert the model to Self which implements CRUDResource
        // This gives us access to the id field directly
        Ok(Self::from(model))
    }

    async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        update_model: Self::UpdateModel,
    ) -> Result<Self, ApiError> {
        let model = Self::EntityType::find_by_id(id)
            .one(db)
            .await
            .map_err(ApiError::database)?
            .ok_or_else(|| {
                ApiError::not_found(Self::RESOURCE_NAME_SINGULAR, Some(id.to_string()))
            })?;
        let existing: Self::ActiveModelType = model.into_active_model();
        let updated_model = update_model.merge_into_activemodel(existing)?;
        let updated = updated_model.update(db).await.map_err(ApiError::database)?;
        Ok(Self::from(updated))
    }

    async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<Uuid, ApiError> {
        let res = Self::EntityType::delete_by_id(id)
            .exec(db)
            .await
            .map_err(ApiError::database)?;
        match res.rows_affected {
            0 => Err(ApiError::not_found(
                Self::RESOURCE_NAME_SINGULAR,
                Some(id.to_string()),
            )),
            _ => Ok(id),
        }
    }

    async fn delete_many(db: &DatabaseConnection, ids: Vec<Uuid>) -> Result<Vec<Uuid>, ApiError> {
        if ids.len() > Self::batch_limit() {
            return Err(ApiError::bad_request(format!(
                "Batch delete limited to {} items. Received {} items.",
                Self::batch_limit(),
                ids.len()
            )));
        }

        if ids.is_empty() {
            return Ok(vec![]);
        }

        // Pre-query: which IDs actually exist?
        let existing: Vec<UuidIdResult> = Self::EntityType::find()
            .select_only()
            .column_as(Self::ID_COLUMN, "id")
            .filter(Self::ID_COLUMN.is_in(ids.clone()))
            .into_model::<UuidIdResult>()
            .all(db)
            .await
            .map_err(ApiError::database)?;
        let existing_set: std::collections::HashSet<Uuid> =
            existing.into_iter().map(|r| r.id).collect();

        // Delete only existing IDs
        if !existing_set.is_empty() {
            Self::EntityType::delete_many()
                .filter(Self::ID_COLUMN.is_in(existing_set.iter().copied().collect::<Vec<_>>()))
                .exec(db)
                .await
                .map_err(ApiError::database)?;
        }

        // Return only IDs that actually existed (preserving input order)
        Ok(ids
            .into_iter()
            .filter(|id| existing_set.contains(id))
            .collect())
    }

    /// Create multiple entities in a batch.
    ///
    /// Uses a transaction to ensure all-or-nothing semantics: if any insert fails,
    /// the entire batch is rolled back and no entities are created.
    ///
    /// # Arguments
    /// * `db` - The database connection
    /// * `create_models` - A vector of create models to insert
    ///
    /// # Returns
    /// A vector of the created entities
    ///
    /// # Errors
    /// Returns an `ApiError` if any insert fails (entire batch is rolled back)
    async fn create_many(
        db: &DatabaseConnection,
        create_models: Vec<Self::CreateModel>,
    ) -> Result<Vec<Self>, ApiError> {
        use sea_orm::{ActiveModelTrait, TransactionTrait};

        // Security: Limit batch size to prevent DoS attacks
        if create_models.len() > Self::batch_limit() {
            return Err(ApiError::bad_request(format!(
                "Batch create limited to {} items. Received {} items.",
                Self::batch_limit(),
                create_models.len()
            )));
        }

        // Use a transaction for all-or-nothing semantics
        let txn = db.begin().await.map_err(ApiError::database)?;

        let mut results = Vec::with_capacity(create_models.len());
        for create_model in create_models {
            let active_model: Self::ActiveModelType = create_model.into();
            let model = match active_model.insert(&txn).await {
                Ok(m) => m,
                Err(e) => {
                    // Rollback is automatic when txn is dropped
                    return Err(ApiError::database(e));
                }
            };
            results.push(Self::from(model));
        }

        txn.commit().await.map_err(ApiError::database)?;
        Ok(results)
    }

    /// Update multiple entities in a batch.
    ///
    /// Uses a transaction to ensure all-or-nothing semantics: if any update fails,
    /// the entire batch is rolled back and no entities are updated.
    ///
    /// # Arguments
    /// * `db` - The database connection
    /// * `updates` - A vector of (id, update_model) pairs
    ///
    /// # Returns
    /// A vector of the updated entities
    ///
    /// # Errors
    /// Returns an `ApiError` if any update fails (entire batch is rolled back)
    async fn update_many(
        db: &DatabaseConnection,
        updates: Vec<(Uuid, Self::UpdateModel)>,
    ) -> Result<Vec<Self>, ApiError> {
        use sea_orm::TransactionTrait;

        // Security: Limit batch size to prevent DoS attacks
        if updates.len() > Self::batch_limit() {
            return Err(ApiError::bad_request(format!(
                "Batch update limited to {} items. Received {} items.",
                Self::batch_limit(),
                updates.len()
            )));
        }

        // Use a transaction for atomicity
        let txn = db.begin().await.map_err(ApiError::database)?;

        let mut results = Vec::with_capacity(updates.len());
        for (id, update_model) in updates {
            let model = Self::EntityType::find_by_id(id)
                .one(&txn)
                .await
                .map_err(ApiError::database)?
                .ok_or_else(|| {
                    ApiError::not_found(Self::RESOURCE_NAME_SINGULAR, Some(id.to_string()))
                })?;
            let existing: Self::ActiveModelType = model.into_active_model();
            let updated_model = update_model.merge_into_activemodel(existing)?;
            let updated = updated_model
                .update(&txn)
                .await
                .map_err(ApiError::database)?;
            results.push(Self::from(updated));
        }

        txn.commit().await.map_err(ApiError::database)?;
        Ok(results)
    }

    async fn total_count(db: &DatabaseConnection, condition: &Condition) -> u64 {
        let query = Self::EntityType::find().filter(condition.clone());
        match PaginatorTrait::count(query, db).await {
            Ok(count) => count,
            Err(e) => {
                // Log database error internally; return 0 to degrade gracefully
                // Users see pagination with count=0, internal error is logged for debugging
                tracing::warn!(
                    error = %e,
                    table = Self::TABLE_NAME,
                    "Database error in total_count - returning 0"
                );
                0
            }
        }
    }

    #[must_use]
    fn default_index_column() -> Self::ColumnType {
        Self::ID_COLUMN
    }

    #[must_use]
    fn sortable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        vec![("id", Self::ID_COLUMN)]
    }

    #[must_use]
    fn filterable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        vec![("id", Self::ID_COLUMN)]
    }

    /// Check if a specific field is an enum type at runtime.
    /// This is used to determine which fields need special enum handling.
    /// Default implementation returns false.
    #[must_use]
    fn is_enum_field(field_name: &str) -> bool {
        let _ = field_name;
        false
    }

    /// Normalizes an enum value for case-insensitive matching.
    /// This is used for enum types that don't support case-insensitive operations.
    /// Default implementation returns None, indicating no enum normalization is available.
    /// Override this method to provide enum value mapping for specific fields.
    #[must_use]
    fn normalize_enum_value(_field_name: &str, _value: &str) -> Option<String> {
        None
    }

    /// Returns a list of field names that should use LIKE queries (substring matching).
    /// Other string fields will use exact matching.
    /// Default is empty - no fields use LIKE by default.
    #[must_use]
    fn like_filterable_columns() -> Vec<&'static str> {
        vec![]
    }

    /// Returns a list of field names and their column types that should be included in fulltext search.
    /// These fields will be concatenated and searched when the 'q' parameter is used.
    /// Default is empty - no fields are included in fulltext search by default.
    #[must_use]
    fn fulltext_searchable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        vec![]
    }

    /// Returns a list of filterable columns on joined/related entities.
    ///
    /// These columns can be filtered using dot-notation in query parameters:
    /// ```ignore
    /// GET /customers?filter={"vehicles.make":"BMW","vehicles.year_gte":2020}
    /// ```
    ///
    /// Define on join fields using:
    /// ```ignore
    /// #[crudcrate(join(one, all, filterable("make", "year", "color")))]
    /// pub vehicles: Vec<Vehicle>,
    /// ```
    #[must_use]
    fn joined_filterable_columns() -> Vec<crate::JoinedColumnDef> {
        vec![]
    }

    /// Returns a list of sortable columns on joined/related entities.
    ///
    /// These columns can be sorted using dot-notation in query parameters:
    /// ```ignore
    /// GET /customers?sort=["vehicles.year","DESC"]
    /// ```
    ///
    /// Define on join fields using:
    /// ```ignore
    /// #[crudcrate(join(one, all, sortable("year", "mileage")))]
    /// pub vehicles: Vec<Vehicle>,
    /// ```
    #[must_use]
    fn joined_sortable_columns() -> Vec<crate::JoinedColumnDef> {
        vec![]
    }
}
