use crate::codegen::joins::loading::{generate_get_all_batch_loading, generate_get_one_join_loading};
use crate::traits::crudresource::structs::{CRUDResourceMeta, EntityFieldAnalysis};
use quote::quote;

/// Generate `get_all` method implementation with hook support.
///
/// Hook execution order: pre → body → transform → post
/// - `read::many::pre`: Preparation before query (receives condition, pagination params)
/// - `read::many::body`: Replaces default query logic (returns `Vec<ListModel>`)
/// - `read::many::transform`: Modify the results (receives `Vec<ListModel>`, returns `Vec<ListModel>`)
/// - `read::many::post`: Side effects after query (receives `&[ListModel]`)
///
/// **Performance**: Uses batch loading to reduce N+1 queries to 2 queries when loading
/// related entities. Instead of querying for each parent's children separately, we:
/// 1. Query all parents
/// 2. Batch query all children WHERE parent_id IN (parent_ids)
/// 3. Group children by parent_id in memory
pub fn generate_get_all_impl(
    crud_meta: &CRUDResourceMeta,
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    // If operations is specified, use it (takes full control)
    if let Some(ops_path) = &crud_meta.operations {
        return quote! {
            async fn get_all(
                db: &sea_orm::DatabaseConnection,
                condition: &sea_orm::Condition,
                order_column: Self::ColumnType,
                order_direction: sea_orm::Order,
                offset: u64,
                limit: u64,
            ) -> Result<Vec<Self::ListModel>, crudcrate::ApiError> {
                let ops = #ops_path;
                crudcrate::CRUDOperations::get_all(&ops, db, condition, order_column, order_direction, offset, limit).await
            }
        };
    }

    // Get hooks for read::many
    let hooks = &crud_meta.hooks.read.many;

    // Generate pre hook call
    let pre_hook = hooks.pre.as_ref().map(|fn_path| {
        quote! { #fn_path(db, condition, order_column, order_direction, offset, limit).await?; }
    });

    // Check if there are join(all) fields that need loading
    let has_join_all_fields = !analysis.join_on_all_fields.is_empty();

    // Generate body - either custom or default
    let body = if let Some(fn_path) = &hooks.body {
        quote! { let result = #fn_path(db, condition, order_column, order_direction, offset, limit).await?; }
    } else if has_join_all_fields {
        // Generate get_all with BATCH loading (optimized: 2 queries instead of N+1)
        let (pre_loop_code, in_loop_code) = generate_get_all_batch_loading(analysis, api_struct_name);
        quote! {
            use sea_orm::{QueryOrder, QuerySelect, EntityTrait, ModelTrait};

            let models = Self::EntityType::find()
                .filter(condition.clone())
                .order_by(order_column, order_direction)
                .offset(offset)
                .limit(limit)
                .all(db)
                .await?;

            // Batch load all related entities (one query per join field)
            #pre_loop_code

            // Assign pre-loaded data to each model (no queries in loop)
            let mut result = Vec::new();
            for model in models {
                let item = {
                    #in_loop_code
                };
                result.push(Self::ListModel::from(item));
            }
        }
    } else {
        // Standard get_all without joins
        quote! {
            use sea_orm::{QueryOrder, QuerySelect, EntityTrait};

            let models = Self::EntityType::find()
                .filter(condition.clone())
                .order_by(order_column, order_direction)
                .offset(offset)
                .limit(limit)
                .all(db)
                .await?;
            let result: Vec<Self::ListModel> = models.into_iter().map(|model| Self::ListModel::from(Self::from(model))).collect();
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
        async fn get_all(
            db: &sea_orm::DatabaseConnection,
            condition: &sea_orm::Condition,
            order_column: Self::ColumnType,
            order_direction: sea_orm::Order,
            offset: u64,
            limit: u64,
        ) -> Result<Vec<Self::ListModel>, crudcrate::ApiError> {
            #pre_hook
            #body
            #transform_hook
            #post_hook
            Ok(result)
        }
    }
}

/// Generate `get_one` method implementation with hook support.
///
/// Hook execution order: pre → body → transform → post
/// - `read::one::pre`: Preparation before fetch (receives id)
/// - `read::one::body`: Replaces default fetch logic (receives id, returns `Self`)
/// - `read::one::transform`: Modify the result (receives `Self`, returns `Self`)
/// - `read::one::post`: Side effects after fetch (receives `&Self`)
pub fn generate_get_one_impl(
    crud_meta: &CRUDResourceMeta,
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    // If operations is specified, use it (takes full control)
    if let Some(ops_path) = &crud_meta.operations {
        return quote! {
            async fn get_one(db: &sea_orm::DatabaseConnection, id: uuid::Uuid) -> Result<Self, crudcrate::ApiError> {
                let ops = #ops_path;
                crudcrate::CRUDOperations::get_one(&ops, db, id).await
            }
        };
    }

    // Get hooks for read::one
    let hooks = &crud_meta.hooks.read.one;

    // Generate pre hook call
    let pre_hook = hooks.pre.as_ref().map(|fn_path| {
        quote! { #fn_path(db, id).await?; }
    });

    // Generate default implementation for get_one with recursive join support
    let has_joins =
        !analysis.join_on_one_fields.is_empty() || !analysis.join_on_all_fields.is_empty();

    // Generate body - either custom or default
    let body = if let Some(fn_path) = &hooks.body {
        quote! { let result = #fn_path(db, id).await?; }
    } else if has_joins {
        // Use consolidated join loading implementation
        let join_loading_code = generate_get_one_join_loading(analysis, api_struct_name);
        quote! {
            use sea_orm::{EntityTrait, ModelTrait, Related};

            // Load the main entity first
            let main_model = Self::EntityType::find_by_id(id)
                .one(db)
                .await?;

            let result = match main_model {
                Some(model) => {
                    #join_loading_code
                }
                None => return Err(crudcrate::ApiError::not_found(Self::RESOURCE_NAME_SINGULAR, Some(id.to_string()))),
            };
        }
    } else {
        quote! {
            let model = Self::EntityType::find_by_id(id)
                .one(db)
                .await?;
            let result = match model {
                Some(model) => Self::from(model),
                None => return Err(crudcrate::ApiError::not_found(Self::RESOURCE_NAME_SINGULAR, Some(id.to_string()))),
            };
        }
    };

    // Generate transform hook call (modifies the result)
    let transform_hook = hooks.transform.as_ref().map(|fn_path| {
        quote! { let result = #fn_path(db, result).await?; }
    });

    // Generate post hook call
    let post_hook = hooks.post.as_ref().map(|fn_path| {
        quote! { #fn_path(db, &result).await?; }
    });

    quote! {
        async fn get_one(db: &sea_orm::DatabaseConnection, id: uuid::Uuid) -> Result<Self, crudcrate::ApiError> {
            #pre_hook
            #body
            #transform_hook
            #post_hook
            Ok(result)
        }
    }
}
