// join_generators functionality consolidated into this file to avoid duplicate/stub implementations
use crate::traits::crudresource::structs::CRUDResourceMeta;
use quote::quote;

/// Generate create method implementation with hook support.
///
/// Hook execution order: pre → body → post
/// - `create::one::pre`: Validation/preparation before create (receives &CreateModel)
/// - `create::one::body`: Replaces default create logic (receives CreateModel, returns Self)
/// - `create::one::post`: Side effects after create (receives &Self)
pub fn generate_create_impl(crud_meta: &CRUDResourceMeta) -> proc_macro2::TokenStream {
    // If operations is specified, use it (takes full control)
    if let Some(ops_path) = &crud_meta.operations {
        return quote! {
            async fn create(db: &sea_orm::DatabaseConnection, data: Self::CreateModel) -> Result<Self, crudcrate::ApiError> {
                let ops = #ops_path;
                crudcrate::CRUDOperations::create(&ops, db, data).await
            }
        };
    }

    // Get hooks for create::one
    let hooks = &crud_meta.hooks.create.one;

    // Generate pre hook call
    let pre_hook = hooks.pre.as_ref().map(|fn_path| {
        quote! { #fn_path(db, &data).await?; }
    });

    // Generate body - either custom or default
    let body = if let Some(fn_path) = &hooks.body {
        quote! { let result = #fn_path(db, data).await?; }
    } else {
        quote! {
            let active_model: Self::ActiveModelType = data.into();
            let insert_result = Self::EntityType::insert(active_model).exec(db).await?;
            let result = Self::get_one(db, insert_result.last_insert_id.into()).await?;
        }
    };

    // Generate post hook call
    let post_hook = hooks.post.as_ref().map(|fn_path| {
        quote! { #fn_path(db, &result).await?; }
    });

    quote! {
        async fn create(db: &sea_orm::DatabaseConnection, data: Self::CreateModel) -> Result<Self, crudcrate::ApiError> {
            #pre_hook
            #body
            #post_hook
            Ok(result)
        }
    }
}

/// Generate `create_many` method implementation with hook support.
///
/// Hook execution order: pre → body → post
/// - `create::many::pre`: Validation/preparation before batch create (receives &[CreateModel])
/// - `create::many::body`: Replaces default create logic (receives Vec<CreateModel>, returns Vec<Self>)
/// - `create::many::post`: Side effects after batch create (receives &[Self])
///
/// **Security Note**: The default implementation limits batch creates to 100 items to prevent
/// DoS attacks via resource exhaustion.
pub fn generate_create_many_impl(crud_meta: &CRUDResourceMeta) -> proc_macro2::TokenStream {
    // If operations is specified, use it (takes full control)
    if let Some(ops_path) = &crud_meta.operations {
        return quote! {
            async fn create_many(db: &sea_orm::DatabaseConnection, data: Vec<Self::CreateModel>) -> Result<Vec<Self>, crudcrate::ApiError> {
                let ops = #ops_path;
                crudcrate::CRUDOperations::create_many(&ops, db, data).await
            }
        };
    }

    // Get hooks for create::many
    let hooks = &crud_meta.hooks.create.many;

    // Generate pre hook call
    let pre_hook = hooks.pre.as_ref().map(|fn_path| {
        quote! { #fn_path(db, &data).await?; }
    });

    // Generate body - either custom or default
    let body = if let Some(fn_path) = &hooks.body {
        quote! { let result = #fn_path(db, data).await?; }
    } else {
        quote! {
            use sea_orm::ActiveModelTrait;

            // Security: Limit batch size to prevent DoS attacks
            const MAX_BATCH_CREATE_SIZE: usize = 100;
            if data.len() > MAX_BATCH_CREATE_SIZE {
                return Err(crudcrate::ApiError::bad_request(
                    format!("Batch create limited to {} items. Received {} items.", MAX_BATCH_CREATE_SIZE, data.len())
                ));
            }

            let mut result = Vec::with_capacity(data.len());
            for create_model in data {
                let active_model: Self::ActiveModelType = create_model.into();
                let model = active_model.insert(db).await?;
                result.push(Self::from(model));
            }
        }
    };

    // Generate post hook call
    let post_hook = hooks.post.as_ref().map(|fn_path| {
        quote! { #fn_path(db, &result).await?; }
    });

    quote! {
        async fn create_many(db: &sea_orm::DatabaseConnection, data: Vec<Self::CreateModel>) -> Result<Vec<Self>, crudcrate::ApiError> {
            #pre_hook
            #body
            #post_hook
            Ok(result)
        }
    }
}
