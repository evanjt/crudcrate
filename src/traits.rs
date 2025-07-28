use async_trait::async_trait;
use sea_orm::{
    Condition, DatabaseConnection, EntityTrait, IntoActiveModel, Order, PaginatorTrait, QueryOrder,
    QuerySelect, entity::prelude::*,
};
use uuid::Uuid;

pub trait MergeIntoActiveModel<ActiveModelType> {
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

    const ID_COLUMN: Self::ColumnType;
    const RESOURCE_NAME_SINGULAR: &str;
    const RESOURCE_NAME_PLURAL: &str;
    const RESOURCE_DESCRIPTION: &'static str = "";

    async fn get_all(
        db: &DatabaseConnection,
        condition: Condition,
        order_column: Self::ColumnType,
        order_direction: Order,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Self>, DbErr> {
        let models = Self::EntityType::find()
            .filter(condition)
            .order_by(order_column, order_direction)
            .offset(offset)
            .limit(limit)
            .all(db)
            .await?;
        Ok(models.into_iter().map(Self::from).collect())
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
        let active_model: Self::ActiveModelType = create_model.into();
        let result = Self::EntityType::insert(active_model).exec(db).await?;
        Self::get_one(db, result.last_insert_id.into()).await
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

    async fn total_count(db: &DatabaseConnection, condition: Condition) -> u64 {
        let query = Self::EntityType::find().filter(condition);
        PaginatorTrait::count(query, db).await.unwrap()
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

    /// Indicates whether enum filtering should be case-sensitive.
    /// Default is false (case-insensitive).
    #[must_use]
    fn enum_case_sensitive() -> bool {
        false
    }
}
