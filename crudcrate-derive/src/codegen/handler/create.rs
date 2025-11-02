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
use quote::quote;
use syn::Type;

/// Generate create method implementation
pub fn generate_create_impl(
    crud_meta: &CRUDResourceMeta,
    _analysis: &EntityFieldAnalysis,
) -> proc_macro2::TokenStream {
    if let Some(fn_path) = &crud_meta.fn_create {
        quote! {
            async fn create(db: &sea_orm::DatabaseConnection, data: Self::CreateModel) -> Result<Self, sea_orm::DbErr> {
                #fn_path(db, data).await
            }
        }
    } else {
        quote! {
            // Default create implementation
            async fn create(db: &sea_orm::DatabaseConnection, data: Self::CreateModel) -> Result<Self, sea_orm::DbErr> {
                let active_model: Self::ActiveModelType = data.into();
                let result = Self::EntityType::insert(active_model).exec(db).await?;
                Self::get_one(db, result.last_insert_id.into()).await
            }
        }
    }
}
