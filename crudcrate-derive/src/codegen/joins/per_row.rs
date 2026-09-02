//! Per-row join loading for `get_one`, including depth recursion.

use crate::attrs::get_join_config;
use crate::codegen::joins::fk::{
    MAX_JOIN_DEPTH, derive_fk_idents, list_type_of_child, relation_def_expr, self_referencing,
};
use crate::ir::EntityFieldAnalysis;
use crate::syn_type::{
    extract_api_struct_type_for_recursive_call, extract_option_or_direct_inner_type,
    get_path_from_field_type, is_vec_type,
};
use quote::quote;

/// Generate join loading code for `get_one()` method
///
/// Returns code that evaluates to `Self` (not wrapped in Result).
/// The caller is responsible for wrapping in `Ok()`.
pub(crate) fn generate_get_one_join_loading(
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    generate_get_one_join_loading_inner(analysis, api_struct_name, false)
}

/// Generate join loading code for `get_one_scoped()`: applies child entity
/// scope conditions to Vec join queries at the SQL level.
pub(crate) fn generate_get_one_scoped_join_loading(
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    generate_get_one_join_loading_inner(analysis, api_struct_name, true)
}

fn generate_get_one_join_loading_inner(
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
    scoped: bool,
) -> proc_macro2::TokenStream {
    // Check if there are any join fields
    if analysis.join_on_one_fields.is_empty() && analysis.join_on_all_fields.is_empty() {
        return quote! { Self::from(model) };
    }

    // Deduplicate fields (some may have both join(one) and join(all))
    let mut seen_fields = std::collections::HashSet::new();
    let mut join_fields: Vec<&syn::Field> = Vec::new();

    for field in analysis
        .join_on_one_fields
        .iter()
        .chain(analysis.join_on_all_fields.iter())
    {
        if field
            .ident
            .as_ref()
            .is_none_or(|name| seen_fields.insert(name.to_string()))
        {
            join_fields.push(field);
        }
    }

    generate_join_loading_impl(&join_fields, api_struct_name, scoped)
}

