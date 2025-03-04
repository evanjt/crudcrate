// use crate::routes;
// use crate::traits::crudcrate::traits::CRUDResource;
// use aide::axum::{
//     routing::{delete_with, get_with, post_with, put_with},
//     ApiRouter,
// };
// use aide::transform::TransformOperation;
// use axum::{extract::State, http::StatusCode, Json};
// // use http::StatusCode;
// use schemars::JsonSchema;
// use sea_orm::DatabaseConnection;

// macro_rules! wrap_handler {
//     ($func:path, ($($arg:ident : $ty:ty),+)) => {
//         move |$($arg: $ty),+| {
//             $func($($arg),+)
//         }
//     };
// }

// // Documentation function for GET all endpoint.
// pub fn get_all_docs<T: crudcrate::traits::CRUDResource + JsonSchema>(op: TransformOperation) -> TransformOperation {
//     op.description(&format!("Retrieve all {}.", T::RESOURCE_NAME_PLURAL))
//         .response::<200, Json<Vec<T::ApiModel>>>()
//         .description("List of resources.")
// }

// // Documentation function for GET one endpoint.
// pub fn get_one_docs<T: crudcrate::traits::CRUDResource + JsonSchema>(op: TransformOperation) -> TransformOperation {
//     op.description(&format!(
//         "Retrieve a single {} by ID.",
//         T::RESOURCE_NAME_SINGULAR
//     ))
//     .response::<200, Json<T::ApiModel>>()
//     .description("The requested resource.")
//     .response::<404, ()>()
//     .description("Resource not found.")
// }

// // Documentation function for CREATE endpoint.
// pub fn create_one_docs<T: crudcrate::traits::CRUDResource + JsonSchema>(op: TransformOperation) -> TransformOperation {
//     op.description(&format!("Create a new {}.", T::RESOURCE_NAME_SINGULAR))
//         .response::<201, Json<T::ApiModel>>()
//         .description("Resource created successfully.")
// }

// // Documentation function for UPDATE endpoint.
// pub fn update_one_docs<T: crudcrate::traits::CRUDResource + JsonSchema>(op: TransformOperation) -> TransformOperation {
//     op.description(&format!(
//         "Update an existing {} by ID.",
//         T::RESOURCE_NAME_SINGULAR
//     ))
//     .response::<200, Json<T::ApiModel>>()
//     .description("Resource updated successfully.")
//     .response::<404, ()>()
//     .description("Resource not found.")
// }

// // Documentation function for DELETE one endpoint.
// pub fn delete_one_docs<T: crudcrate::traits::CRUDResource + JsonSchema>(op: TransformOperation) -> TransformOperation {
//     op.description(&format!("Delete a {} by ID.", T::RESOURCE_NAME_SINGULAR))
//         .response::<204, ()>()
//         .description("Resource deleted successfully.")
//         .response::<404, ()>()
//         .description("Resource not found.")
// }

// // Documentation function for DELETE many endpoint.
// pub fn delete_many_docs<T: crudcrate::traits::CRUDResource + JsonSchema>(
//     op: TransformOperation,
// ) -> TransformOperation {
//     op.description(&format!("Batch delete {}.", T::RESOURCE_NAME_PLURAL))
//         .response::<204, Json<Vec<String>>>()
//         .description("Resources deleted successfully.")
// }
// async fn create_one_handler<T: crudcrate::traits::CRUDResource>(
//     state: State<DatabaseConnection>,
//     json: Json<T::CreateModel>,
// ) -> Result<(StatusCode, Json<T::ApiModel>), (StatusCode, Json<String>)> {
//     routes::create_one::<T>(state, json).await
// }

// // Build an Aide-based router that mounts all CRUD endpoints along with their docs functions.
// pub fn crud_router<T: crudcrate::traits::CRUDResource + JsonSchema + 'static>(db: DatabaseConnection) -> ApiRouter {
//     ApiRouter::new()
//         .api_route(
//             "/",
//             post_with(
//                 wrap_handler!(routes::create_one::<T>, (state: State<DatabaseConnection>, json: Json<T::CreateModel>)),
//                 create_one_docs::<T>,
//             ),
//         )
//         .with_state(db)
// }

