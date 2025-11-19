use async_trait::async_trait;
use sea_orm::{
    Condition, DatabaseConnection, EntityTrait, IntoActiveModel, Order, PaginatorTrait, QueryOrder,
    QuerySelect, entity::prelude::*,
};
use uuid::Uuid;

pub trait MergeIntoActiveModel<ActiveModelType> {
    /// Merge this update model into an existing active model
    /// 
    /// # Errors
    /// 
    /// Returns a `DbErr` if the merge operation fails due to data conversion issues.
    fn merge_into_activemodel(self, existing: ActiveModelType) -> Result<ActiveModelType, DbErr>;
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

    async fn get_all(
        db: &DatabaseConnection,
        condition: &Condition,
        order_column: Self::ColumnType,
        order_direction: Order,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Self::ListModel>, DbErr> {
        let models = Self::EntityType::find()
            .filter(condition.clone())
            .order_by(order_column, order_direction)
            .offset(offset)
            .limit(limit)
            .all(db)
            .await?;
        Ok(models.into_iter().map(|model| Self::ListModel::from(Self::from(model))).collect())
    }


    async fn get_one(db: &DatabaseConnection, id: Uuid) -> Result<Self, DbErr> {
        let model =
            Self::EntityType::find_by_id(id)
                .one(db)
                .await?
                .ok_or(DbErr::RecordNotFound(format!(
                    "{} not found",
                    Self::RESOURCE_NAME_SINGULAR
                )))?;
        Ok(Self::from(model))
    }

    async fn create(
        db: &DatabaseConnection,
        create_model: Self::CreateModel,
    ) -> Result<Self, DbErr> {
        use sea_orm::ActiveModelTrait;
        let active_model: Self::ActiveModelType = create_model.into();

        // Use insert and return the model directly
        // This works across all databases unlike last_insert_id for UUIDs
        let model = active_model.insert(db).await?;

        // Convert the model to Self which implements CRUDResource
        // This gives us access to the id field directly
        Ok(Self::from(model))
    }

    async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        update_model: Self::UpdateModel,
    ) -> Result<Self, DbErr> {
        let model =
            Self::EntityType::find_by_id(id)
                .one(db)
                .await?
                .ok_or(DbErr::RecordNotFound(format!(
                    "{} not found",
                    Self::RESOURCE_NAME_PLURAL
                )))?;
        let existing: Self::ActiveModelType = model.into_active_model();
        let updated_model = update_model.merge_into_activemodel(existing)?;
        let updated = updated_model.update(db).await?;
        Ok(Self::from(updated))
    }

    async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<Uuid, DbErr> {
        let res = Self::EntityType::delete_by_id(id).exec(db).await?;
        match res.rows_affected {
            0 => Err(DbErr::RecordNotFound(format!(
                "{} not found",
                Self::RESOURCE_NAME_SINGULAR
            ))),
            _ => Ok(id),
        }
    }

    async fn delete_many(db: &DatabaseConnection, ids: Vec<Uuid>) -> Result<Vec<Uuid>, DbErr> {
        Self::EntityType::delete_many()
            .filter(Self::ID_COLUMN.is_in(ids.clone()))
            .exec(db)
            .await?;
        Ok(ids)
    }

    async fn total_count(db: &DatabaseConnection, condition: &Condition) -> u64 {
        let query = Self::EntityType::find().filter(condition.clone());
        match PaginatorTrait::count(query, db).await {
            Ok(count) => count,
            Err(e) => {
                eprintln!("Database error in total_count: {}", e);
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
}