/// Shared implementation for generating join loading code
///
/// When `scoped` is true, Vec<T> join queries include the child entity's
/// `ScopeFilterable::scope_condition()` as an additional WHERE clause,
/// filtering private children at the SQL level.
fn generate_join_loading_impl(
    join_fields: &[&syn::Field],
    api_struct_name: &syn::Ident,
    scoped: bool,
) -> proc_macro2::TokenStream {
    let mut loading_statements = Vec::new();
    let mut field_assignments = Vec::new();

    for field in join_fields {
        let Some(field_name) = &field.ident else {
            continue;
        };
        let field_name_str = field_name.to_string();

        let join_config = get_join_config(field).unwrap_or_default();
        let is_vec_field = is_vec_type(&field.ty);

        // Check if this is a self-referencing field (e.g., Category { children: Vec<Category> })
        // Extract the inner type from Vec<T> or Option<T> and check if it matches the API struct name
        let inner_type = extract_api_struct_type_for_recursive_call(&field.ty);
        let _inner_type_string = inner_type.to_string();
        let _api_struct_name_string = api_struct_name.to_string();
        // Check for exact match (not substring) to avoid false positives like VehiclePart matching Vehicle
        let is_self_referencing = self_referencing(&field.ty, api_struct_name);

        // Security: Cap depth to prevent infinite recursion and performance issues
        // - Regular joins: Max depth 5 (MAX_JOIN_DEPTH)
        // - Self-referencing: MUST use depth=1 (load immediate children only, no recursion)
        let effective_depth = if is_self_referencing {
            let original_depth = join_config.depth.unwrap_or(1).min(MAX_JOIN_DEPTH);
            if original_depth > 1 {
                let error_msg = format!(
                    "Self-referencing field '{field_name}' in struct '{api_struct_name}' has depth={original_depth}, but self-references only support depth=1 to prevent exponential query growth. Please change to: join(one, depth = 1)"
                );
                return quote! { compile_error!(#error_msg); };
            }
            1 // Always use depth=1 for self-referencing (no recursive loading)
        } else {
            join_config
                .depth
                .unwrap_or(MAX_JOIN_DEPTH)
                .min(MAX_JOIN_DEPTH)
        };

        // For self-referencing fields, we use Entity::find().filter() instead of find_related()
        // Self-referencing fields are ALWAYS depth-limited (depth=1) to prevent exponential growth
        // Regular fields use recursive loading when depth > 1
        let depth_limited = effective_depth == 1;

        // Get entity path and model path (custom or derived from type)
        let (entity_path, model_path) = if let Some(custom_path) = &join_config.path {
            if let Ok(path_tokens) = custom_path.parse::<proc_macro2::TokenStream>() {
                (
                    quote! { #path_tokens::Entity },
                    quote! { #path_tokens::Model },
                )
            } else {
                // Generate a compile error if the path is invalid
                let error_msg = format!(
                    "Invalid join path '{custom_path}' for field '{field_name}'. Expected a valid Rust module path."
                );
                return quote! { compile_error!(#error_msg); };
            }
        } else {
            (
                get_path_from_field_type(&field.ty, "Entity"),
                get_path_from_field_type(&field.ty, "Model"),
            )
        };

        let (fk_column_pascal, _fk_field_snake, use_runtime_join) =
            derive_fk_idents(&join_config, api_struct_name, is_self_referencing);
        let relation_def = relation_def_expr(&join_config, &field.ty, &entity_path);

        // When scoped, derive the child's {StructName}List type path so we can
        // reuse its ScopeFilterable::scope_condition() for both SQL-level
        // filtering (Vec fields) and for recursing via get_one_scoped at
        // depth > 1.
        let child_list_type_path = if scoped {
            Some(list_type_of_child(field))
        } else {
            None
        };

        // SQL-level scope filter for Vec children (applied before fetching).
        let scope_filter = if let (true, Some(child_list_type)) =
            (is_vec_field, &child_list_type_path)
        {
            quote! {
                // Apply child entity's scope condition (if any) at the SQL level
                let query = if let Some(child_scope) = <#child_list_type as crudcrate::ScopeFilterable>::scope_condition() {
                    query.filter(child_scope)
                } else {
                    query
                };
            }
        } else {
            quote! {}
        };

        if is_vec_field {
            // Vec<T> relationships (has_many)
            let api_struct_type = extract_api_struct_type_for_recursive_call(&field.ty);

            if depth_limited {
                let loaded_var = quote::format_ident!("loaded_{}", field_name);
                let column_path = get_path_from_field_type(&field.ty, "Column");

                let filter_expr = if use_runtime_join {
                    quote! {
                        let __rel_def = #relation_def;
                        let __fk_col_name = sea_orm::Iden::to_string(&__rel_def.from_col);
                        let query = #entity_path::find()
                            .filter(sea_orm::sea_query::Expr::col(
                                sea_orm::sea_query::Alias::new(&__fk_col_name)
                            ).eq(<Self as crudcrate::traits::CRUDResource>::pk_value(&model)));
                    }
                } else {
                    quote! {
                        let query = #entity_path::find()
                            .filter(#column_path::#fk_column_pascal.eq(<Self as crudcrate::traits::CRUDResource>::pk_value(&model)));
                    }
                };

                loading_statements.push(quote! {
                    let #loaded_var: Vec<#api_struct_type> = {
                        use sea_orm::{EntityTrait, ExprTrait, QueryFilter, ColumnTrait};
                        #filter_expr
                        #scope_filter
                        let __profile = <Self as crudcrate::traits::CRUDResource>::security_profile();
                        let query = match __profile.child_row_limit() {
                            Some(__l) => sea_orm::QuerySelect::limit(query, __l),
                            None => query,
                        };
                        let related_models = Box::pin(query.all(db)).await?;
                        __profile.check_child_rows(
                            related_models.len(),
                            <Self as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR,
                            #field_name_str,
                        )?;
                        related_models
                            .into_iter()
                            .map(|m: #model_path| #api_struct_type::from(m))
                            .collect::<Vec<_>>()
                    };
                });
                field_assignments.push(quote! { result.#field_name = #loaded_var; });
            } else {
                // Depth > 1: Recursive loading.
                // - Scoped: fetch via get_one_scoped with the child's own scope condition
                //   so grandchildren remain filtered.
                // - Unscoped: use get_one (existing behaviour).
                let column_path = get_path_from_field_type(&field.ty, "Column");
                let recursive_fetch = if let Some(child_list_type) = &child_list_type_path {
                    quote! {
                        let __child_scope = <#child_list_type as crudcrate::ScopeFilterable>::scope_condition();
                        match __child_scope {
                            Some(cs) => match <#api_struct_type as crudcrate::traits::CRUDResource>::get_one_scoped(db, <#api_struct_type as crudcrate::traits::CRUDResource>::pk_value(&related_model), &cs).await {
                                Ok(entity) => result.push(entity),
                                Err(e) => {
                                    crudcrate::tracing::warn!(error = %e, "Failed to load nested scoped relations, using flat model");
                                    result.push(related_model.into());
                                }
                            },
                            None => match <#api_struct_type as crudcrate::traits::CRUDResource>::get_one(db, <#api_struct_type as crudcrate::traits::CRUDResource>::pk_value(&related_model)).await {
                                Ok(entity) => result.push(entity),
                                Err(e) => {
                                    crudcrate::tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                    result.push(related_model.into());
                                }
                            },
                        }
                    }
                } else {
                    quote! {
                        match <#api_struct_type as crudcrate::traits::CRUDResource>::get_one(db, <#api_struct_type as crudcrate::traits::CRUDResource>::pk_value(&related_model)).await {
                            Ok(entity) => result.push(entity),
                            Err(e) => {
                                crudcrate::tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                result.push(related_model.into());
                            }
                        }
                    }
                };

                let filter_expr_deep = if use_runtime_join {
                    quote! {
                        let __rel_def = #relation_def;
                        let __fk_col_name = sea_orm::Iden::to_string(&__rel_def.from_col);
                        let query = #entity_path::find()
                            .filter(sea_orm::sea_query::Expr::col(
                                sea_orm::sea_query::Alias::new(&__fk_col_name)
                            ).eq(<Self as crudcrate::traits::CRUDResource>::pk_value(&model)));
                    }
                } else {
                    quote! {
                        let query = #entity_path::find()
                            .filter(#column_path::#fk_column_pascal.eq(<Self as crudcrate::traits::CRUDResource>::pk_value(&model)));
                    }
                };

                loading_statements.push(quote! {
                    let #field_name: Vec<#api_struct_type> = {
                        use sea_orm::{EntityTrait, ExprTrait, QueryFilter, ColumnTrait};
                        #filter_expr_deep
                        #scope_filter
                        let __profile = <Self as crudcrate::traits::CRUDResource>::security_profile();
                        let query = match __profile.child_row_limit() {
                            Some(__l) => sea_orm::QuerySelect::limit(query, __l),
                            None => query,
                        };
                        let related_models = Box::pin(query.all(db)).await?;
                        __profile.check_child_rows(
                            related_models.len(),
                            <Self as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR,
                            #field_name_str,
                        )?;
                        let mut result = Vec::new();
                        for related_model in related_models {
                            #recursive_fetch
                        }
                        result
                    };
                });
                field_assignments.push(quote! { result.#field_name = #field_name; });
            }
        } else {
            // Option<T> or T relationships (belongs_to/has_one)
            // Use find_related() here (wrapped in Box::pin for stack safety) because
            // the FK direction varies: belongs_to has FK on self, has_one has FK on related.
            // find_related() resolves this correctly via the Related<E> trait definition.
            let target_type = extract_option_or_direct_inner_type(&field.ty);

            if depth_limited {
                // Depth=1: Load data, no recursion
                let loaded_var = quote::format_ident!("loaded_{}", field_name);
                loading_statements.push(quote! {
                    let #loaded_var = Box::pin(
                        model.find_related(#entity_path).one(db)
                    ).await?
                    .map(|m: #model_path| #target_type::from(m));
                });
                field_assignments.push(quote! {
                    result.#field_name = #loaded_var;
                });
            } else {
                // Unlimited depth: Recursive loading.
                // Scoped paths use get_one_scoped with the child's own scope condition.
                let recursive_option_fetch = if let Some(child_list_type) = &child_list_type_path {
                    quote! {
                        let __child_scope = <#child_list_type as crudcrate::ScopeFilterable>::scope_condition();
                        match __child_scope {
                            Some(cs) => match <#target_type as crudcrate::traits::CRUDResource>::get_one_scoped(db, <#target_type as crudcrate::traits::CRUDResource>::pk_value(&related_model), &cs).await {
                                Ok(entity) => Some(entity),
                                Err(e) => {
                                    crudcrate::tracing::warn!(error = %e, "Failed to load nested scoped relations, using flat model");
                                    Some(related_model.into())
                                }
                            },
                            None => match <#target_type as crudcrate::traits::CRUDResource>::get_one(db, <#target_type as crudcrate::traits::CRUDResource>::pk_value(&related_model)).await {
                                Ok(entity) => Some(entity),
                                Err(e) => {
                                    crudcrate::tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                    Some(related_model.into())
                                }
                            },
                        }
                    }
                } else {
                    quote! {
                        match <#target_type as crudcrate::traits::CRUDResource>::get_one(db, <#target_type as crudcrate::traits::CRUDResource>::pk_value(&related_model)).await {
                            Ok(entity) => Some(entity),
                            Err(e) => {
                                crudcrate::tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                Some(related_model.into())
                            }
                        }
                    }
                };

                loading_statements.push(quote! {
                    let #field_name = match Box::pin(
                        model.find_related(#entity_path).one(db)
                    ).await? {
                        Some(related_model) => { #recursive_option_fetch }
                        None => None,
                    };
                });
                field_assignments.push(quote! {
                    result.#field_name = #field_name;
                });
            }
        }
    }

    // Both contexts return Self directly (not wrapped in Result)
    // The caller is responsible for wrapping in Ok() when needed
    quote! {
        #( #loading_statements )*
        let mut result: Self = model.into();
        #( #field_assignments )*
        result
    }
}
