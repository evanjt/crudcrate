use crate::traits::crudresource::structs::{AggregateConfig, CRUDResourceMeta};
use convert_case::{Case, Casing};
use quote::{format_ident, quote};

/// Information about filterable columns for aggregate queries.
///
/// Each entry is (snake_case field name, PascalCase Column ident).
pub type FilterableColumnInfo = Vec<(String, String)>;

/// Generate all aggregate code: the `aggregate_query()` method on the API struct
/// and the `aggregate_handler` function.
///
/// The generated code references `Entity` and `Column` directly (bare idents in the
/// entity module scope), so it does NOT depend on `CRUDResource`.
pub fn generate_aggregate_code(
    crud_meta: &CRUDResourceMeta,
    api_struct_name: &syn::Ident,
    filterable_columns: &FilterableColumnInfo,
) -> proc_macro2::TokenStream {
    let Some(ref agg_config) = crud_meta.aggregate else {
        return quote! {};
    };

    let query_method =
        generate_aggregate_query_method(agg_config, api_struct_name, filterable_columns, crud_meta);
    let pivot_config_method = generate_pivot_config_method(agg_config, api_struct_name);
    let handler = generate_aggregate_handler(agg_config, api_struct_name, crud_meta);

    quote! {
        // Emit a compile error if the `aggregation` feature is not enabled on crudcrate
        crudcrate::_require_aggregation_feature!();

        #query_method
        #pivot_config_method
        #handler
    }
}

