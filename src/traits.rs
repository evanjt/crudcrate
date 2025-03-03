use async_trait::async_trait;
use sea_orm::{
    entity::prelude::*, Condition, DatabaseConnection, EntityTrait, Order, PaginatorTrait,
};
use uuid::Uuid;

#[async_trait]
pub trait CRUDResource: Sized + Send + Sync
where
    Self::EntityType: EntityTrait + Sync,
    Self::ActiveModelType: ActiveModelTrait + ActiveModelBehavior + Send + Sync,
    <Self::EntityType as EntityTrait>::Model: Sync,
    <<Self::EntityType as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType: From<uuid::Uuid>,
    <<Self::EntityType as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType: Into<Uuid>,
    <<Self::EntityType as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType: Into<Uuid>,
{
    type EntityType: EntityTrait + Sync;
    type ColumnType: ColumnTrait + std::fmt::Debug;
    type ModelType: ModelTrait;
    type ActiveModelType: sea_orm::ActiveModelTrait<Entity = Self::EntityType>;
    type ApiModel: From<Self::ModelType>;
    type CreateModel: Into<Self::ActiveModelType> + Send;
    type UpdateModel: Send + Sync;

    const ID_COLUMN: Self::ColumnType;
    const RESOURCE_NAME_SINGULAR: &str;
    const RESOURCE_NAME_PLURAL: &str;

    async fn get_all(
        db: &DatabaseConnection,
        condition: Condition,
        order_column: Self::ColumnType,
        order_direction: Order,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Self::ApiModel>, DbErr>;

    async fn get_one(db: &DatabaseConnection, id: Uuid) -> Result<Self::ApiModel, DbErr>;

    async fn create(
        db: &DatabaseConnection,
        create_model: Self::CreateModel,
    ) -> Result<Self::ApiModel, DbErr> {
        let active_model: Self::ActiveModelType = create_model.into();
        let result = <Self::EntityType as EntityTrait>::insert(active_model)
            .exec(db)
            .await?;
        match Self::get_one(db, result.last_insert_id.into()).await {
            Ok(obj) => Ok(obj),
            Err(_) => Err(DbErr::RecordNotFound(format!(
                "{} not created",
                Self::RESOURCE_NAME_SINGULAR
            ))),
        }
    }
    async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        update_model: Self::UpdateModel,
    ) -> Result<Self::ApiModel, DbErr>;

    async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<Uuid, DbErr> {
        let res = <Self::EntityType as EntityTrait>::delete_by_id(id)
            .exec(db)
            .await?;
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
        let query = <Self::EntityType as EntityTrait>::find().filter(condition);
        PaginatorTrait::count(query, db).await.unwrap()
    }

    #[must_use]
    fn default_index_column() -> Self::ColumnType {
        // Default to the ID column
        Self::ID_COLUMN
    }

    #[must_use]
    fn sortable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        // Default sort at least for the ID column
        vec![("id", Self::ID_COLUMN)]
    }

    #[must_use]
    fn filterable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        vec![("id", Self::ID_COLUMN)]
    }
}
