//! Batch join loading for `get_all`: one query per join field, grouped by parent id.

use crate::attrs::get_join_config;
use crate::codegen::joins::fk::{
    MAX_JOIN_DEPTH, derive_fk_idents, list_type_of_child, self_referencing,
};
use crate::ir::EntityFieldAnalysis;
use crate::syn_type::{
    extract_api_struct_type_for_recursive_call, extract_option_or_direct_inner_type,
    get_path_from_field_type, is_vec_type,
};
use quote::quote;

/// Generate batch loading code for `get_all()` method
///
/// This generates optimized batch loading that reduces N+1 queries to 2 queries:
/// 1. One query to fetch N parent entities (already done before this code runs)
/// 2. One query per join field to fetch ALL related entities for ALL parents
///
/// Returns a tuple of (`pre_loop_code`, `in_loop_code)`:
/// - `pre_loop_code`: Batch loads all related entities and groups them by parent ID
/// - `in_loop_code`: Looks up pre-loaded data from `HashMaps` (no queries)
pub(crate) fn generate_get_all_batch_loading(
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    if analysis.join_on_all_fields.is_empty() {
        return (quote! {}, quote! { Self::from(model) });
    }

    // Extract PK field ident (fallback to `id` for backward compat)
    let pk_ident = analysis
        .primary_key_field
        .and_then(|f| f.ident.as_ref())
        .cloned()
        .unwrap_or_else(|| quote::format_ident!("id"));

    let join_fields: Vec<&syn::Field> = analysis.join_on_all_fields.clone();
    generate_batch_loading_impl(&join_fields, api_struct_name, &pk_ident, false)
}

/// Generate batch loading code for `get_all_scoped()`: applies child entity
/// scope conditions to Vec child batch queries at the SQL level, and recurses
/// via `get_one_scoped` for depth > 1 so grandchildren are also filtered.
pub(crate) fn generate_get_all_scoped_batch_loading(
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    if analysis.join_on_all_fields.is_empty() {
        return (quote! {}, quote! { Self::from(model) });
    }

    let pk_ident = analysis
        .primary_key_field
        .and_then(|f| f.ident.as_ref())
        .cloned()
        .unwrap_or_else(|| quote::format_ident!("id"));

    let join_fields: Vec<&syn::Field> = analysis.join_on_all_fields.clone();
    generate_batch_loading_impl(&join_fields, api_struct_name, &pk_ident, true)
}

