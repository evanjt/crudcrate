use crate::traits::crudresource::structs::CRUDResourceMeta;
use quote::quote;

/// Generate update method implementation with hook support.
///
/// Hook execution order: pre → body → post
/// - `update::one::pre`: Validation/preparation before update (receives id, &UpdateModel)
/// - `update::one::body`: Replaces default update logic (receives id, UpdateModel, returns Self)
/// - `update::one::post`: Side effects after update (receives &Self)
pub fn generate_update_impl(crud_meta: &CRUDResourceMeta) -> proc_macro2::TokenStream {
    // If operations is specified, use it (takes full control)
    if let Some(ops_path) = &crud_meta.operations {
        return quote! {
            async fn update(db: &sea_orm::DatabaseConnection, id: uuid::Uuid, data: Self::UpdateModel) -> Result<Self, crudcrate::ApiError> {
                let ops = #ops_path;
                crudcrate::CRUDOperations::update(&ops, db, id, data).await
            }
        };
    }

    // Get hooks for update::one
    let hooks = &crud_meta.hooks.update.one;

    // Generate pre hook call
    let pre_hook = hooks.pre.as_ref().map(|fn_path| {
        quote! { #fn_path(db, id, &data).await?; }
    });

    // Generate body - either custom or default
    let body = if let Some(fn_path) = &hooks.body {
        quote! { let result = #fn_path(db, id, data).await?; }
    } else {
        quote! {
            use sea_orm::{EntityTrait, IntoActiveModel, ActiveModelTrait};
            use crudcrate::traits::MergeIntoActiveModel;

            let model = Self::EntityType::find_by_id(id)
                .one(db)
                .await?
                .ok_or_else(|| crudcrate::ApiError::not_found(
                    Self::RESOURCE_NAME_SINGULAR,
                    Some(id.to_string())
                ))?;
            let existing: Self::ActiveModelType = model.into_active_model();
            let updated_model = data.merge_into_activemodel(existing)?;
            let updated = updated_model.update(db).await?;
            let result = Self::from(updated);
        }
    };

    // Generate post hook call
    let post_hook = hooks.post.as_ref().map(|fn_path| {
        quote! { #fn_path(db, &result).await?; }
    });

    quote! {
        async fn update(db: &sea_orm::DatabaseConnection, id: uuid::Uuid, data: Self::UpdateModel) -> Result<Self, crudcrate::ApiError> {
            #pre_hook
            #body
            #post_hook
            Ok(result)
        }
    }
}
