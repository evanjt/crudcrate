// join_generators functionality consolidated into this file to avoid duplicate/stub implementations
use crate::traits::crudresource::structs::CRUDResourceMeta;
use quote::quote;

/// Generate delete method implementation
pub fn generate_delete_impl(crud_meta: &CRUDResourceMeta) -> proc_macro2::TokenStream {
    // If operations is specified, use it; otherwise fall back to fn_delete or default
    if let Some(ops_path) = &crud_meta.operations {
        quote! {
            async fn delete(db: &sea_orm::DatabaseConnection, id: uuid::Uuid) -> Result<uuid::Uuid, sea_orm::DbErr> {
                let ops = #ops_path;
                crudcrate::CRUDOperations::delete(&ops, db, id).await
            }
        }
    } else if let Some(fn_path) = &crud_meta.fn_delete {
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
///
/// **Security Note**: The default implementation limits batch deletes to 100 items to prevent
/// DoS attacks via resource exhaustion. To handle larger batches, provide a custom implementation:
///
/// ```ignore
/// #[crudcrate(fn_delete_many = my_custom_delete_many)]
/// ```
///
/// Your custom function signature:
/// ```ignore
/// async fn my_custom_delete_many(
///     db: &sea_orm::DatabaseConnection,
///     ids: Vec<uuid::Uuid>
/// ) -> Result<Vec<uuid::Uuid>, sea_orm::DbErr>
/// ```
pub fn generate_delete_many_impl(crud_meta: &CRUDResourceMeta) -> proc_macro2::TokenStream {
    // If operations is specified, use it; otherwise fall back to fn_delete_many or default
    if let Some(ops_path) = &crud_meta.operations {
        quote! {
            async fn delete_many(db: &sea_orm::DatabaseConnection, ids: Vec<uuid::Uuid>) -> Result<Vec<uuid::Uuid>, sea_orm::DbErr> {
                let ops = #ops_path;
                crudcrate::CRUDOperations::delete_many(&ops, db, ids).await
            }
        }
    } else if let Some(fn_path) = &crud_meta.fn_delete_many {
        quote! {
            async fn delete_many(db: &sea_orm::DatabaseConnection, ids: Vec<uuid::Uuid>) -> Result<Vec<uuid::Uuid>, sea_orm::DbErr> {
                #fn_path(db, ids).await
            }
        }
    } else {
        quote! {
            /// Default delete_many implementation with security limits
            ///
            /// **SECURITY LIMIT**: Batch deletes are limited to 100 items to prevent DoS attacks.
            /// To increase this limit, provide a custom `fn_delete_many` implementation:
            ///
            /// ```ignore
            /// #[crudcrate(fn_delete_many = my_custom_delete_many)]
            /// async fn my_custom_delete_many(
            ///     db: &sea_orm::DatabaseConnection,
            ///     ids: Vec<uuid::Uuid>
            /// ) -> Result<Vec<uuid::Uuid>, sea_orm::DbErr> {
            ///     const MAX_SIZE: usize = 500; // Your custom limit
            ///     if ids.len() > MAX_SIZE {
            ///         return Err(sea_orm::DbErr::Custom(format!("Too many items: {}", ids.len())));
            ///     }
            ///     // Your implementation...
            /// }
            /// ```
            async fn delete_many(db: &sea_orm::DatabaseConnection, ids: Vec<uuid::Uuid>) -> Result<Vec<uuid::Uuid>, sea_orm::DbErr> {
                use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                // Security: Limit batch size to prevent DoS attacks
                // To increase, provide custom fn_delete_many implementation (see docs above)
                const MAX_BATCH_DELETE_SIZE: usize = 100;
                if ids.len() > MAX_BATCH_DELETE_SIZE {
                    return Err(sea_orm::DbErr::Custom(
                        format!("Batch delete limited to {} items. Received {} items. Use fn_delete_many attribute for custom limits.", MAX_BATCH_DELETE_SIZE, ids.len())
                    ));
                }

                Self::EntityType::delete_many()
                    .filter(Self::ID_COLUMN.is_in(ids.clone()))
                    .exec(db)
                    .await?;
                Ok(ids)
            }
        }
    }
}
