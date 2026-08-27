use async_trait::async_trait;
use sea_orm::{
    Condition, DatabaseConnection, EntityTrait, IntoActiveModel, Order, PaginatorTrait,
    entity::prelude::*,
};
use uuid::Uuid;

use crate::ApiError;

/// Helper for extracting UUID PKs in batch queries.
/// Used by `delete_many` to verify which IDs actually existed.
#[derive(Debug, sea_orm::FromQueryResult)]
pub struct UuidIdResult {
    pub id: Uuid,
}

/// The primary-key value type of a [`CRUDResource`]'s entity.
///
/// Resolves to `<<<R::EntityType as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType`,
/// e.g. `uuid::Uuid`, `i32`, or `String` depending on the entity's `#[sea_orm(primary_key)]`
/// column. Used throughout the CRUD stack so identifier-taking methods stay generic over the
/// concrete PK type rather than hardcoding `Uuid`.
pub type PrimaryKeyType<R> =
    <<<R as CRUDResource>::EntityType as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType;

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
    // The PK value type must be usable across the whole CRUD stack: cloned for
    // re-use after a move, compared/hashed for the delete_many existence set,
    // displayed in not-found errors, deserialized from the Axum `Path`, and
    // bound into SeaORM queries via `Into<sea_orm::Value>`. Both `uuid::Uuid`
    // and `i32` (and `String`) satisfy all of these.
    <<Self::EntityType as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType:
        Clone
            + Eq
            + std::hash::Hash
            + std::fmt::Display
            + Send
            + Sync
            + serde::de::DeserializeOwned
            + Into<sea_orm::Value>
            + 'static,
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

    /// When true, the generated read handlers (`get_one`, `get_all`) return HTTP 500
    /// if no `ScopeCondition` middleware is present, so a misrouted mount fails closed
    /// instead of leaking every row. Set via `#[crudcrate(require_scope)]` on the struct.
    ///
    /// Reads only. Write handlers are governed solely by scope presence: 403 when a
    /// `ScopeCondition` is present, allowed when absent. This supports mounting the
    /// scope on safe methods only so writes arrive unscoped deliberately. Confining
    /// writes to a tenant needs hooks or auth middleware, not this flag.
    const REQUIRE_SCOPE: bool = false;

    /// Maximum number of items allowed in batch create/update/delete operations.
    /// Override with `#[crudcrate(batch_limit = 500)]` on your struct, or implement
    /// manually for runtime logic (env vars, config, etc.).
    #[must_use]
    fn batch_limit() -> usize {
        100
    }

    /// Maximum page size for pagination.
    /// Override with `#[crudcrate(max_page_size = 500)]` on your struct, or implement
    /// manually for runtime logic (env vars, config, etc.).
    #[must_use]
    fn max_page_size() -> u64 {
        1000
    }

    /// Per-resource security profile, applied unless overridden by a request-time
    /// `axum::Extension<SecurityProfile>`. See [`crate::SecurityProfile`] for the
    /// preset rationale and override syntax.
    ///
    /// Default is [`SecurityProfile::secure`](crate::SecurityProfile::secure) as of 0.9.0. Consumers upgrading from
    /// 0.8.x can restore pre-0.9.0 behavior with
    /// `#[crudcrate(security_profile = "legacy")]` on each resource, or by applying
    /// `.layer(Extension(SecurityProfile::legacy()))` at the app level.
    #[must_use]
    fn security_profile() -> crate::SecurityProfile {
        crate::SecurityProfile::secure()
    }

    /// Returns whether the named joined field's child entity carries its own
    /// `ScopeFilterable::scope_condition()`, ie. whether a sub-query on that
    /// child is automatically scope-restricted.
    ///
    /// Consulted only when `SecurityProfile::scope_propagation_strict` is `true`
    /// and a request carries a `ScopeCondition` extension. The handler rejects
    /// joined filters whose target field returns `false`, preventing parent-existence
    /// side-channels via unscoped child columns.
    ///
    /// The default implementation returns `false` for every field, the safe choice
    /// when the child's scope status is unknown. The derive macro overrides this to
    /// return `true` for joined fields whose child type has `exclude(scoped)` fields.
    #[must_use]
    fn joined_field_has_scope(_field: &str) -> bool {
        false
    }

    /// List rows matching `condition`, ordered and paginated.
    ///
    /// The primary key is appended as a secondary sort key whenever the requested
    /// sort column is not the primary key itself, so `OFFSET`/`LIMIT` paging over a
    /// column with duplicate values cannot repeat or skip a row between pages.
    async fn get_all(
        db: &DatabaseConnection,
        condition: &Condition,
        order_column: Self::ColumnType,
        order_direction: Order,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Self::ListModel>, ApiError> {
        crate::core::defaults::get_all::<Self>(
            db,
            condition,
            order_column,
            order_direction,
            offset,
            limit,
        )
        .await
    }

    /// Scope-aware variant of `get_all` used by `get_all_handler` when a
    /// `ScopeCondition` extension is present. The parent-level scope is already
    /// merged into `condition` by the handler; this method is responsible for
    /// propagating scope into joined-child batch queries so private children
    /// are filtered at the SQL level.
    ///
    /// The derive macro overrides this to apply each child's
    /// `ScopeFilterable::scope_condition()` to the per-join batch query, and to
    /// recurse via `get_one_scoped` at depth > 1. The default impl delegates to
    /// `get_all`, which is safe for resources without `join(all)` children.
    async fn get_all_scoped(
        db: &DatabaseConnection,
        condition: &Condition,
        order_column: Self::ColumnType,
        order_direction: Order,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Self::ListModel>, ApiError> {
        Self::get_all(db, condition, order_column, order_direction, offset, limit).await
    }

    /// Order parent rows by a column on a joined child entity (dot-notation
    /// sort, e.g. `sort=["vehicles.year","DESC"]`).
    ///
    /// The parent query is ordered by a correlated sub-query over the child
    /// table: `(SELECT MIN(child.<column>) FROM child WHERE child.<fk> =
    /// parent.<pk>)`, so each parent keeps a single row (no JOIN, no
    /// `DISTINCT`) and to-many relations have a well-defined ordering key.
    /// `MIN` is used for both ASC and DESC: ascending lists parents by their
    /// smallest child value first, descending by their largest smallest-value
    /// last. Parents with no children sort as `NULL`.
    ///
    /// The default implementation ignores `join_field`/`column` and falls back
    /// to ordering by [`Self::default_index_column`], so resources without
    /// joined sortable columns (and the trait default) behave exactly like a
    /// plain `get_all`. The derive macro overrides this for resources that
    /// declare `join(..., sortable(...))` on a `Vec<Child>` field.
    ///
    /// # Errors
    /// Returns `ApiError::Database` if the parent query fails.
    async fn get_all_joined_sorted(
        db: &DatabaseConnection,
        condition: &Condition,
        join_field: &str,
        column: &str,
        direction: Order,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Self::ListModel>, ApiError> {
        let _ = (join_field, column);
        Self::get_all(
            db,
            condition,
            Self::default_index_column(),
            direction,
            offset,
            limit,
        )
        .await
    }

    /// Resolve dot-notation joined filters (e.g. `{"vehicles.make":"BMW"}`)
    /// into an augmented `Condition` that the caller passes to `get_all` /
    /// `get_all_scoped` / `total_count`.
    ///
    /// Each [`crate::JoinedFilter`] runs as a sub-query on the child table.
    /// The generated implementation:
    /// - applies the child's static
    ///   [`crate::ScopeFilterable::scope_condition()`] to the sub-query so
    ///   that `#[crudcrate(exclude(scoped))]` privacy flags on the child are
    ///   respected, and
    /// - collects matching parent-FK values and adds
    ///   `Self::ID_COLUMN.is_in(ids)` to the returned condition.
    ///
    /// The default implementation ignores `joined_filters` and returns the
    /// incoming condition unchanged. The derive macro overrides this for
    /// resources that declare `join(..., filterable(...))` on any field.
    ///
    /// # Errors
    /// Returns `ApiError::Database` if any child sub-query fails.
    async fn resolve_joined_filters(
        db: &DatabaseConnection,
        condition: Condition,
        joined_filters: &[crate::JoinedFilter],
    ) -> Result<Condition, ApiError> {
        let _ = db;
        if !joined_filters.is_empty() {
            tracing::debug!(
                count = joined_filters.len(),
                "Default resolve_joined_filters() ignoring joined filters; override this method or use the derive macro to apply them"
            );
        }
        Ok(condition)
    }

    async fn get_one(db: &DatabaseConnection, id: PrimaryKeyType<Self>) -> Result<Self, ApiError> {
        crate::core::defaults::get_one::<Self>(db, id).await
    }

    /// Fetch a single entity by ID with a scope condition applied atomically.
    ///
    /// Unlike calling `get_one()` followed by a separate scope verification query,
    /// this combines the ID and scope condition into a single `WHERE id = ? AND <scope>`
    /// query, preventing TOCTOU races where the entity's scope-relevant columns could
    /// change between two separate queries.
    ///
    /// The derive macro overrides this to include join loading.
    async fn get_one_scoped(
        db: &DatabaseConnection,
        id: PrimaryKeyType<Self>,
        scope: &Condition,
    ) -> Result<Self, ApiError> {
        use sea_orm::QueryFilter;
        let condition = Condition::all()
            .add(Self::ID_COLUMN.eq(id.clone()))
            .add(scope.clone());
        let model = Self::EntityType::find()
            .filter(condition)
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
        crate::core::defaults::create::<Self>(db, create_model).await
    }

    async fn update(
        db: &DatabaseConnection,
        id: PrimaryKeyType<Self>,
        update_model: Self::UpdateModel,
    ) -> Result<Self, ApiError> {
        crate::core::defaults::update::<Self>(db, id, update_model).await
    }

    async fn delete(
        db: &DatabaseConnection,
        id: PrimaryKeyType<Self>,
    ) -> Result<PrimaryKeyType<Self>, ApiError> {
        crate::core::defaults::delete::<Self>(db, id).await
    }

    async fn delete_many(
        db: &DatabaseConnection,
        ids: Vec<PrimaryKeyType<Self>>,
    ) -> Result<Vec<PrimaryKeyType<Self>>, ApiError> {
        crate::core::defaults::delete_many::<Self>(db, ids).await
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
    /// * `updates` - A vector of (id, `update_model`) pairs
    ///
    /// # Returns
    /// A vector of the updated entities
    ///
    /// # Errors
    /// Returns an `ApiError` if any update fails (entire batch is rolled back)
    async fn update_many(
        db: &DatabaseConnection,
        updates: Vec<(PrimaryKeyType<Self>, Self::UpdateModel)>,
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
            let model = Self::EntityType::find_by_id(id.clone())
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

    /// Returns column names excluded from filtering/sorting when a `ScopeCondition` is active.
    ///
    /// Fields marked with `#[crudcrate(exclude(scoped))]` are automatically included.
    /// When a request is scoped (e.g. public/unauthenticated), these columns are stripped
    /// from the filterable and sortable lists to prevent schema probing.
    ///
    /// Default: empty (no columns excluded).
    #[must_use]
    fn scoped_excluded_columns() -> &'static [&'static str] {
        &[]
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
