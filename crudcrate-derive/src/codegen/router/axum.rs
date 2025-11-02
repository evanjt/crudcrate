use crate::attribute_parser::{
    field_has_crudcrate_flag, get_crudcrate_bool, get_crudcrate_expr, get_join_config,
};
use crate::field_analyzer::{
    extract_inner_type_for_update, field_is_optional, resolve_target_models,
    resolve_target_models_with_list,
};
// join_generators functionality consolidated into this file to avoid duplicate/stub implementations
use crate::structs::{CRUDResourceMeta, EntityFieldAnalysis};
use convert_case::{Case, Casing};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Type;

pub(crate) fn generate_router_impl(api_struct_name: &syn::Ident) -> proc_macro2::TokenStream {
    let create_model_name = format_ident!("{}Create", api_struct_name);
    let update_model_name = format_ident!("{}Update", api_struct_name);
    let list_model_name = format_ident!("{}List", api_struct_name);
    let response_model_name = format_ident!("{}Response", api_struct_name);

    generate_axum_router(
        api_struct_name,
        &create_model_name,
        &update_model_name,
        &list_model_name,
        &response_model_name,
    )
}

fn generate_axum_router(
    api_struct_name: &syn::Ident,
    create_model_name: &syn::Ident,
    update_model_name: &syn::Ident,
    list_model_name: &syn::Ident,
    response_model_name: &syn::Ident,
) -> proc_macro2::TokenStream {
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

                OpenApiRouter::new()
                    .routes(routes!(get_one_handler))
                    .routes(routes!(get_all_handler))
                    .routes(routes!(create_one_handler))
                    .routes(routes!(update_one_handler))
                    .routes(routes!(delete_one_handler))
                    .routes(routes!(delete_many_handler))
                    .with_state(db.clone())
            }
        }
    }
}