#[macro_export]
macro_rules! crud_handlers {
    ($resource:ty, $update_model:ty, $create_model:ty) => {
        use crudcrate::filter::{apply_filters, parse_range};
        use crudcrate::models::FilterOptions;
        use crudcrate::pagination::calculate_content_range;
        use crudcrate::sort::generic_sort;

        use axum::{
            extract::{Path, Query, State},
            http::StatusCode,
            Json,
        };
        use hyper::HeaderMap;
        use sea_orm::{DbErr, SqlErr};
        use uuid::Uuid;

        #[utoipa::path(
            get,
            path = "/{id}",
            responses(
                (status = 200, description = "The requested resource", body = $resource),
                (status = axum::http::StatusCode::NOT_FOUND, description = "Resource not found")
            )
        )]
        pub async fn get_one_handler(
            State(db): axum::extract::State<sea_orm::DatabaseConnection>,
            Path(id): axum::extract::Path<uuid::Uuid>,
        ) -> Result<axum::Json<$resource>, (axum::http::StatusCode, axum::Json<String>)> {
            match <$resource as crudcrate::traits::CRUDResource>::get_one(&db, id).await {
                Ok(item) => Ok(axum::Json(item)),
                Err(sea_orm::DbErr::RecordNotFound(_)) => {
                    Err((axum::http::StatusCode::NOT_FOUND, axum::Json("Not Found".to_string())))
                }
                Err(_) => Err((axum::http::StatusCode::INTERNAL_SERVER_ERROR, axum::Json("Internal Server Error".to_string()))),
            }
        }

        #[utoipa::path(
            get,
            path = "/",
            responses(
                (status = 200, description = "List of resources", body = [$resource])
            )
        )]
        pub async fn get_all_handler(
            Query(params): axum::extract::Query<crudcrate::models::FilterOptions>,
            State(db): axum::extract::State<sea_orm::DatabaseConnection>,
        ) -> Result<(hyper::HeaderMap, axum::Json<Vec<$resource>>), (axum::http::StatusCode, String)> {
            let (offset, limit) = crudcrate::filter::parse_range(params.range.clone());
            let condition = crudcrate::filter::apply_filters(params.filter.clone(), &<$resource as CRUDResource>::filterable_columns());
            let (order_column, order_direction) = crudcrate::sort::generic_sort(
                params.sort.clone(),
                &<$resource as crudcrate::traits::CRUDResource>::sortable_columns(),
                <$resource as crudcrate::traits::CRUDResource>::default_index_column(),
            );
            let items = match <$resource as crudcrate::traits::CRUDResource>::get_all(&db, condition.clone(), order_column, order_direction, offset, limit).await {
                Ok(items) => items,
                Err(err) => return Err((axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string())),
            };
            let total_count = <$resource as crudcrate::traits::CRUDResource>::total_count(&db, condition).await;
            let headers = crudcrate::pagination::calculate_content_range(offset, limit, total_count, <$resource as crudcrate::traits::CRUDResource>::RESOURCE_NAME_PLURAL);
            Ok((headers, axum::Json(items)))
        }


        #[utoipa::path(
            delete,
            path = "/{id}",
            responses(
                (status = 204, description = "Resource deleted successfully"),
                (status = axum::http::StatusCode::NOT_FOUND, description = "Resource not found")
            )
        )]
        pub async fn delete_one_handler(
            state: axum::extract::State<sea_orm::DatabaseConnection>,
            path: axum::extract::Path<uuid::Uuid>,
        ) -> Result<axum::http::StatusCode, (axum::http::StatusCode, axum::Json<String>)> {
            <$resource as crudcrate::traits::CRUDResource>::delete(&state.0, path.0)
                .await
                .map(|_| axum::http::StatusCode::NO_CONTENT)
                .map_err(|_| {
                    (
                        axum::http::StatusCode::NOT_FOUND,
                        axum::Json("Not Found".to_string()),
                    )
                })
        }

        #[utoipa::path(
            post,
            path = "/",
            request_body = $create_model,
            responses(
                (
                    status = 201, description = "Resource created successfully",
                    body = $resource
                ),
            )
        )]
        pub async fn create_one_handler(
            state: axum::extract::State<sea_orm::DatabaseConnection>,
            json: axum::Json<$create_model>,
        ) -> Result<
            (
                axum::http::StatusCode,
                axum::Json<<$resource as crudcrate::traits::CRUDResource>::ApiModel>,
            ),
            (axum::http::StatusCode, axum::Json<String>),
        > {
            <$resource as crudcrate::traits::CRUDResource>::create(&state.0, json.0)
                .await
                .map(|res| (axum::http::StatusCode::CREATED, axum::Json(res)))
                .map_err(|_| {
                    (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        axum::Json("Internal Server Error".to_string()),
                    )
                })
        }

        #[utoipa::path(
            delete,
            path = "/",
            responses(
                (status = 204, description = "Resources deleted successfully", body = [String])
            )
        )]
        pub async fn delete_many_handler(
            state: axum::extract::State<sea_orm::DatabaseConnection>,
            json: axum::Json<Vec<uuid::Uuid>>,
        ) -> Result<axum::http::StatusCode, (axum::http::StatusCode, axum::Json<String>)> {
            <$resource as crudcrate::traits::CRUDResource>::delete_many(&state.0, json.0)
                .await
                .map(|_| axum::http::StatusCode::NO_CONTENT)
                .map_err(|_| {
                    (
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        axum::Json("Internal Server Error".to_string()),
                    )
                })
        }

        #[utoipa::path(
            put,
            path = "/{id}",
            request_body = $update_model,
            responses(
                (status = 200, description = "Resource updated successfully", body = $resource),
                (status = axum::http::StatusCode::NOT_FOUND, description = "Resource not found")
            )
        )]
        pub async fn update_one_handler(
            state: axum::extract::State<sea_orm::DatabaseConnection>,
            path: axum::extract::Path<uuid::Uuid>,
            json: axum::Json<$update_model>,
        ) -> Result<axum::Json<<$resource as crudcrate::traits::CRUDResource>::ApiModel>, (axum::http::StatusCode, axum::Json<String>)> {
            <$resource as crudcrate::traits::CRUDResource>::update(&state.0, path.0, json.0)
                .await
                .map(axum::Json)
                .map_err(|_| {
                    (
                        axum::http::StatusCode::NOT_FOUND,
                        axum::Json("Not Found".to_string()),
                    )
                })
        }
    };
}
