use crate::codegen::handlers::aggregate::FilterableColumnInfo;
use crate::traits::crudresource::structs::CRUDResourceMeta;
use quote::{format_ident, quote};

/// Generate the full CRUD router implementation (with optional aggregate route).
///
/// Used when `generate_router` is set — generates all CRUD handlers + router.
pub(crate) fn generate_router_impl(
    api_struct_name: &syn::Ident,
    crud_meta: &CRUDResourceMeta,
    filterable_columns: &FilterableColumnInfo,
) -> proc_macro2::TokenStream {
    let create_model_name = format_ident!("{}Create", api_struct_name);
    let update_model_name = format_ident!("{}Update", api_struct_name);
    let list_model_name = format_ident!("{}List", api_struct_name);
    let response_model_name = format_ident!("{}Response", api_struct_name);

    // Generate aggregate code + route if aggregate config is present
    let (aggregate_code, aggregate_route) = if crud_meta.aggregate.is_some() {
        let code = crate::codegen::handlers::aggregate::generate_aggregate_code(
            crud_meta,
            api_struct_name,
            filterable_columns,
        );
        let route = quote! {
            .routes(routes!(aggregate_handler))
        };
        (code, route)
    } else {
        (quote! {}, quote! {})
    };

    quote! {
        // Generate CRUD handlers using the crudcrate macro
        crudcrate::crud_handlers!(#api_struct_name, #update_model_name, #create_model_name, #list_model_name, #response_model_name);

        // Generate aggregate query method + handler (if configured)
        #aggregate_code

        impl #api_struct_name {
            /// Generate router with all CRUD endpoints
            pub fn router(db: &sea_orm::DatabaseConnection) -> utoipa_axum::router::OpenApiRouter
            where
                Self: crudcrate::traits::CRUDResource,
            {
                use utoipa_axum::{router::OpenApiRouter, routes};

                tracing::info!(
                    resource = Self::RESOURCE_NAME_PLURAL,
                    table = Self::TABLE_NAME,
                    batch_limit = Self::batch_limit(),
                    max_page_size = Self::max_page_size(),
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
                    #aggregate_route
                    .with_state(db.clone())
            }
        }
    }
}

/// Generate an aggregate-only router (no CRUD endpoints).
///
/// Used when entity has `aggregate(...)` but NOT `generate_router`.
/// Creates a mini-router with just `GET /aggregate`.
pub(crate) fn generate_aggregate_router_impl(
    api_struct_name: &syn::Ident,
    crud_meta: &CRUDResourceMeta,
    filterable_columns: &FilterableColumnInfo,
) -> proc_macro2::TokenStream {
    let aggregate_code = crate::codegen::handlers::aggregate::generate_aggregate_code(
        crud_meta,
        api_struct_name,
        filterable_columns,
    );

    let resource_name = crud_meta.name_plural.as_deref().unwrap_or("resources");

    quote! {
        // Generate aggregate query method + handler
        #aggregate_code

        impl #api_struct_name {
            /// Generate a mini-router with just the aggregate endpoint.
            pub fn aggregate_router(db: &sea_orm::DatabaseConnection) -> utoipa_axum::router::OpenApiRouter {
                use utoipa_axum::{router::OpenApiRouter, routes};

                tracing::info!(
                    resource = #resource_name,
                    "Mounting aggregate-only route"
                );

                OpenApiRouter::new()
                    .routes(routes!(aggregate_handler))
                    .with_state(db.clone())
            }
        }
    }
}
