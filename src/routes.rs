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


        #[utoipa::path(
            get,
            path = "/{id}",
            responses(
                (status = axum::http::StatusCode::OK, description = "The requested resource", body = $resource),
                (status = axum::http::StatusCode::NOT_FOUND, description = "Resource not found"),
                (status = axum::http::StatusCode::INTERNAL_SERVER_ERROR, description = "Internal Server Error")
            ),
            operation_id = format!("get_one_{}", <$resource as CRUDResource>::RESOURCE_NAME_SINGULAR),
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
                (status = axum::http::StatusCode::OK, description = "List of resources", body = [$resource]),
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
        ) -> Result<(hyper::HeaderMap, axum::Json<Vec<$resource>>), (axum::http::StatusCode, String)> {
            let (offset, limit) = crudcrate::filter::parse_range(params.range.clone());
            let condition = crudcrate::filter::apply_filters::<$resource>(params.filter.clone(), &<$resource as CRUDResource>::filterable_columns());
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
        ) -> Result<axum::http::StatusCode, (axum::http::StatusCode, axum::Json<String>)> {
            <$resource as crudcrate::traits::CRUDResource>::delete(&state.0, path.0)
                .await
                .map(|_| axum::http::StatusCode::NO_CONTENT)
                .map_err(|err| {
                    match err {
                        sea_orm::DbErr::RecordNotFound(_) => (
                            axum::http::StatusCode::NOT_FOUND,
                            axum::Json("Not Found".to_string()),
                        ),
                        _ => (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            axum::Json("Internal Server Error".to_string()),
                        ),
                    }
                })
        }

        #[utoipa::path(
            post,
            path = "/",
            request_body = $create_model,
            responses(
                (
                    status =  axum::http::StatusCode::CREATED,
                    description = "Resource created successfully",
                    body = $resource
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
        ) -> Result<
            (
                axum::http::StatusCode,
                axum::Json<$resource>,
            ),
            (axum::http::StatusCode, axum::Json<String>),
        > {
            <$resource as crudcrate::traits::CRUDResource>::create(&state.0, json.0)
                .await
                .map(|res| (axum::http::StatusCode::CREATED, axum::Json(res)))
                .map_err(|err| {
                    if let Some(sea_orm::SqlErr::UniqueConstraintViolation(detail)) = err.sql_err() {
                        (
                            axum::http::StatusCode::CONFLICT,
                            axum::Json(format!("Conflict: {}", detail)),
                        )
                    } else {
                        (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            axum::Json("Internal Server Error".to_string()),
                        )
                    }
                })
        }

        #[utoipa::path(
            delete,
            path = "/batch",
            responses(
                (status = axum::http::StatusCode::NO_CONTENT, description = "Resources deleted successfully", body = [String]),
                (status = axum::http::StatusCode::INTERNAL_SERVER_ERROR, description = "Internal Server Error", body = String)
            ),
            operation_id = format!("delete_many_{}", <$resource as CRUDResource>::RESOURCE_NAME_PLURAL),
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
            (status =  axum::http::StatusCode::OK, description = "Resource updated successfully", body = $resource),
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
        ) -> Result<axum::Json<$resource>, (axum::http::StatusCode, axum::Json<String>)>{
            <$resource as crudcrate::traits::CRUDResource>::update(&state.0, path.0, json.0)
            .await
            .map(axum::Json)
            .map_err(|err| {
                if let Some(sea_orm::SqlErr::UniqueConstraintViolation(detail)) = err.sql_err() {
                (
                    axum::http::StatusCode::CONFLICT,
                    axum::Json(format!("Conflict: {}", detail)),
                )
                } else {
                (
                    axum::http::StatusCode::NOT_FOUND,
                    axum::Json("Not Found".to_string()),
                )
                }
            })
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

            OpenApiRouter::new()
                .routes(routes!(get_one_handler))
                .routes(routes!(get_all_handler))
                .routes(routes!(create_one_handler))
                .routes(routes!(update_one_handler))
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

            OpenApiRouter::new()
                .routes(routes!(get_one_handler))
                .routes(routes!(get_all_handler))
                .routes(routes!(create_one_handler))
                .routes(routes!(update_one_handler))
                .routes(routes!(delete_one_handler))
                .routes(routes!(delete_many_handler))
                $(
                    .routes($extra_routes)
                )*
                .with_state(db.clone())
        }
    };
}
