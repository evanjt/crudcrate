#[macro_export]
macro_rules! crud_handlers {
    // New version with ListModel and ResponseModel
    ($resource:ty, $update_model:ty, $create_model:ty, $list_model:ty, $response_model:ty) => {
        crudcrate::crud_handlers_impl!(
            $resource,
            $update_model,
            $create_model,
            $list_model,
            $response_model
        );
    };

    // Backward compatibility - use Self as ResponseModel
    ($resource:ty, $update_model:ty, $create_model:ty, $list_model:ty) => {
        crudcrate::crud_handlers_impl!(
            $resource,
            $update_model,
            $create_model,
            $list_model,
            $resource
        );
    };

    // Backward compatibility - use Self as ListModel and ResponseModel
    ($resource:ty, $update_model:ty, $create_model:ty) => {
        crudcrate::crud_handlers_impl!(
            $resource,
            $update_model,
            $create_model,
            $resource,
            $resource
        );
    };
}

#[macro_export]
macro_rules! crud_handlers_impl {
    ($resource:ty, $update_model:ty, $create_model:ty, $list_model:ty, $response_model:ty) => {
        use crudcrate::filter::{apply_filters, parse_pagination};
        use crudcrate::models::FilterOptions;
        use crudcrate::pagination::calculate_content_range;
        use crudcrate::sort::parse_sorting;

        use axum::{
            extract::{Path, Query, State},
            http::StatusCode,
            Json,
        };

        use hyper::HeaderMap;
        use sea_orm::{DbErr, SqlErr};


        #[utoipa::path(
            get,
            path = "/{id}",
            responses(
                (status = axum::http::StatusCode::OK, description = "The requested resource", body = $response_model),
                (status = axum::http::StatusCode::NOT_FOUND, description = "Resource not found"),
                (status = axum::http::StatusCode::BAD_REQUEST, description = "Bad request"),
                (status = axum::http::StatusCode::INTERNAL_SERVER_ERROR, description = "Internal Server Error")
            ),
            operation_id = format!("get_one_{}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR),
            summary = format!("Get one {}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR),
            description = format!("Retrieves one {} by its ID.\n\n{}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR, <$resource as CRUDResource>::RESOURCE_DESCRIPTION)
        )]
        pub async fn get_one_handler(
            axum::extract::State(db): axum::extract::State<sea_orm::DatabaseConnection>,
            axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
        ) -> Result<axum::Json<$response_model>, crudcrate::ApiError> {
            <$resource as crudcrate::traits::CRUDResource>::get_one(&db, id)
                .await
                .map(|item| axum::Json(item.into()))
                .map_err(crudcrate::ApiError::from)
        }

        #[utoipa::path(
            get,
            path = "/",
            responses(
                (status = axum::http::StatusCode::OK, description = "List of resources", body = [$list_model]),
                (status = axum::http::StatusCode::INTERNAL_SERVER_ERROR, description = "Internal Server Error")
            ),
            params(crudcrate::models::FilterOptions),
            operation_id = format!("get_all_{}", <$resource as CRUDResource>::RESOURCE_NAME_PLURAL),
            summary = format!("Get all {}", <$resource as CRUDResource>::RESOURCE_NAME_PLURAL),
            description = format!(
                "Retrieves all {}.\n\n{}\n\nAdditional sortable columns: {}.\n\nAdditional filterable columns: {}.",
                <$resource as CRUDResource>::RESOURCE_NAME_PLURAL,
                <$resource as CRUDResource>::RESOURCE_DESCRIPTION,
                <$resource as CRUDResource>::sortable_columns()
                    .iter()
                    .map(|(name, _)| format!("\n- {}", name))
                    .collect::<Vec<String>>()
                    .join(""),
                <$resource as CRUDResource>::filterable_columns()
                    .iter()
                    .map(|(name, _)| format!("\n- {}", name))
                    .collect::<Vec<String>>()
                    .join("")
            )
        )]
        pub async fn get_all_handler(
            axum::extract::Query(params): axum::extract::Query<crudcrate::models::FilterOptions>,
            axum::extract::State(db): axum::extract::State<sea_orm::DatabaseConnection>,
        ) -> Result<(hyper::HeaderMap, axum::Json<Vec<$list_model>>), crudcrate::ApiError> {
            let (offset, limit) = crudcrate::filter::parse_pagination(&params);
            let limit = limit.min(<$resource as crudcrate::traits::CRUDResource>::max_page_size());

            // Use join-aware filter parsing to detect dot-notation filters
            let parsed_filters = crudcrate::apply_filters_with_joins::<$resource>(
                params.filter.clone(),
                &<$resource as CRUDResource>::filterable_columns(),
                db.get_database_backend()
            );

            // Use join-aware sort parsing to detect dot-notation sorts
            let sort_config = crudcrate::parse_sorting_with_joins::<$resource, _>(
                &params,
                &<$resource as crudcrate::traits::CRUDResource>::sortable_columns(),
                <$resource as crudcrate::traits::CRUDResource>::default_index_column(),
            );

            // For now, use the main condition and regular sorting
            // Joined filters/sorts are validated but require a custom read::many::body hook to execute
            // TODO: Add built-in join query support in a future version
            let condition = parsed_filters.main_condition;

            let (order_column, order_direction) = match &sort_config {
                crudcrate::SortConfig::Column { column, direction } => (*column, direction.clone()),
                crudcrate::SortConfig::Joined { direction, .. } => {
                    // Fall back to default column for joined sorts (requires hook for actual implementation)
                    (<$resource as crudcrate::traits::CRUDResource>::default_index_column(), direction.clone())
                }
            };

            let items = <$resource as crudcrate::traits::CRUDResource>::get_all(&db, &condition, order_column, order_direction, offset, limit)
                .await
                .map_err(crudcrate::ApiError::from)?;
            let total_count = <$resource as crudcrate::traits::CRUDResource>::total_count(&db, &condition).await;
            let headers = crudcrate::pagination::calculate_content_range(offset, limit, total_count, <$resource as crudcrate::traits::CRUDResource>::RESOURCE_NAME_PLURAL);
            Ok((headers, axum::Json(items)))
        }


        #[utoipa::path(
            delete,
            path = "/{id}",
            responses(
                (status = axum::http::StatusCode::NO_CONTENT, description = "Resource deleted successfully"),
                (status = axum::http::StatusCode::NOT_FOUND, description = "Resource not found"),
                (status = axum::http::StatusCode::INTERNAL_SERVER_ERROR, description = "Internal Server Error")
            ),
            operation_id = format!("delete_one_{}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR),
            summary = format!("Delete one {}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR),
            description = format!("Deletes one {} by its ID.\n\n{}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR, <$resource as CRUDResource>::RESOURCE_DESCRIPTION)
        )]
        pub async fn delete_one_handler(
            state: axum::extract::State<sea_orm::DatabaseConnection>,
            path: axum::extract::Path<uuid::Uuid>,
        ) -> Result<axum::http::StatusCode, crudcrate::ApiError> {
            <$resource as crudcrate::traits::CRUDResource>::delete(&state.0, path.0)
                .await
                .map(|_| axum::http::StatusCode::NO_CONTENT)
                .map_err(crudcrate::ApiError::from)
        }

        #[utoipa::path(
            post,
            path = "/",
            request_body = $create_model,
            responses(
                (
                    status =  axum::http::StatusCode::CREATED,
                    description = "Resource created successfully",
                    body = $response_model
                ),
                (
                    status = axum::http::StatusCode::CONFLICT,
                    description = "Duplicate record",
                    body = String
                )
            ),
            operation_id = format!("create_one_{}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR),
            summary = format!("Create one {}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR),
            description = format!("Creates a new {}.\n\n{}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR, <$resource as CRUDResource>::RESOURCE_DESCRIPTION)
        )]
        pub async fn create_one_handler(
            state: axum::extract::State<sea_orm::DatabaseConnection>,
            json: axum::Json<$create_model>,
        ) -> Result<(axum::http::StatusCode, axum::Json<$response_model>), crudcrate::ApiError> {
            <$resource as crudcrate::traits::CRUDResource>::create(&state.0, json.0)
                .await
                .map(|res| (axum::http::StatusCode::CREATED, axum::Json(res.into())))
                .map_err(crudcrate::ApiError::from)
        }

        #[utoipa::path(
            delete,
            path = "/batch",
            params(crudcrate::BatchOptions),
            responses(
                (status = axum::http::StatusCode::OK, description = "Resources deleted successfully", body = [uuid::Uuid]),
                (status = 207, description = "Partial success - some items deleted, some failed"),
                (status = axum::http::StatusCode::BAD_REQUEST, description = "Bad request - batch size exceeded", body = String),
                (status = axum::http::StatusCode::INTERNAL_SERVER_ERROR, description = "Internal Server Error", body = String)
            ),
            operation_id = format!("delete_many_{}", <$resource as CRUDResource>::RESOURCE_NAME_PLURAL),
            summary = format!("Delete many {}", <$resource as CRUDResource>::RESOURCE_NAME_PLURAL),
            description = format!("Deletes many {} by their IDs and returns array of deleted UUIDs.\n\nUse `?partial=true` for partial success mode (deletes valid items even if some fail).\n\n{}", <$resource as CRUDResource>::RESOURCE_NAME_PLURAL, <$resource as CRUDResource>::RESOURCE_DESCRIPTION)
        )]
        pub async fn delete_many_handler(
            state: axum::extract::State<sea_orm::DatabaseConnection>,
            axum::extract::Query(options): axum::extract::Query<crudcrate::BatchOptions>,
            json: axum::Json<Vec<uuid::Uuid>>,
        ) -> axum::response::Response {
            use axum::response::IntoResponse;

            let ids = json.0;

            // Check batch size limit
            if ids.len() > <$resource as crudcrate::traits::CRUDResource>::batch_limit() {
                return crudcrate::ApiError::bad_request(
                    format!("Batch delete limited to {} items. Received {} items.",
                        <$resource as crudcrate::traits::CRUDResource>::batch_limit(), ids.len())
                ).into_response();
            }

            if options.partial {
                // Partial success mode: process each item individually
                let mut result: crudcrate::BatchResult<uuid::Uuid> = crudcrate::BatchResult::new();

                for (index, id) in ids.into_iter().enumerate() {
                    match <$resource as crudcrate::traits::CRUDResource>::delete(&state.0, id).await {
                        Ok(_) => result.add_success(id),
                        Err(e) => result.add_failure(index, e.to_string()),
                    }
                }

                // Determine response status
                if result.all_failed() {
                    // All failed - return 400
                    (axum::http::StatusCode::BAD_REQUEST, axum::Json(result)).into_response()
                } else if result.is_partial() {
                    // Some succeeded, some failed - return 207
                    (axum::http::StatusCode::MULTI_STATUS, axum::Json(result)).into_response()
                } else {
                    // All succeeded - return 200
                    (axum::http::StatusCode::OK, axum::Json(result)).into_response()
                }
            } else {
                // All-or-nothing mode (default)
                match <$resource as crudcrate::traits::CRUDResource>::delete_many(&state.0, ids).await {
                    Ok(deleted_ids) => {
                        (axum::http::StatusCode::OK, axum::Json(deleted_ids)).into_response()
                    }
                    Err(e) => crudcrate::ApiError::from(e).into_response()
                }
            }
        }

        #[utoipa::path(
            put,
            path = "/{id}",
            request_body = $update_model,
            responses(
            (status =  axum::http::StatusCode::OK, description = "Resource updated successfully", body = $response_model),
            (status = axum::http::StatusCode::NOT_FOUND, description = "Resource not found"),
            (status =  axum::http::StatusCode::CONFLICT, description = "Duplicate record", body = String)
            ),
            operation_id = format!("update_one_{}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR),
            summary = format!("Update one {}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR),
            description = format!("Updates one {} by its ID.\n\n{}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR, <$resource as CRUDResource>::RESOURCE_DESCRIPTION)
        )]
        pub async fn update_one_handler(
            state: axum::extract::State<sea_orm::DatabaseConnection>,
            path: axum::extract::Path<uuid::Uuid>,
            json: axum::Json<$update_model>,
        ) -> Result<axum::Json<$response_model>, crudcrate::ApiError> {
            <$resource as crudcrate::traits::CRUDResource>::update(&state.0, path.0, json.0)
                .await
                .map(|res| axum::Json(res.into()))
                .map_err(crudcrate::ApiError::from)
        }

        #[utoipa::path(
            post,
            path = "/batch",
            request_body = Vec<$create_model>,
            params(crudcrate::BatchOptions),
            responses(
                (status = axum::http::StatusCode::CREATED, description = "Resources created successfully", body = [$response_model]),
                (status = 207, description = "Partial success - some items created, some failed"),
                (status = axum::http::StatusCode::BAD_REQUEST, description = "Bad request - batch size exceeded or validation failed", body = String),
                (status = axum::http::StatusCode::CONFLICT, description = "Duplicate record", body = String),
                (status = axum::http::StatusCode::INTERNAL_SERVER_ERROR, description = "Internal Server Error", body = String)
            ),
            operation_id = format!("create_many_{}", <$resource as CRUDResource>::RESOURCE_NAME_PLURAL),
            summary = format!("Create many {}", <$resource as CRUDResource>::RESOURCE_NAME_PLURAL),
            description = format!("Creates multiple {} in a batch. Limited to {} items per request.\n\nUse `?partial=true` for partial success mode (commits successful items even if some fail).\n\n{}", <$resource as CRUDResource>::RESOURCE_NAME_PLURAL, <$resource as CRUDResource>::batch_limit(), <$resource as CRUDResource>::RESOURCE_DESCRIPTION)
        )]
        pub async fn create_many_handler(
            state: axum::extract::State<sea_orm::DatabaseConnection>,
            axum::extract::Query(options): axum::extract::Query<crudcrate::BatchOptions>,
            json: axum::Json<Vec<$create_model>>,
        ) -> axum::response::Response {
            use axum::response::IntoResponse;

            let data = json.0;

            // Check batch size limit
            if data.len() > <$resource as crudcrate::traits::CRUDResource>::batch_limit() {
                return crudcrate::ApiError::bad_request(
                    format!("Batch create limited to {} items. Received {} items.",
                        <$resource as crudcrate::traits::CRUDResource>::batch_limit(), data.len())
                ).into_response();
            }

            if options.partial {
                // Partial success mode: process each item individually
                let mut result: crudcrate::BatchResult<$response_model> = crudcrate::BatchResult::new();

                for (index, create_model) in data.into_iter().enumerate() {
                    match <$resource as crudcrate::traits::CRUDResource>::create(&state.0, create_model).await {
                        Ok(created) => result.add_success(created.into()),
                        Err(e) => result.add_failure(index, e.to_string()),
                    }
                }

                // Determine response status
                if result.all_failed() {
                    // All failed - return 400
                    (axum::http::StatusCode::BAD_REQUEST, axum::Json(result)).into_response()
                } else if result.is_partial() {
                    // Some succeeded, some failed - return 207
                    (axum::http::StatusCode::MULTI_STATUS, axum::Json(result)).into_response()
                } else {
                    // All succeeded - return 201
                    (axum::http::StatusCode::CREATED, axum::Json(result)).into_response()
                }
            } else {
                // All-or-nothing mode (default)
                match <$resource as crudcrate::traits::CRUDResource>::create_many(&state.0, data).await {
                    Ok(results) => {
                        let response: Vec<$response_model> = results.into_iter().map(|r| r.into()).collect();
                        (axum::http::StatusCode::CREATED, axum::Json(response)).into_response()
                    }
                    Err(e) => crudcrate::ApiError::from(e).into_response()
                }
            }
        }

        /// Wrapper type for batch update request items.
        /// Each item contains an `id` field and the update fields flattened into the same object.
        #[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
        #[allow(dead_code)]
        pub struct BatchUpdateRequest {
            /// The ID of the resource to update
            pub id: uuid::Uuid,
            /// Additional update fields (flattened)
            #[serde(flatten)]
            pub data: $update_model,
        }

        #[utoipa::path(
            patch,
            path = "/batch",
            request_body = Vec<BatchUpdateRequest>,
            params(crudcrate::BatchOptions),
            responses(
                (status = axum::http::StatusCode::OK, description = "Resources updated successfully", body = [$response_model]),
                (status = 207, description = "Partial success - some items updated, some failed"),
                (status = axum::http::StatusCode::BAD_REQUEST, description = "Bad request - batch size exceeded or validation failed", body = String),
                (status = axum::http::StatusCode::NOT_FOUND, description = "One or more resources not found"),
                (status = axum::http::StatusCode::CONFLICT, description = "Duplicate record", body = String),
                (status = axum::http::StatusCode::INTERNAL_SERVER_ERROR, description = "Internal Server Error", body = String)
            ),
            operation_id = format!("update_many_{}", <$resource as CRUDResource>::RESOURCE_NAME_PLURAL),
            summary = format!("Update many {}", <$resource as CRUDResource>::RESOURCE_NAME_PLURAL),
            description = format!("Updates multiple {} in a batch. Limited to {} items per request.\n\nUse `?partial=true` for partial success mode (commits successful items even if some fail).\n\n{}", <$resource as CRUDResource>::RESOURCE_NAME_PLURAL, <$resource as CRUDResource>::batch_limit(), <$resource as CRUDResource>::RESOURCE_DESCRIPTION)
        )]
        pub async fn update_many_handler(
            state: axum::extract::State<sea_orm::DatabaseConnection>,
            axum::extract::Query(options): axum::extract::Query<crudcrate::BatchOptions>,
            json: axum::Json<Vec<BatchUpdateRequest>>,
        ) -> axum::response::Response {
            use axum::response::IntoResponse;

            let updates: Vec<(uuid::Uuid, $update_model)> = json.0
                .into_iter()
                .map(|item| (item.id, item.data))
                .collect();

            // Check batch size limit
            if updates.len() > <$resource as crudcrate::traits::CRUDResource>::batch_limit() {
                return crudcrate::ApiError::bad_request(
                    format!("Batch update limited to {} items. Received {} items.",
                        <$resource as crudcrate::traits::CRUDResource>::batch_limit(), updates.len())
                ).into_response();
            }

            if options.partial {
                // Partial success mode: process each item individually
                let mut result: crudcrate::BatchResult<$response_model> = crudcrate::BatchResult::new();

                for (index, (id, update_model)) in updates.into_iter().enumerate() {
                    match <$resource as crudcrate::traits::CRUDResource>::update(&state.0, id, update_model).await {
                        Ok(updated) => result.add_success(updated.into()),
                        Err(e) => result.add_failure(index, e.to_string()),
                    }
                }

                // Determine response status
                if result.all_failed() {
                    // All failed - return 400
                    (axum::http::StatusCode::BAD_REQUEST, axum::Json(result)).into_response()
                } else if result.is_partial() {
                    // Some succeeded, some failed - return 207
                    (axum::http::StatusCode::MULTI_STATUS, axum::Json(result)).into_response()
                } else {
                    // All succeeded - return 200
                    (axum::http::StatusCode::OK, axum::Json(result)).into_response()
                }
            } else {
                // All-or-nothing mode (default)
                match <$resource as crudcrate::traits::CRUDResource>::update_many(&state.0, updates).await {
                    Ok(results) => {
                        let response: Vec<$response_model> = results.into_iter().map(|r| r.into()).collect();
                        (axum::http::StatusCode::OK, axum::Json(response)).into_response()
                    }
                    Err(e) => crudcrate::ApiError::from(e).into_response()
                }
            }
        }
    };
}

