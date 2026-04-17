use crate::codegen::joins::loading::{
    generate_get_all_batch_loading, generate_get_all_scoped_batch_loading,
    generate_get_one_join_loading, generate_get_one_scoped_join_loading,
};
use crate::codegen::models::should_include_in_model;
use crate::traits::crudresource::structs::{CRUDResourceMeta, EntityFieldAnalysis};
use convert_case::{Case, Casing};
use quote::{format_ident, quote};

/// Check if a type is `Option<T>`
fn is_option_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        type_path
            .path
            .segments
            .last()
            .map(|s| s.ident == "Option")
            .unwrap_or(false)
    } else {
        false
    }
}

/// Generate `select_only()` column selection for list queries.
/// Included columns are selected normally. Excluded Option<T> columns are replaced
/// with NULL to avoid fetching heavy data (photos, blobs) while keeping FromQueryResult happy.
/// Returns `Some(token_stream)` if there are skippable columns, `None` otherwise.
fn generate_select_only_columns(
    analysis: &EntityFieldAnalysis,
) -> Option<proc_macro2::TokenStream> {
    // Check if any DB columns are excluded from list AND are Option<T>
    let has_skippable = analysis.db_fields.iter().any(|f| {
        !should_include_in_model(f, "list_model") && is_option_type(&f.ty)
    });

    if !has_skippable {
        return None;
    }

    // Build column selections: real columns for included, NULL for excluded Option<T>
    let mut selections = Vec::new();
    for field in &analysis.db_fields {
        let col_ident = {
            let name = field.ident.as_ref().unwrap().to_string();
            format_ident!("{}", name.to_case(Case::Pascal))
        };
        let included = should_include_in_model(field, "list_model");

        if included || !is_option_type(&field.ty) {
            // Include this column (real data)
            selections.push(quote! {
                .column(<Self as crudcrate::traits::CRUDResource>::ColumnType::#col_ident)
            });
        } else {
            // Excluded Option<T> column — replace with NULL to skip data transfer
            // Use column_as with IdenStatic::as_str() to get the correct DB column name
            selections.push(quote! {
                .column_as(
                    sea_orm::sea_query::Expr::cust("NULL"),
                    sea_orm::IdenStatic::as_str(&<Self as crudcrate::traits::CRUDResource>::ColumnType::#col_ident)
                )
            });
        }
    }

    Some(quote! {
        .select_only()
        #( #selections )*
    })
}

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
/// 2. Batch query all children WHERE `parent_id` IN (`parent_ids`)
/// 3. Group children by `parent_id` in memory
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

    // Generate select_only() optimization for skipping heavy Option columns excluded from ListModel
    let select_only_columns = generate_select_only_columns(analysis);
    let select_clause = select_only_columns.unwrap_or_default();

    // Shared body builder: given a batch-loading fragment, produce the full body.
    // Used for both get_all (unscoped) and get_all_scoped variants so they share
    // ordering, pagination, select_only, and hook semantics.
    let build_body = |batch_loading: Option<(proc_macro2::TokenStream, proc_macro2::TokenStream)>| {
        if let Some(fn_path) = &hooks.body {
            // Custom body takes full control; applies to both variants.
            quote! { let result = #fn_path(db, condition, order_column, order_direction, offset, limit).await?; }
        } else if let Some((pre_loop_code, in_loop_code)) = batch_loading {
            quote! {
                use sea_orm::{QueryOrder, QuerySelect, EntityTrait, ModelTrait};

                let models = Self::EntityType::find()
                    #select_clause
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
                    #select_clause
                    .filter(condition.clone())
                    .order_by(order_column, order_direction)
                    .offset(offset)
                    .limit(limit)
                    .all(db)
                    .await?;
                let result: Vec<Self::ListModel> = models.into_iter().map(|model| Self::ListModel::from(Self::from(model))).collect();
            }
        }
    };

    let body = build_body(if has_join_all_fields {
        Some(generate_get_all_batch_loading(analysis, api_struct_name))
    } else {
        None
    });

    let scoped_body = build_body(if has_join_all_fields {
        Some(generate_get_all_scoped_batch_loading(analysis, api_struct_name))
    } else {
        None
    });

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

        async fn get_all_scoped(
            db: &sea_orm::DatabaseConnection,
            condition: &sea_orm::Condition,
            order_column: Self::ColumnType,
            order_direction: sea_orm::Order,
            offset: u64,
            limit: u64,
        ) -> Result<Vec<Self::ListModel>, crudcrate::ApiError> {
            #pre_hook
            #scoped_body
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

            // Load the main entity first — Box::pin to keep future off the stack
            let main_model = Box::pin(
                Self::EntityType::find_by_id(id).one(db)
            ).await?;

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

    // Generate get_one_scoped — scope-filtered query + scoped join loading.
    // Uses scope condition on the parent query AND child entity scope conditions on joins.
    let scoped_body = if has_joins {
        let join_loading_code = generate_get_one_scoped_join_loading(analysis, api_struct_name);
        quote! {
            use sea_orm::{EntityTrait, ModelTrait, Related, QueryFilter};

            let scoped_condition = sea_orm::Condition::all()
                .add(Self::ID_COLUMN.eq(id))
                .add(scope.clone());

            let main_model = Box::pin(
                Self::EntityType::find().filter(scoped_condition).one(db)
            ).await?;

            let result = match main_model {
                Some(model) => {
                    #join_loading_code
                }
                None => return Err(crudcrate::ApiError::not_found(Self::RESOURCE_NAME_SINGULAR, Some(id.to_string()))),
            };
        }
    } else {
        quote! {
            use sea_orm::QueryFilter;
            let scoped_condition = sea_orm::Condition::all()
                .add(Self::ID_COLUMN.eq(id))
                .add(scope.clone());
            let model = Self::EntityType::find()
                .filter(scoped_condition)
                .one(db)
                .await?;
            let result = match model {
                Some(model) => Self::from(model),
                None => return Err(crudcrate::ApiError::not_found(Self::RESOURCE_NAME_SINGULAR, Some(id.to_string()))),
            };
        }
    };

    quote! {
        async fn get_one(db: &sea_orm::DatabaseConnection, id: uuid::Uuid) -> Result<Self, crudcrate::ApiError> {
            #pre_hook
            #body
            #transform_hook
            #post_hook
            Ok(result)
        }

        async fn get_one_scoped(
            db: &sea_orm::DatabaseConnection,
            id: uuid::Uuid,
            scope: &sea_orm::Condition,
        ) -> Result<Self, crudcrate::ApiError> {
            #pre_hook
            #scoped_body
            #transform_hook
            #post_hook
            Ok(result)
        }
    }
}
