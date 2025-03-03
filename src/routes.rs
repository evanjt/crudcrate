use crate::filter::{apply_filters, parse_range};
use crate::models::FilterOptions;
use crate::pagination::calculate_content_range;
use crate::sort::generic_sort;
use crate::traits::CRUDResource;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use hyper::HeaderMap;
use sea_orm::{DatabaseConnection, DbErr, SqlErr};
use uuid::Uuid;

// Example: Get all resources.
pub async fn get_all<T>(
    Query(params): Query<FilterOptions>,
    State(db): State<DatabaseConnection>,
) -> Result<(HeaderMap, Json<Vec<T::ApiModel>>), (StatusCode, String)>
where
    T: CRUDResource,
{
    let (offset, limit) = parse_range(params.range.clone());
    let condition = apply_filters(params.filter.clone(), &T::filterable_columns());
    let (order_column, order_direction) = generic_sort(
        params.sort.clone(),
        &T::sortable_columns(),
        T::default_index_column(),
    );

    let items = match T::get_all(
        &db,
        condition.clone(),
        order_column,
        order_direction,
        offset,
        limit,
    )
    .await
    {
        Ok(items) => items,
        Err(err) => return Err((StatusCode::INTERNAL_SERVER_ERROR, err.to_string())),
    };

    let total_count = T::total_count(&db, condition).await;
    let headers = calculate_content_range(offset, limit, total_count, T::RESOURCE_NAME_PLURAL);
    Ok((headers, Json(items)))
}

// Example: Get one resource.
pub async fn get_one<T>(
    State(db): State<DatabaseConnection>,
    Path(id): Path<Uuid>,
) -> Result<Json<T::ApiModel>, (StatusCode, Json<String>)>
where
    T: CRUDResource,
{
    match T::get_one(&db, id).await {
        Ok(item) => Ok(Json(item)),
        Err(DbErr::RecordNotFound(_)) => {
            Err((StatusCode::NOT_FOUND, Json("Not Found".to_string())))
        }
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json("Internal Server Error".to_string()),
        )),
    }
}

// Example: Create one resource.
pub async fn create_one<T>(
    State(db): State<DatabaseConnection>,
    Json(payload): Json<T::CreateModel>,
) -> Result<(StatusCode, Json<T::ApiModel>), (StatusCode, Json<String>)>
where
    T: CRUDResource,
{
    match T::create(&db, payload).await {
        Ok(created_item) => Ok((StatusCode::CREATED, Json(created_item))),
        Err(err) => match err.sql_err() {
            Some(SqlErr::UniqueConstraintViolation(_)) => {
                Err((StatusCode::CONFLICT, Json("Duplicate entry".to_string())))
            }
            Some(_) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json("Error adding object".to_string()),
            )),
            _ => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json("Server error".to_string()),
            )),
        },
    }
}

// Example: Update one resource.
pub async fn update_one<T>(
    State(db): State<DatabaseConnection>,
    Path(id): Path<Uuid>,
    Json(payload): Json<T::UpdateModel>,
) -> Result<Json<T::ApiModel>, (StatusCode, Json<String>)>
where
    T: CRUDResource,
{
    match T::update(&db, id, payload).await {
        Ok(updated_item) => Ok(Json(updated_item)),
        Err(DbErr::RecordNotFound(_)) => {
            Err((StatusCode::NOT_FOUND, Json("Not Found".to_string())))
        }
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json("Error updating item".to_string()),
        )),
    }
}

/// Deletes a single resource by its UUID.
///
/// # Parameters
/// - `State(db)`: The database connection state.
/// - `Path(id)`: The UUID of the resource to be deleted.
///
/// # Returns
/// - `Ok((StatusCode::NO_CONTENT, Json<Uuid>))`: If the resource was successfully deleted.
/// - `Err((StatusCode::INTERNAL_SERVER_ERROR, Json<String>))`: If there was an error deleting the resource.
///
/// # Errors
/// - Returns `StatusCode::INTERNAL_SERVER_ERROR` if there is an error during the deletion process.
pub async fn delete_one<T>(
    State(db): State<DatabaseConnection>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<Uuid>), (StatusCode, Json<String>)>
where
    T: CRUDResource,
{
    match T::delete(&db, id).await {
        Ok(deleted_row) => Ok((StatusCode::NO_CONTENT, Json(deleted_row))),
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json("Error deleting item".to_string()),
        )),
    }
}

/// Example: Delete many resources.
///
/// # Parameters
/// - `State(db)`: The database connection state.
/// - `Json(ids)`: A list of UUIDs to be deleted.
///
/// # Returns
/// - `Ok((StatusCode::NO_CONTENT, Json<Vec<Uuid>))`: If the resources were successfully deleted.
/// - `Err((StatusCode::INTERNAL_SERVER_ERROR, Json<String>))`: If there was an error deleting the resources.
///
/// # Errors
/// - Returns `StatusCode::INTERNAL_SERVER_ERROR` if there is an error during the deletion process.
///
/// # Panics
/// - Panics if the `T` type does not implement the `CRUDResource` trait.
pub async fn delete_many<T>(
    State(db): State<DatabaseConnection>,
    Json(ids): Json<Vec<Uuid>>,
) -> Result<(StatusCode, Json<Vec<Uuid>>), (StatusCode, Json<String>)>
where
    T: CRUDResource,
{
    match T::delete_many(&db, ids.clone()).await {
        Ok(deleted_ids) => Ok((StatusCode::NO_CONTENT, Json(deleted_ids))),
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json("Error deleting items".to_string()),
        )),
    }
}