#[macro_export]
macro_rules! generate_crud_router {
    ($model:ty, $api_struct:ty, $create_model:ty, $update_model:ty) => {
        crudcrate::crud_handlers!($api_struct, $update_model, $create_model);

        pub fn router(db: &sea_orm::DatabaseConnection) -> utoipa_axum::router::OpenApiRouter
        where
            $api_struct: crudcrate::traits::CRUDResource,
        {
            use utoipa_axum::{router::OpenApiRouter, routes};

            tracing::info!(
                resource = <$api_struct as crudcrate::traits::CRUDResource>::RESOURCE_NAME_PLURAL,
                table = <$api_struct as crudcrate::traits::CRUDResource>::TABLE_NAME,
                batch_limit = <$api_struct as crudcrate::traits::CRUDResource>::batch_limit(),
                max_page_size = <$api_struct as crudcrate::traits::CRUDResource>::max_page_size(),
                "Mounting CRUD routes with security defaults: input_sanitization=enabled, sql_parameterization=enabled. See https://crudcrate.evanjt.com/latest/advanced/security.html"
            );

            OpenApiRouter::new()
                .routes(routes!(get_one_handler))
                .routes(routes!(get_all_handler))
                .routes(routes!(create_one_handler))
                .routes(routes!(create_many_handler))
                .routes(routes!(update_one_handler))
                .routes(routes!(update_many_handler))
                .routes(routes!(delete_one_handler))
                .routes(routes!(delete_many_handler))
                .with_state(db.clone())
        }
    };
    ($model:ty, $api_struct:ty, $create_model:ty, $update_model:ty, $($extra_routes:expr),* $(,)?) => {
        crudcrate::crud_handlers!($api_struct, $update_model, $create_model);

        pub fn router(db: &sea_orm::DatabaseConnection) -> utoipa_axum::router::OpenApiRouter
        where
            $api_struct: crudcrate::traits::CRUDResource,
        {
            use utoipa_axum::{router::OpenApiRouter, routes};

            tracing::info!(
                resource = <$api_struct as crudcrate::traits::CRUDResource>::RESOURCE_NAME_PLURAL,
                table = <$api_struct as crudcrate::traits::CRUDResource>::TABLE_NAME,
                batch_limit = <$api_struct as crudcrate::traits::CRUDResource>::batch_limit(),
                max_page_size = <$api_struct as crudcrate::traits::CRUDResource>::max_page_size(),
                "Mounting CRUD routes with security defaults: input_sanitization=enabled, sql_parameterization=enabled. See https://crudcrate.evanjt.com/latest/advanced/security.html"
            );

            OpenApiRouter::new()
                .routes(routes!(get_one_handler))
                .routes(routes!(get_all_handler))
                .routes(routes!(create_one_handler))
                .routes(routes!(create_many_handler))
                .routes(routes!(update_one_handler))
                .routes(routes!(update_many_handler))
                .routes(routes!(delete_one_handler))
                .routes(routes!(delete_many_handler))
                $(
                    .routes($extra_routes)
                )*
                .with_state(db.clone())
        }
    };
}
