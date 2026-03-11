use crate::traits::crudresource::structs::CRUDResourceMeta;
use quote::{format_ident, quote};

/// Generate the full CRUD router implementation.
///
/// Used when `generate_router` is set — generates all CRUD handlers + router.
pub(crate) fn generate_router_impl(
    api_struct_name: &syn::Ident,
    _crud_meta: &CRUDResourceMeta,
) -> proc_macro2::TokenStream {
    let create_model_name = format_ident!("{}Create", api_struct_name);
    let update_model_name = format_ident!("{}Update", api_struct_name);
    let list_model_name = format_ident!("{}List", api_struct_name);
    let response_model_name = format_ident!("{}Response", api_struct_name);

    quote! {
        // Generate CRUD handlers using the crudcrate macro
        crudcrate::crud_handlers!(#api_struct_name, #update_model_name, #create_model_name, #list_model_name, #response_model_name);

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
                    .with_state(db.clone())
            }
        }
    }
}