/// Generate optimized batch loading code for `get_all()`
///
/// Returns (`pre_loop_code`, `in_loop_code`) where:
/// - `pre_loop_code` runs ONCE before the loop to batch load all related entities
/// - `in_loop_code` runs for each model to assign pre-loaded data
///
/// When `scoped` is true, Vec<T> child batch queries include the child entity's
/// `ScopeFilterable::scope_condition()` at the SQL level, and depth > 1 recursion
/// uses `get_one_scoped` so that grandchildren are filtered too.
fn generate_batch_loading_impl(
    join_fields: &[&syn::Field],
    api_struct_name: &syn::Ident,
    pk_ident: &syn::Ident,
    scoped: bool,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let mut batch_loading_statements = Vec::new();
    let mut field_assignments = Vec::new();

    // The parent's primary-key value type. Used as the HashMap key and the
    // `parent_ids` element type so batch loading works for UUID, integer, or
    // String primary keys (not just `uuid::Uuid`). Generated inside the
    // `impl CRUDResource for ParentApiStruct`, so `Self` resolves to the parent.
    let parent_pk_ty = quote! { crudcrate::PrimaryKeyType<Self> };

    for field in join_fields {
        let Some(field_name) = &field.ident else {
            continue;
        };

        let join_config = get_join_config(field).unwrap_or_default();
        let is_vec_field = is_vec_type(&field.ty);

        // Check if this is a self-referencing field
        let inner_type = extract_api_struct_type_for_recursive_call(&field.ty);
        let _inner_type_string = inner_type.to_string();
        let _api_struct_name_string = api_struct_name.to_string();
        let is_self_referencing = self_referencing(&field.ty, api_struct_name);

        // Security: Cap depth
        let effective_depth = if is_self_referencing {
            let original_depth = join_config.depth.unwrap_or(1).min(MAX_JOIN_DEPTH);
            if original_depth > 1 {
                let error_msg = format!(
                    "Self-referencing field '{field_name}' in struct '{api_struct_name}' has depth={original_depth}, but self-references only support depth=1"
                );
                return (quote! { compile_error!(#error_msg); }, quote! {});
            }
            1
        } else {
            join_config
                .depth
                .unwrap_or(MAX_JOIN_DEPTH)
                .min(MAX_JOIN_DEPTH)
        };

        let depth_limited = effective_depth == 1;

        // Get entity and model paths
        let (entity_path, model_path) = if let Some(custom_path) = &join_config.path {
            if let Ok(path_tokens) = custom_path.parse::<proc_macro2::TokenStream>() {
                (
                    quote! { #path_tokens::Entity },
                    quote! { #path_tokens::Model },
                )
            } else {
                let error_msg =
                    format!("Invalid join path '{custom_path}' for field '{field_name}'");
                return (quote! { compile_error!(#error_msg); }, quote! {});
            }
        } else {
            (
                get_path_from_field_type(&field.ty, "Entity"),
                get_path_from_field_type(&field.ty, "Model"),
            )
        };

        // Get the Column path for the FK column
        let column_path = get_path_from_field_type(&field.ty, "Column");

        let (fk_column_pascal, fk_field_snake, use_runtime) =
            derive_fk_idents(&join_config, api_struct_name, is_self_referencing);

        // HashMap variable for storing batch-loaded data
        let map_var = quote::format_ident!("{}_by_parent", field_name);

        // When scoped, compute the child's ScopeFilterable::scope_condition()
        // once per batch query and apply it to both the SQL-level filter and
        // the depth > 1 recursive fetch.
        let scope_filter_for_vec = if scoped && is_vec_field {
            let child_list_type = list_type_of_child(field);
            quote! {
                let __child_scope: Option<sea_orm::Condition> =
                    <#child_list_type as crudcrate::ScopeFilterable>::scope_condition();
                let query = if let Some(ref cs) = __child_scope {
                    query.filter(cs.clone())
                } else {
                    query
                };
            }
        } else {
            quote! {}
        };

        if is_vec_field {
            let api_struct_type = extract_api_struct_type_for_recursive_call(&field.ty);

            if depth_limited {
                // Depth=1: Simple batch load without recursion
                // Each batch load is Box::pin'd to move its future to the heap,
                // preventing async state machine bloat when multiple joins accumulate.
                if is_self_referencing {
                    // Self-referencing: FK column derived via derive_fk_idents
                    batch_loading_statements.push(quote! {
                        let mut #map_var: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> = Box::pin(async {
                            use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                            let query = #entity_path::find()
                                .filter(#column_path::#fk_column_pascal.is_in(parent_ids.clone()));
                            #scope_filter_for_vec
                            let all_related = query.all(db).await?;

                            let mut map: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> =
                                std::collections::HashMap::new();
                            for related_model in all_related {
                                if let Some(parent_id) = related_model.#fk_field_snake {
                                    map.entry(parent_id)
                                        .or_insert_with(Vec::new)
                                        .push(#api_struct_type::from(related_model));
                                }
                            }
                            Ok::<_, crudcrate::ApiError>(map)
                        }).await?;
                    });
                } else if use_runtime {
                    // Runtime FK resolution from SeaORM RelationDef
                    batch_loading_statements.push(quote! {
                        let mut #map_var: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> = Box::pin(async {
                            use sea_orm::{EntityTrait, ExprTrait, QueryFilter, ColumnTrait, ModelTrait};
                            use std::str::FromStr;

                            let __rel_def = <#entity_path as sea_orm::Related<
                                <Self as crudcrate::traits::CRUDResource>::EntityType
                            >>::to();
                            let __fk_col_name = sea_orm::Iden::to_string(&__rel_def.from_col);

                            let query = #entity_path::find()
                                .filter(sea_orm::sea_query::Expr::col(
                                    sea_orm::sea_query::Alias::new(&__fk_col_name)
                                ).is_in(parent_ids.clone()));
                            #scope_filter_for_vec
                            let all_related = query.all(db).await?;

                            let __fk_col = match <<#entity_path as sea_orm::EntityTrait>::Column
                                as FromStr>::from_str(&__fk_col_name)
                            {
                                Ok(__c) => __c,
                                Err(_) => return Err(crudcrate::ApiError::internal(
                                    "CrudCrate: foreign key column not found on child entity",
                                    None,
                                )),
                            };

                            let mut map: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> =
                                std::collections::HashMap::new();
                            for related_model in all_related {
                                let fk_value: #parent_pk_ty = match <#parent_pk_ty as sea_orm::sea_query::ValueType>::try_from(
                                    ModelTrait::get(&related_model, __fk_col.clone())
                                ) {
                                    Ok(v) => v,
                                    Err(_) => continue,
                                };
                                map.entry(fk_value)
                                    .or_insert_with(Vec::new)
                                    .push(#api_struct_type::from(related_model));
                            }
                            Ok::<_, crudcrate::ApiError>(map)
                        }).await?;
                    });
                } else {
                    // Static FK column (explicit fk_column override)
                    batch_loading_statements.push(quote! {
                        let mut #map_var: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> = Box::pin(async {
                            use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                            let query = #entity_path::find()
                                .filter(#column_path::#fk_column_pascal.is_in(parent_ids.clone()));
                            #scope_filter_for_vec
                            let all_related = query.all(db).await?;

                            let mut map: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> =
                                std::collections::HashMap::new();
                            for related_model in all_related {
                                let fk_value = related_model.#fk_field_snake.clone();
                                map.entry(fk_value)
                                    .or_insert_with(Vec::new)
                                    .push(#api_struct_type::from(related_model));
                            }
                            Ok::<_, crudcrate::ApiError>(map)
                        }).await?;
                    });
                }

                field_assignments.push(quote! {
                    item.#field_name = #map_var.remove(&parent_id).unwrap_or_default();
                });
            } else {
                // Depth > 1: Need recursive loading via get_one() / get_one_scoped()
                // When scoped, recurse via get_one_scoped so grandchildren are also
                // filtered by the child's scope_condition. Note: Self-referencing
                // fields are always depth=1, so this branch is only reached for
                // cross-model joins.
                let recursive_fetch = if scoped {
                    quote! {
                        let entity = match __child_scope.as_ref() {
                            Some(cs) => match #api_struct_type::get_one_scoped(db, related_model.id, cs).await {
                                Ok(e) => e,
                                Err(e) => {
                                    crudcrate::tracing::warn!(error = %e, "Failed to load nested scoped relations, using flat model");
                                    #api_struct_type::from(related_model)
                                }
                            },
                            None => match #api_struct_type::get_one(db, related_model.id).await {
                                Ok(e) => e,
                                Err(e) => {
                                    crudcrate::tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                    #api_struct_type::from(related_model)
                                }
                            },
                        };
                    }
                } else {
                    quote! {
                        let entity = match #api_struct_type::get_one(db, related_model.id).await {
                            Ok(e) => e,
                            Err(e) => {
                                crudcrate::tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                #api_struct_type::from(related_model)
                            }
                        };
                    }
                };

                if use_runtime {
                    batch_loading_statements.push(quote! {
                        let mut #map_var: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> = Box::pin(async {
                            use sea_orm::{EntityTrait, ExprTrait, QueryFilter, ColumnTrait, ModelTrait};
                            use std::str::FromStr;

                            let __rel_def = <#entity_path as sea_orm::Related<
                                <Self as crudcrate::traits::CRUDResource>::EntityType
                            >>::to();
                            let __fk_col_name = sea_orm::Iden::to_string(&__rel_def.from_col);

                            let query = #entity_path::find()
                                .filter(sea_orm::sea_query::Expr::col(
                                    sea_orm::sea_query::Alias::new(&__fk_col_name)
                                ).is_in(parent_ids.clone()));
                            #scope_filter_for_vec
                            let all_related_models: Vec<#model_path> = query.all(db).await?;

                            let __fk_col = match <<#entity_path as sea_orm::EntityTrait>::Column
                                as FromStr>::from_str(&__fk_col_name)
                            {
                                Ok(__c) => __c,
                                Err(_) => return Err(crudcrate::ApiError::internal(
                                    "CrudCrate: foreign key column not found on child entity",
                                    None,
                                )),
                            };

                            let mut map: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> =
                                std::collections::HashMap::new();
                            for related_model in all_related_models {
                                let fk_value: #parent_pk_ty = match <#parent_pk_ty as sea_orm::sea_query::ValueType>::try_from(
                                    ModelTrait::get(&related_model, __fk_col.clone())
                                ) {
                                    Ok(v) => v,
                                    Err(_) => continue,
                                };
                                #recursive_fetch
                                map.entry(fk_value)
                                    .or_insert_with(Vec::new)
                                    .push(entity);
                            }
                            Ok::<_, crudcrate::ApiError>(map)
                        }).await?;
                    });
                } else {
                    batch_loading_statements.push(quote! {
                        let mut #map_var: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> = Box::pin(async {
                            use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                            let query = #entity_path::find()
                                .filter(#column_path::#fk_column_pascal.is_in(parent_ids.clone()));
                            #scope_filter_for_vec
                            let all_related_models: Vec<#model_path> = query.all(db).await?;

                            let mut map: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> =
                                std::collections::HashMap::new();
                            for related_model in all_related_models {
                                let fk_value = related_model.#fk_field_snake.clone();
                                #recursive_fetch
                                map.entry(fk_value)
                                    .or_insert_with(Vec::new)
                                    .push(entity);
                            }
                            Ok::<_, crudcrate::ApiError>(map)
                        }).await?;
                    });
                }

                field_assignments.push(quote! {
                    item.#field_name = #map_var.remove(&parent_id).unwrap_or_default();
                });
            }
        } else {
            // Option<T> relationships (belongs_to/has_one).
            //
            // The FK direction differs between the two shapes: belongs_to has
            // the FK on the parent row, has_one has it on the child. SeaORM's
            // `find_related()` resolves the direction from the `Related<E>`
            // relation definition, so we load each parent's relation that way
            // instead of hand-rolling a sub-query keyed by parent PK (which
            // only handles the has_one direction and silently returns nothing
            // for belongs_to). To-one relations are 1:1 and gain little from a
            // single batched query, so iterating the already-fetched parent
            // models keeps the loader correct for both directions.
            //
            // `fk_column_pascal`, `fk_field_snake`, `column_path`, `model_path`,
            // and `use_runtime` are not needed on this path; mark them used so
            // the Vec<T> branch's bindings don't trip unused-variable lints.
            let _ = (
                &fk_column_pascal,
                &fk_field_snake,
                &column_path,
                &model_path,
                use_runtime,
            );

            let target_type = extract_option_or_direct_inner_type(&field.ty);

            let load_related = if depth_limited {
                quote! {
                    let related = Box::pin(
                        parent_model.find_related(#entity_path).one(db)
                    ).await?
                    .map(#target_type::from);
                }
            } else if scoped {
                // depth > 1, scoped: recurse via get_one_scoped (when the child carries
                // a scope_condition) so the child's own nested joins stay scope-filtered.
                // The Vec<T> branch already does this. Without it an Option<Child> leaks
                // private grandchildren into a scoped response.
                let child_list_type = list_type_of_child(field);
                quote! {
                    let related = match Box::pin(
                        parent_model.find_related(#entity_path).one(db)
                    ).await? {
                        Some(related_model) => {
                            let __child_scope = <#child_list_type as crudcrate::ScopeFilterable>::scope_condition();
                            let __loaded = match __child_scope {
                                Some(cs) => #target_type::get_one_scoped(db, related_model.id, &cs).await,
                                None => #target_type::get_one(db, related_model.id).await,
                            };
                            match __loaded {
                                Ok(e) => Some(e),
                                Err(e) => {
                                    crudcrate::tracing::warn!(error = %e, "Failed to load nested scoped relations, using flat model");
                                    Some(#target_type::from(related_model))
                                }
                            }
                        }
                        None => None,
                    };
                }
            } else {
                // depth > 1: recurse via the child's get_one so the child's own
                // nested joins are loaded too. Fall back to the flat model on error.
                quote! {
                    let related = match Box::pin(
                        parent_model.find_related(#entity_path).one(db)
                    ).await? {
                        Some(related_model) => {
                            match #target_type::get_one(db, related_model.id).await {
                                Ok(e) => Some(e),
                                Err(e) => {
                                    crudcrate::tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                    Some(#target_type::from(related_model))
                                }
                            }
                        }
                        None => None,
                    };
                }
            };

            batch_loading_statements.push(quote! {
                let mut #map_var: std::collections::HashMap<#parent_pk_ty, #target_type> = Box::pin(async {
                    use sea_orm::{EntityTrait, ModelTrait};

                    let mut map: std::collections::HashMap<#parent_pk_ty, #target_type> =
                        std::collections::HashMap::new();
                    for parent_model in models.iter() {
                        #load_related
                        if let Some(related) = related {
                            map.insert(parent_model.#pk_ident.clone(), related);
                        }
                    }
                    Ok::<_, crudcrate::ApiError>(map)
                }).await?;
            });

            field_assignments.push(quote! {
                item.#field_name = #map_var.remove(&parent_id);
            });
        }
    }

    let pre_loop_code = quote! {
        // Collect all parent IDs for batch loading. Clone each PK so the parent
        // models stay intact for later conversion (the PK value type is not
        // required to be Copy, e.g. String primary keys).
        let parent_ids: Vec<#parent_pk_ty> = models.iter().map(|m| m.#pk_ident.clone()).collect();

        #( #batch_loading_statements )*
    };

    let in_loop_code = quote! {
        // Clone the PK before moving `model` into `Self::from` so non-Copy PK
        // types (String) don't trigger a partial-move error.
        let parent_id = model.#pk_ident.clone();
        let mut item = Self::from(model);
        #( #field_assignments )*
        item
    };

    (pre_loop_code, in_loop_code)
}
