//! Default CRUD bodies shared by `CRUDResource` and `CRUDOperations`.

use sea_orm::{
    ActiveModelBehavior, ActiveModelTrait, Condition, ConnectionTrait, EntityTrait, IdenStatic,
    IntoActiveModel, Order, QueryFilter, QueryOrder, QuerySelect,
};

use crate::ApiError;
use crate::core::resource_id::{ResourceId, any_key_condition};
use crate::core::traits::{CRUDResource, MergeIntoActiveModel, PrimaryKeyType};

pub(crate) async fn get_all<R, C: ConnectionTrait>(
    db: &C,
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
    // The key is what makes a page deterministic, so every column of it tie-breaks the sort. One
    // column of a composite key does not order the rows, and two pages would then repeat or skip.
    for key_column in R::id_columns() {
        if order_column.as_str() != key_column.as_str() {
            query = query.order_by(key_column, Order::Asc);
        }
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

pub(crate) async fn get_one<R, C: ConnectionTrait>(
    db: &C,
    id: PrimaryKeyType<R>,
) -> Result<R, ApiError>
where
    R: CRUDResource + From<<R::EntityType as EntityTrait>::Model>,
    PrimaryKeyType<R>: ResourceId,
{
    let model = R::EntityType::find_by_id(id.clone())
        .one(db)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found(R::RESOURCE_NAME_SINGULAR, Some(id.render())))?;
    Ok(R::from(model))
}

pub(crate) async fn create<R, C: ConnectionTrait>(
    db: &C,
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

pub(crate) async fn update<R, C: ConnectionTrait>(
    db: &C,
    id: PrimaryKeyType<R>,
    update_model: R::UpdateModel,
) -> Result<R, ApiError>
where
    R: CRUDResource + From<<R::EntityType as EntityTrait>::Model>,
    R::ActiveModelType: ActiveModelTrait + ActiveModelBehavior + Send + Sync,
    <R::EntityType as EntityTrait>::Model: IntoActiveModel<R::ActiveModelType>,
    PrimaryKeyType<R>: ResourceId,
{
    let model = R::EntityType::find_by_id(id.clone())
        .one(db)
        .await
        .map_err(ApiError::database)?
        .ok_or_else(|| ApiError::not_found(R::RESOURCE_NAME_SINGULAR, Some(id.render())))?;
    let existing: R::ActiveModelType = model.into_active_model();
    let merged = update_model.merge_into_activemodel(existing)?;
    let updated = merged.update(db).await.map_err(ApiError::database)?;
    Ok(R::from(updated))
}

pub(crate) async fn delete<R, C: ConnectionTrait>(
    db: &C,
    id: PrimaryKeyType<R>,
) -> Result<PrimaryKeyType<R>, ApiError>
where
    R: CRUDResource,
    PrimaryKeyType<R>: ResourceId,
{
    let res = R::EntityType::delete_by_id(id.clone())
        .exec(db)
        .await
        .map_err(ApiError::database)?;
    match res.rows_affected {
        0 => Err(ApiError::not_found(
            R::RESOURCE_NAME_SINGULAR,
            Some(id.render()),
        )),
        _ => Ok(id),
    }
}

/// Deletes only the ids that exist and echoes them back de-duplicated in input order,
/// so a repeated input id cannot over-report the rows removed.
pub(crate) async fn delete_many<R, C: ConnectionTrait>(
    db: &C,
    ids: Vec<PrimaryKeyType<R>>,
) -> Result<Vec<PrimaryKeyType<R>>, ApiError>
where
    R: CRUDResource,
    PrimaryKeyType<R>: ResourceId,
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
    let key_columns = R::id_columns();
    let mut selection = R::EntityType::find().select_only();
    for key_column in &key_columns {
        selection = selection.column(*key_column);
    }
    let existing: Vec<PrimaryKeyType<R>> = selection
        .filter(any_key_condition(&key_columns, ids.clone()))
        .into_tuple::<PrimaryKeyType<R>>()
        .all(db)
        .await
        .map_err(ApiError::database)?;
    let existing_set: std::collections::HashSet<PrimaryKeyType<R>> = existing.into_iter().collect();
    if !existing_set.is_empty() {
        R::EntityType::delete_many()
            .filter(any_key_condition(
                &key_columns,
                existing_set.iter().cloned(),
            ))
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
