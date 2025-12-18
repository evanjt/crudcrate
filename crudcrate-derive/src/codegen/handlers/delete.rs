// join_generators functionality consolidated into this file to avoid duplicate/stub implementations
use crate::traits::crudresource::structs::CRUDResourceMeta;
use quote::quote;

/// Generate delete method implementation with hook support.
///
/// Hook execution order: pre → body → transform → post
/// - `delete::one::pre`: Validation/cleanup before delete (receives id)
/// - `delete::one::body`: Replaces default delete logic (receives id, returns `Uuid`)
/// - `delete::one::transform`: Modify the result (receives `Uuid`, returns `Uuid`)
/// - `delete::one::post`: Side effects after delete (receives deleted id)
pub fn generate_delete_impl(crud_meta: &CRUDResourceMeta) -> proc_macro2::TokenStream {
    // If operations is specified, use it (takes full control)
    if let Some(ops_path) = &crud_meta.operations {
        return quote! {
            async fn delete(db: &sea_orm::DatabaseConnection, id: uuid::Uuid) -> Result<uuid::Uuid, crudcrate::ApiError> {
                let ops = #ops_path;
                crudcrate::CRUDOperations::delete(&ops, db, id).await
            }
        };
    }

    // Get hooks for delete::one
    let hooks = &crud_meta.hooks.delete.one;

    // Generate pre hook call
    let pre_hook = hooks.pre.as_ref().map(|fn_path| {
        quote! { #fn_path(db, id).await?; }
    });

    // Generate body - either custom or default
    let body = if let Some(fn_path) = &hooks.body {
        quote! { let result = #fn_path(db, id).await?; }
    } else {
        quote! {
            use sea_orm::EntityTrait;

            let res = Self::EntityType::delete_by_id(id).exec(db).await?;
            let result = match res.rows_affected {
                0 => return Err(crudcrate::ApiError::not_found(
                    Self::RESOURCE_NAME_SINGULAR,
                    Some(id.to_string())
                )),
                _ => id,
            };
        }
    };

    // Generate transform hook call (modifies the result)
    let transform_hook = hooks.transform.as_ref().map(|fn_path| {
        quote! { let result = #fn_path(db, result).await?; }
    });

    // Generate post hook call
    let post_hook = hooks.post.as_ref().map(|fn_path| {
        quote! { #fn_path(db, result).await?; }
    });

    quote! {
        async fn delete(db: &sea_orm::DatabaseConnection, id: uuid::Uuid) -> Result<uuid::Uuid, crudcrate::ApiError> {
            #pre_hook
            #body
            #transform_hook
            #post_hook
            Ok(result)
        }
    }
}

/// Generate `delete_many` method implementation with hook support.
///
/// Hook execution order: pre → body → transform → post
/// - `delete::many::pre`: Validation/cleanup before batch delete (receives `&[Uuid]`)
/// - `delete::many::body`: Replaces default delete logic (receives `Vec<Uuid>`, returns `Vec<Uuid>`)
/// - `delete::many::transform`: Modify the results (receives `Vec<Uuid>`, returns `Vec<Uuid>`)
/// - `delete::many::post`: Side effects after batch delete (receives deleted ids)
///
/// **Security Note**: The default implementation limits batch deletes to 100 items to prevent
/// DoS attacks via resource exhaustion.
pub fn generate_delete_many_impl(crud_meta: &CRUDResourceMeta) -> proc_macro2::TokenStream {
    // If operations is specified, use it (takes full control)
    if let Some(ops_path) = &crud_meta.operations {
        return quote! {
            async fn delete_many(db: &sea_orm::DatabaseConnection, ids: Vec<uuid::Uuid>) -> Result<Vec<uuid::Uuid>, crudcrate::ApiError> {
                let ops = #ops_path;
                crudcrate::CRUDOperations::delete_many(&ops, db, ids).await
            }
        };
    }

    // Get hooks for delete::many
    let hooks = &crud_meta.hooks.delete.many;

    // Generate pre hook call
    let pre_hook = hooks.pre.as_ref().map(|fn_path| {
        quote! { #fn_path(db, &ids).await?; }
    });

    // Generate body - either custom or default
    let body = if let Some(fn_path) = &hooks.body {
        quote! { let result = #fn_path(db, ids).await?; }
    } else {
        quote! {
            use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

            // Security: Limit batch size to prevent DoS attacks (uses configurable BATCH_LIMIT)
            if ids.len() > Self::BATCH_LIMIT {
                return Err(crudcrate::ApiError::bad_request(
                    format!("Batch delete limited to {} items. Received {} items.", Self::BATCH_LIMIT, ids.len())
                ));
            }

            Self::EntityType::delete_many()
                .filter(Self::ID_COLUMN.is_in(ids.clone()))
                .exec(db)
                .await?;
            let result = ids;
        }
    };

    // Generate transform hook call (modifies the results)
    let transform_hook = hooks.transform.as_ref().map(|fn_path| {
        quote! { let result = #fn_path(db, result).await?; }
    });

    // Generate post hook call
    let post_hook = hooks.post.as_ref().map(|fn_path| {
        quote! { #fn_path(db, &result).await?; }
    });

    quote! {
        async fn delete_many(db: &sea_orm::DatabaseConnection, ids: Vec<uuid::Uuid>) -> Result<Vec<uuid::Uuid>, crudcrate::ApiError> {
            #pre_hook
            #body
            #transform_hook
            #post_hook
            Ok(result)
        }
    }
}
