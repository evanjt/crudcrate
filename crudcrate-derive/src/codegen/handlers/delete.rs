// join_generators functionality consolidated into this file to avoid duplicate/stub implementations
use crate::structs::CRUDResourceMeta;
use quote::quote;

/// Generate delete method implementation
pub fn generate_delete_impl(crud_meta: &CRUDResourceMeta) -> proc_macro2::TokenStream {
    if let Some(fn_path) = &crud_meta.fn_delete {
        quote! {
            async fn delete(db: &sea_orm::DatabaseConnection, id: uuid::Uuid) -> Result<uuid::Uuid, sea_orm::DbErr> {
                #fn_path(db, id).await
            }
        }
    } else {
        quote! {
            // Default delete implementation
            async fn delete(db: &sea_orm::DatabaseConnection, id: uuid::Uuid) -> Result<uuid::Uuid, sea_orm::DbErr> {
                use sea_orm::EntityTrait;

                let res = Self::EntityType::delete_by_id(id).exec(db).await?;
                match res.rows_affected {
                    0 => Err(sea_orm::DbErr::RecordNotFound(format!(
                        "{} not found",
                        Self::RESOURCE_NAME_SINGULAR
                    ))),
                    _ => Ok(id),
                }
            }
        }
    }
}

/// Generate `delete_many` method implementation
pub fn generate_delete_many_impl(crud_meta: &CRUDResourceMeta) -> proc_macro2::TokenStream {
    if let Some(fn_path) = &crud_meta.fn_delete_many {
        quote! {
            async fn delete_many(db: &sea_orm::DatabaseConnection, ids: Vec<uuid::Uuid>) -> Result<Vec<uuid::Uuid>, sea_orm::DbErr> {
                #fn_path(db, ids).await
            }
        }
    } else {
        quote! {
            // Default delete_many implementation
            async fn delete_many(db: &sea_orm::DatabaseConnection, ids: Vec<uuid::Uuid>) -> Result<Vec<uuid::Uuid>, sea_orm::DbErr> {
                use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                Self::EntityType::delete_many()
                    .filter(Self::ID_COLUMN.is_in(ids.clone()))
                    .exec(db)
                    .await?;
                Ok(ids)
            }
        }
    }
}
