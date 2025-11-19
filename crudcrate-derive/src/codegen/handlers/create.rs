// join_generators functionality consolidated into this file to avoid duplicate/stub implementations
use crate::traits::crudresource::structs::CRUDResourceMeta;
use quote::quote;

/// Generate create method implementation
pub fn generate_create_impl(
    crud_meta: &CRUDResourceMeta,
) -> proc_macro2::TokenStream {
    // If operations is specified, use it; otherwise fall back to fn_create or default
    if let Some(ops_path) = &crud_meta.operations {
        quote! {
            async fn create(db: &sea_orm::DatabaseConnection, data: Self::CreateModel) -> Result<Self, sea_orm::DbErr> {
                let ops = #ops_path;
                crudcrate::CRUDOperations::create(&ops, db, data).await
            }
        }
    } else if let Some(fn_path) = &crud_meta.fn_create {
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
