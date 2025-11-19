use crate::traits::crudresource::structs::CRUDResourceMeta;
use quote::quote;

/// Generate update method implementation
pub fn generate_update_impl(
    crud_meta: &CRUDResourceMeta,
) -> proc_macro2::TokenStream {
    // If operations is specified, use it; otherwise fall back to fn_update or default
    if let Some(ops_path) = &crud_meta.operations {
        quote! {
            async fn update(db: &sea_orm::DatabaseConnection, id: uuid::Uuid, data: Self::UpdateModel) -> Result<Self, sea_orm::DbErr> {
                let ops = #ops_path;
                crudcrate::CRUDOperations::update(&ops, db, id, data).await
            }
        }
    } else if let Some(fn_path) = &crud_meta.fn_update {
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
