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

/// Generate update method implementation
pub fn generate_update_impl(
    crud_meta: &CRUDResourceMeta,
    _analysis: &EntityFieldAnalysis,
) -> proc_macro2::TokenStream {
    if let Some(fn_path) = &crud_meta.fn_update {
        quote! {
            async fn update(db: &sea_orm::DatabaseConnection, id: uuid::Uuid, data: Self::UpdateModel) -> Result<Self, sea_orm::DbErr> {
                #fn_path(db, id, data).await
            }
        }
    } else {
        quote! {
            // Default update implementation
            async fn update(db: &sea_orm::DatabaseConnection, id: uuid::Uuid, data: Self::UpdateModel) -> Result<Self, sea_orm::DbErr> {
                use sea_orm::{EntityTrait, IntoActiveModel, ActiveModelTrait};
                use crudcrate::traits::MergeIntoActiveModel;

                let model = Self::EntityType::find_by_id(id)
                    .one(db)
                    .await?
                    .ok_or(sea_orm::DbErr::RecordNotFound(format!(
                        "{} not found",
                        Self::RESOURCE_NAME_SINGULAR
                    )))?;
                let existing: Self::ActiveModelType = model.into_active_model();
                let updated_model = data.merge_into_activemodel(existing)?;
                let updated = updated_model.update(db).await?;
                Ok(Self::from(updated))
            }
        }
    }
}
