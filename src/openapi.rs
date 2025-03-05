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
            ),
            summary = format!("Get one {}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR),
            description = format!("Retrieves one {} by its ID.\n\n{}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR, <$resource as CRUDResource>::RESOURCE_DESCRIPTION)
        )]
        pub async fn get_one_handler(
            axum::extract::State(db): axum::extract::State<sea_orm::DatabaseConnection>,
            axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
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
            ),
            summary = format!("Get all {}", <$resource as CRUDResource>::RESOURCE_NAME_PLURAL),
            description = format!("Retrieves all {}.\n\n{}", <$resource as CRUDResource>::RESOURCE_NAME_PLURAL, <$resource as CRUDResource>::RESOURCE_DESCRIPTION)
        )]
        pub async fn get_all_handler(
            axum::extract::Query(params): axum::extract::Query<crudcrate::models::FilterOptions>,
            axum::extract::State(db): axum::extract::State<sea_orm::DatabaseConnection>,
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
            ),
            summary = format!("Delete one {}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR),
            description = format!("Deletes one {} by its ID.\n\n{}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR, <$resource as CRUDResource>::RESOURCE_DESCRIPTION)
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
            ),
            summary = format!("Create one {}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR),
            description = format!("Creates a new {}.\n\n{}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR, <$resource as CRUDResource>::RESOURCE_DESCRIPTION)
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
            ),
            summary = format!("Delete many {}", <$resource as CRUDResource>::RESOURCE_NAME_PLURAL),
            description = format!("Deletes many {} by their IDs.\n\n{}", <$resource as CRUDResource>::RESOURCE_NAME_PLURAL, <$resource as CRUDResource>::RESOURCE_DESCRIPTION)
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
            ),
            summary = format!("Update one {}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR),
            description = format!("Updates one {} by its ID.\n\n{}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR, <$resource as CRUDResource>::RESOURCE_DESCRIPTION)
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
