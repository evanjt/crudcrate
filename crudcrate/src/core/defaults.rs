//! Default CRUD bodies shared by `CRUDResource` and `CRUDOperations`.

use sea_orm::{
    ActiveModelBehavior, ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    IdenStatic, IntoActiveModel, Order, QueryFilter, QueryOrder, QuerySelect,
};

use crate::ApiError;
use crate::core::traits::{CRUDResource, MergeIntoActiveModel, PrimaryKeyType};

pub(crate) async fn get_all<R>(
    db: &DatabaseConnection,
    condition: &Condition,
    order_column: R::ColumnType,
    order_direction: Order,
    offset: u64,
    limit: u64,
) -> Result<Vec<R::ListModel>, ApiError>
where
    R: CRUDResource + From<<R::EntityType as EntityTrait>::Model>,
{
    let mut query = R::EntityType::find()
        .filter(condition.clone())
        .order_by(order_column, order_direction);
    if order_column.as_str() != R::ID_COLUMN.as_str() {
        query = query.order_by(R::ID_COLUMN, Order::Asc);
    }
    let models = query
        .offset(offset)
        .limit(limit)
        .all(db)
        .await
        .map_err(ApiError::database)?;
    Ok(models
        .into_iter()
        .map(|model| R::ListModel::from(R::from(model)))
        .collect())
}

pub(crate) async fn get_one<R>(
    db: &DatabaseConnection,
    id: PrimaryKeyType<R>,
) -> Result<R, ApiError>
where
    R: CRUDResource + From<<R::EntityType as EntityTrait>::Model>,
    PrimaryKeyType<R>: Clone + std::fmt::Display,
{
    let model = R::EntityType::find_by_id(id.clone())
        .one(db)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found(R::RESOURCE_NAME_SINGULAR, Some(id.to_string())))?;
    Ok(R::from(model))
}

pub(crate) async fn create<R>(
    db: &DatabaseConnection,
    create_model: R::CreateModel,
) -> Result<R, ApiError>
where
    R: CRUDResource + From<<R::EntityType as EntityTrait>::Model>,
    R::ActiveModelType: ActiveModelTrait + ActiveModelBehavior + Send + Sync,
    <R::EntityType as EntityTrait>::Model: IntoActiveModel<R::ActiveModelType>,
{
    let active_model: R::ActiveModelType = create_model.into();
    let model = active_model.insert(db).await.map_err(ApiError::database)?;
    Ok(R::from(model))
}

pub(crate) async fn update<R>(
    db: &DatabaseConnection,
    id: PrimaryKeyType<R>,
    update_model: R::UpdateModel,
) -> Result<R, ApiError>
where
    R: CRUDResource + From<<R::EntityType as EntityTrait>::Model>,
    R::ActiveModelType: ActiveModelTrait + ActiveModelBehavior + Send + Sync,
    <R::EntityType as EntityTrait>::Model: IntoActiveModel<R::ActiveModelType>,
    PrimaryKeyType<R>: Clone + std::fmt::Display,
{
    let model = R::EntityType::find_by_id(id.clone())
        .one(db)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found(R::RESOURCE_NAME_SINGULAR, Some(id.to_string())))?;
    let existing: R::ActiveModelType = model.into_active_model();
    let merged = update_model.merge_into_activemodel(existing)?;
    let updated = merged.update(db).await.map_err(ApiError::database)?;
    Ok(R::from(updated))
}

pub(crate) async fn delete<R>(
    db: &DatabaseConnection,
    id: PrimaryKeyType<R>,
) -> Result<PrimaryKeyType<R>, ApiError>
where
    R: CRUDResource,
    PrimaryKeyType<R>: Clone + std::fmt::Display,
{
    let res = R::EntityType::delete_by_id(id.clone())
        .exec(db)
        .await
        .map_err(ApiError::database)?;
    match res.rows_affected {
        0 => Err(ApiError::not_found(
            R::RESOURCE_NAME_SINGULAR,
            Some(id.to_string()),
        )),
        _ => Ok(id),
    }
}

/// Deletes only the ids that exist and echoes them back de-duplicated in input order,
/// so a repeated input id cannot over-report the rows removed.
pub(crate) async fn delete_many<R>(
    db: &DatabaseConnection,
    ids: Vec<PrimaryKeyType<R>>,
) -> Result<Vec<PrimaryKeyType<R>>, ApiError>
where
    R: CRUDResource,
    PrimaryKeyType<R>: Clone + Eq + std::hash::Hash + Into<sea_orm::Value>,
{
    if ids.len() > R::batch_limit() {
        return Err(ApiError::bad_request(format!(
            "Batch delete limited to {} items. Received {} items.",
            R::batch_limit(),
            ids.len()
        )));
    }
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let existing: Vec<PrimaryKeyType<R>> = R::EntityType::find()
        .select_only()
        .column(R::ID_COLUMN)
        .filter(R::ID_COLUMN.is_in(ids.clone()))
        .into_tuple::<PrimaryKeyType<R>>()
        .all(db)
        .await
        .map_err(ApiError::database)?;
    let existing_set: std::collections::HashSet<PrimaryKeyType<R>> = existing.into_iter().collect();
    if !existing_set.is_empty() {
        R::EntityType::delete_many()
            .filter(R::ID_COLUMN.is_in(existing_set.iter().cloned().collect::<Vec<_>>()))
            .exec(db)
            .await
            .map_err(ApiError::database)?;
    }
    let mut seen = std::collections::HashSet::new();
    Ok(ids
        .into_iter()
        .filter(|id| existing_set.contains(id) && seen.insert(id.clone()))
        .collect())
}