/// Generate the `aggregate_query()` method on the API struct.
///
/// This is the core logic, callable programmatically from custom handlers.
fn generate_aggregate_query_method(
    agg_config: &AggregateConfig,
    api_struct_name: &syn::Ident,
    filterable_columns: &FilterableColumnInfo,
    _crud_meta: &CRUDResourceMeta,
) -> proc_macro2::TokenStream {
    let time_col_ident = format_ident!("{}", agg_config.time_column.to_case(Case::Pascal));
    let intervals: Vec<&str> = agg_config.intervals.iter().map(String::as_str).collect();

    // Generate metric SELECT expressions
    let metric_selects = generate_metric_selects(agg_config);

    // Generate group_by column SELECT (using bare Column ident)
    let group_select_chain: Vec<_> = agg_config
        .group_by
        .iter()
        .map(|col| {
            let col_ident = format_ident!("{}", col.to_case(Case::Pascal));
            quote! {
                .column(Column::#col_ident)
            }
        })
        .collect();

    // Generate group_by GROUP BY chain
    let group_group_chain: Vec<_> = agg_config
        .group_by
        .iter()
        .map(|col| {
            let col_name = col.as_str();
            quote! {
                .group_by(sea_orm::sea_query::SimpleExpr::Custom(
                    format!("\"{}\"", #col_name)
                ))
            }
        })
        .collect();

    // Generate inline filter code (no CRUDResource dependency)
    let filter_code = if !filterable_columns.is_empty() {
        let column_entries: Vec<_> = filterable_columns
            .iter()
            .map(|(snake_name, pascal_name)| {
                let col_ident = format_ident!("{}", pascal_name);
                quote! { (#snake_name, Column::#col_ident) }
            })
            .collect();

        quote! {
            if let Some(ref filter_str) = params.filter {
                let columns: Vec<(&str, Column)> = vec![#(#column_entries),*];
                let condition = crudcrate::aggregation::apply_aggregate_filters(
                    Some(filter_str.clone()),
                    &columns,
                    db.get_database_backend(),
                );
                query = query.filter(condition);
            }
        }
    } else {
        quote! {}
    };

    quote! {
        impl #api_struct_name {
            /// Execute the aggregate query and return flat JSON rows.
            ///
            /// Each row contains: `bucket`, group-by columns, `avg_X`/`min_X`/`max_X` per metric, `count`.
            /// Call this programmatically from custom handlers to reshape results.
            pub async fn aggregate_query(
                db: &sea_orm::DatabaseConnection,
                params: &crudcrate::aggregation::AggregateParams,
            ) -> Result<Vec<serde_json::Value>, crudcrate::ApiError> {
                use sea_orm::{EntityTrait, QuerySelect, QueryFilter, QueryOrder, ColumnTrait};

                // Validate interval against allowlist
                let allowed_intervals: &[&str] = &[#(#intervals),*];
                let matched = crudcrate::aggregation::validate_interval(&params.interval, allowed_intervals)?;

                // Parse the validated interval
                let interval = crudcrate::sea_orm_timescale::types::Interval::parse(matched)
                    .map_err(|e| crudcrate::ApiError::bad_request(e.to_string()))?;

                // Build time_bucket expression (using bare Column ident)
                // Use timezone-aware bucketing when timezone param is provided
                let bucket = if let Some(ref tz) = params.timezone {
                    crudcrate::aggregation::validate_timezone(tz)?;
                    crudcrate::sea_orm_timescale::functions::time_bucket_tz(
                        &interval,
                        Column::#time_col_ident,
                        tz,
                    )
                } else {
                    crudcrate::sea_orm_timescale::functions::time_bucket(
                        &interval,
                        Column::#time_col_ident,
                    )
                };

                // Build aggregate query (using bare Entity ident)
                let mut query = Entity::find()
                    .select_only()
                    .column_as(bucket.clone(), "bucket")
                    #(#group_select_chain)*
                    #(#metric_selects)*
                    .column_as(
                        sea_orm::sea_query::SimpleExpr::Custom("COUNT(*)".to_string()),
                        "count"
                    )
                    .group_by(bucket)
                    #(#group_group_chain)*
                    .order_by_asc(sea_orm::sea_query::SimpleExpr::Custom("\"bucket\"".to_string()));

                // Apply time range filters
                if let Some(ref start) = params.start {
                    let start_dt = crudcrate::aggregation::parse_datetime(start)?;
                    query = query.filter(Column::#time_col_ident.gte(start_dt));
                }
                if let Some(ref end) = params.end {
                    let end_dt = crudcrate::aggregation::parse_datetime(end)?;
                    query = query.filter(Column::#time_col_ident.lt(end_dt));
                }

                // Apply additional filters
                #filter_code

                // Execute and return
                query
                    .into_json()
                    .all(db)
                    .await
                    .map_err(crudcrate::ApiError::from)
            }
        }
    }
}

/// Generate the `pivot_config()` method on the API struct.
fn generate_pivot_config_method(
    agg_config: &AggregateConfig,
    api_struct_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    let metrics: Vec<&str> = agg_config.metrics.iter().map(String::as_str).collect();
    let aggregates: Vec<&str> = agg_config.aggregates.iter().map(String::as_str).collect();
    let group_by: Vec<&str> = agg_config.group_by.iter().map(String::as_str).collect();

    quote! {
        impl #api_struct_name {
            /// Build a PivotConfig for the given interval.
            pub fn pivot_config(interval: &str) -> crudcrate::aggregation::PivotConfig {
                crudcrate::aggregation::PivotConfig {
                    metrics: vec![#(#metrics.to_string()),*],
                    aggregates: vec![#(#aggregates.to_string()),*],
                    group_by: vec![#(#group_by.to_string()),*],
                    resolution: interval.to_string(),
                }
            }
        }
    }
}

/// Generate the thin `aggregate_handler` function that wraps `aggregate_query()`.
fn generate_aggregate_handler(
    agg_config: &AggregateConfig,
    api_struct_name: &syn::Ident,
    crud_meta: &CRUDResourceMeta,
) -> proc_macro2::TokenStream {
    let intervals: Vec<&str> = agg_config.intervals.iter().map(String::as_str).collect();
    let resource_name = crud_meta.name_plural.as_deref().unwrap_or("resources");

    // Generate hooks
    let hooks = &crud_meta.hooks.aggregate.one;

    let pre_hook = hooks.pre.as_ref().map(|fn_path| {
        quote! { #fn_path(&db, &params).await?; }
    });

    let transform_hook = hooks.transform.as_ref().map(|fn_path| {
        quote! { let result = #fn_path(&db, result).await?; }
    });

    quote! {
        #[utoipa::path(
            get,
            path = "/aggregate",
            params(crudcrate::aggregation::AggregateParams),
            responses(
                (status = axum::http::StatusCode::OK, description = "Aggregated time-series data", body = crudcrate::aggregation::AggregateResponse),
                (status = axum::http::StatusCode::BAD_REQUEST, description = "Invalid interval or parameters"),
                (status = axum::http::StatusCode::INTERNAL_SERVER_ERROR, description = "Internal Server Error")
            ),
            operation_id = format!("aggregate_{}", #resource_name),
            summary = format!("Aggregate {}", #resource_name),
            description = format!(
                "Returns aggregated time-series data for {} with configurable time buckets.\n\nAllowed intervals: {}",
                #resource_name,
                [#(#intervals),*].join(", ")
            )
        )]
        pub async fn aggregate_handler(
            axum::extract::Query(params): axum::extract::Query<crudcrate::aggregation::AggregateParams>,
            axum::extract::State(db): axum::extract::State<sea_orm::DatabaseConnection>,
        ) -> Result<axum::Json<serde_json::Value>, crudcrate::ApiError> {
            #pre_hook

            let flat_results = #api_struct_name::aggregate_query(&db, &params).await?;
            let config = #api_struct_name::pivot_config(&params.interval);
            let pivoted = crudcrate::aggregation::pivot_to_columnar(
                &flat_results, &config, params.start.as_deref(), params.end.as_deref(),
            );
            let mut result = serde_json::to_value(pivoted)
                .map_err(|e| crudcrate::ApiError::internal(e.to_string(), None))?;

            #transform_hook

            Ok(axum::Json(result))
        }
    }
}

/// Generate metric SELECT expressions based on configured aggregate functions.
fn generate_metric_selects(config: &AggregateConfig) -> Vec<proc_macro2::TokenStream> {
    let mut selects = Vec::new();
    let time_col_name = config.time_column.as_str();

    for metric in &config.metrics {
        let metric_name = metric.as_str();

        for agg in &config.aggregates {
            let alias = format!("{agg}_{metric}");
            let select = match agg.as_str() {
                "avg" => quote! {
                    .column_as(
                        sea_orm::sea_query::SimpleExpr::Custom(format!("AVG(\"{}\")", #metric_name)),
                        #alias
                    )
                },
                "min" => quote! {
                    .column_as(
                        sea_orm::sea_query::SimpleExpr::Custom(format!("MIN(\"{}\")", #metric_name)),
                        #alias
                    )
                },
                "max" => quote! {
                    .column_as(
                        sea_orm::sea_query::SimpleExpr::Custom(format!("MAX(\"{}\")", #metric_name)),
                        #alias
                    )
                },
                "first" => quote! {
                    .column_as(
                        sea_orm::sea_query::SimpleExpr::Custom(
                            format!("first(\"{}\", \"{}\")", #metric_name, #time_col_name)
                        ),
                        #alias
                    )
                },
                "last" => quote! {
                    .column_as(
                        sea_orm::sea_query::SimpleExpr::Custom(
                            format!("last(\"{}\", \"{}\")", #metric_name, #time_col_name)
                        ),
                        #alias
                    )
                },
                _ => continue,
            };
            selects.push(select);
        }
    }

    selects
}
