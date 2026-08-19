#[macro_export]
macro_rules! crud_handlers {
    // Version with scoped models for auth-aware field visibility
    ($resource:ty, $update_model:ty, $create_model:ty, $list_model:ty, $response_model:ty, $scoped_list:ty, $scoped_response:ty) => {
        crudcrate::crud_handlers_impl!(
            $resource,
            $update_model,
            $create_model,
            $list_model,
            $response_model,
            $scoped_list,
            $scoped_response
        );
    };

    // Standard version with ListModel and ResponseModel (scoped = same as regular)
    ($resource:ty, $update_model:ty, $create_model:ty, $list_model:ty, $response_model:ty) => {
        crudcrate::crud_handlers_impl!(
            $resource,
            $update_model,
            $create_model,
            $list_model,
            $response_model,
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
            $resource,
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
            $resource,
            $resource,
            $resource
        );
    };
}

#[macro_export]
macro_rules! crud_handlers_impl {
    ($resource:ty, $update_model:ty, $create_model:ty, $list_model:ty, $response_model:ty, $scoped_list:ty, $scoped_response:ty) => {
        use crudcrate::filter::{apply_filters, parse_pagination};
        use crudcrate::models::FilterOptions;
        use crudcrate::pagination::calculate_content_range;
        use crudcrate::sort::parse_sorting;

        use axum::{
            extract::{Path, Query, State},
            http::StatusCode,
            Json,
        };

        use axum::http::HeaderMap;
        use sea_orm::{DbErr, SqlErr};

        // utoipa's `axum_extras` feature parses handler parameter types to infer
        // OpenAPI params/bodies. A `:ty` macro fragment nested inside a path's
        // generic arguments (`crudcrate::PrimaryKeyType<$resource>`) reaches the
        // utoipa proc-macro as an opaque nonterminal it cannot descend into,
        // surfacing as a spurious "expected expression, found `let`" parse error
        // in any downstream consumer that enables `axum_extras`. Aliasing the
        // primary-key type to a plain path name lets the generated handler
        // signatures reference it without exposing the fragment to utoipa.
        #[allow(dead_code)]
        type CrudPrimaryKey = crudcrate::PrimaryKeyType<$resource>;


        #[utoipa::path(
            get,
            path = "/{id}",
            // Declared explicitly (String) so utoipa's `axum_extras` does not infer the
            // param schema from `Path<CrudPrimaryKey>`, which would require the primary-key
            // type to implement `ToSchema`/`PartialSchema`. Mirrors `BatchUpdateRequest`'s
            // `#[schema(value_type = String)]`. The path value is always a stringified id.
            params(("id" = String, Path, description = "Resource identifier")),
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
            axum::extract::Path(id): axum::extract::Path<CrudPrimaryKey>,
            scope: Option<axum::Extension<crudcrate::ScopeCondition>>,
        ) -> Result<axum::response::Response, crudcrate::ApiError> {
            use axum::response::IntoResponse;

            // require_scope: if scope middleware is required but not present, return 500
            if <$resource as crudcrate::traits::CRUDResource>::REQUIRE_SCOPE && scope.is_none() {
                return Err(crudcrate::ApiError::internal(
                    "Scope middleware required for this resource but not configured",
                    Some("require_scope check failed: ScopeCondition extension not found in request".into()),
                ));
            }

            if let Some(axum::Extension(crudcrate::ScopeCondition { condition: extra })) = scope {
                // Atomic scoped fetch: ID + scope condition in a single query.
                // Prevents TOCTOU race where scope-relevant columns could change
                // between a fetch and a separate verification query.
                // get_one_scoped already returns NotFound (404) when the scope condition
                // excludes the row, so existence stays masked. Propagate the real error
                // instead of rewriting everything to 404; genuine DB/internal faults
                // must surface as 500 (and be logged), not masquerade as a missing row.
                let result = <$resource as crudcrate::traits::CRUDResource>::get_one_scoped(&db, id, &extra)
                    .await?;

                let response: $response_model = result.into();
                let scoped: $scoped_response = response.into();
                Ok(axum::Json(scoped).into_response())
            } else {
                let result = <$resource as crudcrate::traits::CRUDResource>::get_one(&db, id)
                    .await
                    .map_err(crudcrate::ApiError::from)?;
                let response: $response_model = result.into();
                Ok(axum::Json(response).into_response())
            }
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
            scope: Option<axum::Extension<crudcrate::ScopeCondition>>,
            profile_ext: Option<axum::Extension<crudcrate::SecurityProfile>>,
        ) -> Result<axum::response::Response, crudcrate::ApiError> {
            use axum::response::IntoResponse;

            // require_scope: if scope middleware is required but not present, return 500
            if <$resource as crudcrate::traits::CRUDResource>::REQUIRE_SCOPE && scope.is_none() {
                return Err(crudcrate::ApiError::internal(
                    "Scope middleware required for this resource but not configured",
                    Some("require_scope check failed: ScopeCondition extension not found in request".into()),
                ));
            }

            let profile = crudcrate::profile::resolve(
                profile_ext,
                <$resource as crudcrate::traits::CRUDResource>::security_profile,
            );

            // Strict filter parsing: reject malformed filter JSON before falling through
            // to the lenient parser (which would silently return an unfiltered result).
            if profile.strict_filter_parsing
                && let Some(filter_str) = &params.filter
                && serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(filter_str).is_err()
            {
                return Err(crudcrate::ApiError::bad_request(
                    "Invalid JSON in filter parameter",
                ));
            }

            let (offset, limit) = crudcrate::filter::parse_pagination(&params);
            // `std::cmp::min` rather than `.min()`: Sea-Query's blanket `ExprTrait` impl
            // covers every type, so the method call is ambiguous wherever it is in scope.
            let limit = std::cmp::min(
                limit,
                <$resource as crudcrate::traits::CRUDResource>::max_page_size(),
            );

            let is_scoped = scope.is_some();

            // When scoped, drop excluded columns from the filterable/sortable lists
            // (and from joined sorts, below) so unauthenticated callers can't probe
            // them. Unscoped requests get the full lists.
            let scoped_excluded: &[&str] = if is_scoped {
                <$resource as crudcrate::traits::CRUDResource>::scoped_excluded_columns()
            } else {
                &[]
            };
            let filterable_columns: Vec<_> = <$resource as CRUDResource>::filterable_columns()
                .into_iter()
                .filter(|(name, _)| !scoped_excluded.contains(name))
                .collect();
            let sortable_columns: Vec<_> = <$resource as crudcrate::traits::CRUDResource>::sortable_columns()
                .into_iter()
                .filter(|(name, _)| !scoped_excluded.contains(name))
                .collect();

            let parsed_filters = crudcrate::apply_filters_with_joins::<$resource>(
                params.filter.clone(),
                &filterable_columns,
                db.get_database_backend()
            )?;

            let sort_config = crudcrate::parse_sorting_with_joins::<$resource, _>(
                &params,
                &sortable_columns,
                <$resource as crudcrate::traits::CRUDResource>::default_index_column(),
                scoped_excluded,
            );

            let mut condition = parsed_filters.main_condition;

            let scope_was_present = scope.is_some();
            if let Some(axum::Extension(crudcrate::ScopeCondition { condition: extra })) = scope {
                condition = condition.add(extra);
            };

            // Strict scope propagation: reject joined filters targeting child entities
            // that don't carry their own scope_condition. Without scope on the child,
            // the sub-query runs unrestricted and parent existence leaks via the
            // result's cardinality even when the parent scope filters the final rows.
            if profile.scope_propagation_strict
                && scope_was_present
                && !parsed_filters.joined_filters.is_empty()
            {
                for jf in &parsed_filters.joined_filters {
                    if !<$resource as crudcrate::traits::CRUDResource>::joined_field_has_scope(&jf.join_field) {
                        return Err(crudcrate::ApiError::bad_request(format!(
                            "Joined filter on '{}' not allowed under strict scope: child entity has no scope_condition",
                            jf.join_field,
                        )));
                    }
                }
            }

            // Same guard for joined sorts: ordering parent rows by a column on an
            // unscoped child leaks child existence through the row order, so reject it
            // under strict scope just as the joined-filter path above does.
            if let crudcrate::SortConfig::Joined { join_field, .. } = &sort_config {
                if profile.scope_propagation_strict
                    && scope_was_present
                    && !<$resource as crudcrate::traits::CRUDResource>::joined_field_has_scope(join_field)
                {
                    return Err(crudcrate::ApiError::bad_request(format!(
                        "Joined sort on '{join_field}' not allowed under strict scope: child entity has no scope_condition",
                    )));
                }
            }

            // Resolve dot-notation joined filters (e.g. {"vehicles.make":"BMW"})
            // into additional `Self::ID_COLUMN.is_in(...)` clauses on the main
            // condition. The derive macro's override runs a sub-query per
            // filter with the child's scope_condition applied. Default impl
            // (no derive override) returns the condition unchanged.
            let condition = <$resource as crudcrate::traits::CRUDResource>::resolve_joined_filters(
                &db,
                condition,
                &parsed_filters.joined_filters,
            ).await?;

            let items = match &sort_config {
                crudcrate::SortConfig::Column { column, direction } => {
                    let order_column = *column;
                    let order_direction = direction.clone();
                    if is_scoped {
                        <$resource as crudcrate::traits::CRUDResource>::get_all_scoped(&db, &condition, order_column, order_direction, offset, limit)
                            .await
                            .map_err(crudcrate::ApiError::from)?
                    } else {
                        <$resource as crudcrate::traits::CRUDResource>::get_all(&db, &condition, order_column, order_direction, offset, limit)
                            .await
                            .map_err(crudcrate::ApiError::from)?
                    }
                }
                crudcrate::SortConfig::Joined { join_field, column, direction } => {
                    // Joined sort orders the parent query by a correlated sub-query
                    // over the child column (see `get_all_joined_sorted`). The scoped
                    // branch does not yet propagate child scope into the ordering
                    // sub-query, so it falls back to the same parent-level ordering
                    // without the scoped child batch loading; the parent rows
                    // themselves remain scope-filtered via `condition`.
                    <$resource as crudcrate::traits::CRUDResource>::get_all_joined_sorted(
                        &db, &condition, join_field, column, direction.clone(), offset, limit,
                    )
                    .await
                    .map_err(crudcrate::ApiError::from)?
                }
            };
            let total_count = <$resource as crudcrate::traits::CRUDResource>::total_count(&db, &condition).await;
            let headers = crudcrate::pagination::calculate_content_range(offset, limit, total_count, <$resource as crudcrate::traits::CRUDResource>::RESOURCE_NAME_PLURAL);

            if is_scoped {
                let scoped: Vec<$scoped_list> = items.into_iter().map(|item| { let converted: $scoped_list = item.into(); converted }).collect();
                Ok((headers, axum::Json(scoped)).into_response())
            } else {
                Ok((headers, axum::Json(items)).into_response())
            }
        }


        #[utoipa::path(
            delete,
            path = "/{id}",
            // Declared explicitly (String) so utoipa's `axum_extras` does not infer the
            // param schema from `Path<CrudPrimaryKey>`, which would require the primary-key
            // type to implement `ToSchema`/`PartialSchema`. Mirrors `BatchUpdateRequest`'s
            // `#[schema(value_type = String)]`. The path value is always a stringified id.
            params(("id" = String, Path, description = "Resource identifier")),
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
            scope: Option<axum::Extension<crudcrate::ScopeCondition>>,
            path: axum::extract::Path<CrudPrimaryKey>,
        ) -> Result<axum::http::StatusCode, crudcrate::ApiError> {
            if scope.is_some() {
                return Err(crudcrate::ApiError::forbidden("Write access denied in scoped context"));
            }
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
            scope: Option<axum::Extension<crudcrate::ScopeCondition>>,
            json: axum::Json<$create_model>,
        ) -> Result<(axum::http::StatusCode, axum::Json<$response_model>), crudcrate::ApiError> {
            if scope.is_some() {
                return Err(crudcrate::ApiError::forbidden("Write access denied in scoped context"));
            }
            <$resource as crudcrate::traits::CRUDResource>::create(&state.0, json.0)
                .await
                .map(|res| (axum::http::StatusCode::CREATED, axum::Json(res.into())))
                .map_err(crudcrate::ApiError::from)
        }

        #[utoipa::path(
            delete,
            path = "/batch",
            params(crudcrate::BatchOptions),
            // Explicit so utoipa does not infer the body from `Json<Vec<CrudPrimaryKey>>`,
            // which would require the primary-key type to implement `ToSchema`. Ids are
            // accepted as their string form.
            request_body = Vec<String>,
            responses(
                (status = axum::http::StatusCode::OK, description = "Resources deleted successfully", body = [String]),
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
            scope: Option<axum::Extension<crudcrate::ScopeCondition>>,
            profile_ext: Option<axum::Extension<crudcrate::SecurityProfile>>,
            axum::extract::Query(options): axum::extract::Query<crudcrate::BatchOptions>,
            json: axum::Json<Vec<CrudPrimaryKey>>,
        ) -> axum::response::Response {
            use axum::response::IntoResponse;

            if scope.is_some() {
                return crudcrate::ApiError::forbidden("Write access denied in scoped context").into_response();
            }

            let profile = crudcrate::profile::resolve(
                profile_ext,
                <$resource as crudcrate::traits::CRUDResource>::security_profile,
            );

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
                let mut result: crudcrate::BatchResult<crudcrate::PrimaryKeyType<$resource>> = crudcrate::BatchResult::new();

                for (index, id) in ids.into_iter().enumerate() {
                    match <$resource as crudcrate::traits::CRUDResource>::delete(&state.0, id).await {
                        // Use the deleted id returned by `delete` rather than the moved
                        // `id`; the PK value type may be non-`Copy` (e.g. a `String` PK).
                        Ok(deleted) => result.add_success(deleted),
                        Err(e) => result.add_failure(index, e.to_string()),
                    }
                }

                let status = if result.all_failed() {
                    axum::http::StatusCode::BAD_REQUEST
                } else if result.is_partial() {
                    axum::http::StatusCode::MULTI_STATUS
                } else {
                    axum::http::StatusCode::OK
                };

                if profile.expose_deleted_ids {
                    (status, axum::Json(result)).into_response()
                } else {
                    // expose_deleted_ids=false must also hide WHICH ids failed: each
                    // per-item not-found error embeds the (missing) UUID, so serializing
                    // `failed` verbatim would be an existence-enumeration oracle, the very
                    // side-channel the non-partial path collapses to `{deleted: count}`.
                    let secure = serde_json::json!({
                        "succeeded_count": result.succeeded.len(),
                        "failed_count": result.failed.len(),
                    });
                    (status, axum::Json(secure)).into_response()
                }
            } else {
                // All-or-nothing mode (default)
                match <$resource as crudcrate::traits::CRUDResource>::delete_many(&state.0, ids).await {
                    Ok(deleted_ids) => {
                        if profile.expose_deleted_ids {
                            (axum::http::StatusCode::OK, axum::Json(deleted_ids)).into_response()
                        } else {
                            let secure = serde_json::json!({"deleted": deleted_ids.len()});
                            (axum::http::StatusCode::OK, axum::Json(secure)).into_response()
                        }
                    }
                    Err(e) => crudcrate::ApiError::from(e).into_response()
                }
            }
        }

        #[utoipa::path(
            put,
            path = "/{id}",
            // Declared explicitly (String) so utoipa's `axum_extras` does not infer the
            // param schema from `Path<CrudPrimaryKey>`, which would require the primary-key
            // type to implement `ToSchema`/`PartialSchema`. Mirrors `BatchUpdateRequest`'s
            // `#[schema(value_type = String)]`. The path value is always a stringified id.
            params(("id" = String, Path, description = "Resource identifier")),
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
            scope: Option<axum::Extension<crudcrate::ScopeCondition>>,
            path: axum::extract::Path<CrudPrimaryKey>,
            json: axum::Json<$update_model>,
        ) -> Result<axum::Json<$response_model>, crudcrate::ApiError> {
            if scope.is_some() {
                return Err(crudcrate::ApiError::forbidden("Write access denied in scoped context"));
            }
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
            scope: Option<axum::Extension<crudcrate::ScopeCondition>>,
            axum::extract::Query(options): axum::extract::Query<crudcrate::BatchOptions>,
            json: axum::Json<Vec<$create_model>>,
        ) -> axum::response::Response {
            use axum::response::IntoResponse;

            if scope.is_some() {
                return crudcrate::ApiError::forbidden("Write access denied in scoped context").into_response();
            }

            let data = json.0;

            // Check batch size limit
            if data.len() > <$resource as crudcrate::traits::CRUDResource>::batch_limit() {
                return crudcrate::ApiError::bad_request(
                    format!("Batch create limited to {} items. Received {} items.",
                        <$resource as crudcrate::traits::CRUDResource>::batch_limit(), data.len())
                ).into_response();
            }

            if options.partial {
                // Partial success mode: process each item individually.
                // Use create_many with a single-item batch (not the single `create`,
                // which on the derive path re-fetches via get_one and applies join
                // loading + read::one::transform). This keeps the partial response the
                // same flat shape as the all-or-nothing path below (which calls
                // create_many) and runs the same create::many hooks for both modes.
                let mut result: crudcrate::BatchResult<$response_model> = crudcrate::BatchResult::new();

                for (index, create_model) in data.into_iter().enumerate() {
                    match <$resource as crudcrate::traits::CRUDResource>::create_many(&state.0, vec![create_model]).await {
                        Ok(mut created) => match created.pop() {
                            Some(item) => result.add_success(item.into()),
                            None => result.add_failure(index, "create produced no row".to_string()),
                        },
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
            #[schema(value_type = String)]
            pub id: crudcrate::PrimaryKeyType<$resource>,
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
            scope: Option<axum::Extension<crudcrate::ScopeCondition>>,
            axum::extract::Query(options): axum::extract::Query<crudcrate::BatchOptions>,
            json: axum::Json<Vec<BatchUpdateRequest>>,
        ) -> axum::response::Response {
            use axum::response::IntoResponse;

            if scope.is_some() {
                return crudcrate::ApiError::forbidden("Write access denied in scoped context").into_response();
            }

            let updates: Vec<(crudcrate::PrimaryKeyType<$resource>, $update_model)> = json.0
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
                .layer(axum::extract::DefaultBodyLimit::max(
                    <$api_struct as crudcrate::traits::CRUDResource>::security_profile()
                        .max_request_body_bytes,
                ))
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
                .layer(axum::extract::DefaultBodyLimit::max(
                    <$api_struct as crudcrate::traits::CRUDResource>::security_profile()
                        .max_request_body_bytes,
                ))
                .with_state(db.clone())
        }
    };
}
