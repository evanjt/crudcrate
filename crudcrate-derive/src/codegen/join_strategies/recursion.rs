use crate::codegen::join_strategies::JoinConfig;
use crate::codegen::type_resolution::{
    extract_api_struct_type_for_recursive_call, extract_option_or_direct_inner_type,
    extract_vec_inner_type, get_path_from_field_type, is_vec_type,
};
use crate::traits::crudresource::structs::EntityFieldAnalysis;
use quote::quote;
use syn::{Lit, Meta, parse::Parser, punctuated::Punctuated, token::Comma};

/// Generate recursive join loading implementation for `get_one` method
#[allow(clippy::too_many_lines)]
pub fn generate_recursive_loading_implementation(
    analysis: &EntityFieldAnalysis,
) -> proc_macro2::TokenStream {
    // Check if there are any join fields for get_one
    // Fields with join(one) OR join(all) should appear in get_one() responses
    if analysis.join_on_one_fields.is_empty() && analysis.join_on_all_fields.is_empty() {
        return quote! {
            Ok(model.into())
        };
    }

    // Generate single-level loading for join(one) fields only
    let mut loading_statements = Vec::new();
    let mut field_assignments = Vec::new();

    // Process both join(one) and join(all) fields for get_one method
    // Fields with either join(one) or join(all) should appear in get_one() responses
    // IMPORTANT: Deduplicate fields - if a field has join(one, all), it appears in both lists
    let mut seen_fields = std::collections::HashSet::new();
    let all_join_fields_for_get_one: Vec<_> = analysis
        .join_on_one_fields
        .iter()
        .chain(analysis.join_on_all_fields.iter())
        .filter(|field| {
            if let Some(field_name) = &field.ident {
                seen_fields.insert(field_name.to_string())
            } else {
                true // Include fields without names (shouldn't happen)
            }
        })
        .collect();

    for field in all_join_fields_for_get_one {
        if let Some(field_name) = &field.ident {
            let join_config = get_join_config(field).unwrap_or_default();
            let is_vec_field = is_vec_type(&field.ty);

            // Extract entity and model paths from the field type or use custom path
            let entity_path = if let Some(custom_path) = &join_config.path {
                // Parse custom path string into a token stream
                let path_tokens: proc_macro2::TokenStream = custom_path.parse().unwrap();
                quote! { #path_tokens::Entity }
            } else {
                get_path_from_field_type(&field.ty, "Entity")
            };
            let _model_path = get_path_from_field_type(&field.ty, "Model");

            // Check if this join should stop recursion at this level
            // depth=1 means "load this level but don't recurse into nested joins"
            // depth=2+ means "load this level AND recurse into nested joins"
            // None means "unlimited recursion"
            let stop_recursion = join_config.depth == Some(1);
            let _should_recurse = join_config.depth.is_none() || join_config.depth.unwrap_or(1) > 1;

            if is_vec_field {
                let _inner_type = syn::parse2::<syn::Type>(extract_vec_inner_type(&field.ty))
                    .unwrap_or_else(|_| field.ty.clone());

                // For Vec<T> fields (has_many relationships) - depth-aware loading
                if stop_recursion {
                    // Depth-limited loading (depth=1) - Load data but don't recurse
                    // This prevents infinite recursion by converting Models directly to API structs
                    // without calling their get_one() methods (which would load nested joins)
                    let api_struct_type = extract_api_struct_type_for_recursive_call(&field.ty);
                    let loaded_var_name = quote::format_ident!("loaded_{}", field_name);
                    loading_statements.push(quote! {
                                        let related_models = model.find_related(#entity_path).all(db).await.unwrap_or_default();
                                        let #loaded_var_name: Vec<#api_struct_type> = related_models.into_iter()
                                            .map(|related_model| Into::<#api_struct_type>::into(related_model))
                                            .collect();
                                    });
                    field_assignments.push(quote! { result.#field_name = #loaded_var_name; });
                } else {
                    // Unlimited recursion - use the original recursive approach
                    let api_struct_type = extract_api_struct_type_for_recursive_call(&field.ty);
                    // Generate loading for unlimited recursion
                    loading_statements.push(quote! {
                                        let related_models = model.find_related(#entity_path).all(db).await.unwrap_or_default();
                                        let mut #field_name = Vec::new();
                                        for related_model in related_models {
                                            // Use recursive get_one to respect join loading for nested entities
                                            match #api_struct_type::get_one(db, related_model.id).await {
                                                Ok(loaded_entity) => #field_name.push(loaded_entity),
                                                Err(_) => {
                                                    // Fallback: convert Model to API struct using explicit Into::into
                                                    // The From<Model> impl is in the target module and should be found via imports
                                                    #field_name.push(Into::<#api_struct_type>::into(related_model))
                                                },
                                            }
                                        }
                                    });
                    field_assignments.push(quote! { result.#field_name = #field_name; });
                }
            } else {
                // Extract the inner type from Option<T> or T
                let inner_type = extract_option_or_direct_inner_type(&field.ty);

                // For single T or Option<T> fields (belongs_to/has_one relationships) - depth-aware loading
                if stop_recursion {
                    // Depth-limited loading (depth=1) - Load data but don't recurse
                    // This prevents infinite recursion by converting Models directly to API structs
                    // without calling their get_one() methods (which would load nested joins)
                    let loaded_var_name = quote::format_ident!("loaded_{}", field_name);
                    loading_statements.push(quote! {
                        let #loaded_var_name = if let Ok(Some(related_model)) = model.find_related(#entity_path).one(db).await {
                            Some(Into::<#inner_type>::into(related_model))
                        } else {
                            None
                        };
                    });
                    field_assignments.push(quote! {
                        result.#field_name = #loaded_var_name;
                    });
                } else {
                    // Unlimited recursion - use the original recursive approach
                    loading_statements.push(quote! {
                        let #field_name = if let Ok(Some(related_model)) = model.find_related(#entity_path).one(db).await {
                            // Use recursive get_one to respect join loading for nested entities
                            match #inner_type::get_one(db, related_model.id).await {
                                Ok(loaded_entity) => Some(loaded_entity),
                                Err(_) => {
                                    // Fallback: convert Model to API struct using explicit Into::into
                                    Some(Into::<#inner_type>::into(related_model))
                                },
                            }
                        } else {
                            None
                        };
                    });
                    field_assignments.push(quote! {
                        result.#field_name = #field_name;
                    });
                }
            }
        }
    }

    quote! {
        // Load all join fields with recursive loading
        #(#loading_statements)*

        // Create result struct with loaded join data
        let mut result: Self = model.into();
        #(#field_assignments)*

        Ok(result)
    }
}


/// Parses join configuration from a field's crudcrate attributes.
/// Looks for `#[crudcrate(join(...))]` syntax and extracts join parameters.
pub(crate) fn get_join_config(field: &syn::Field) -> Option<JoinConfig> {
    for attr in &field.attrs {
        if attr.path().is_ident("crudcrate")
            && let Meta::List(meta_list) = &attr.meta
            && let Ok(metas) =
                Punctuated::<Meta, Comma>::parse_terminated.parse2(meta_list.tokens.clone())
        {
            for meta in metas {
                if let Meta::List(list_meta) = meta
                    && list_meta.path.is_ident("join")
                {
                    return parse_join_parameters(&list_meta);
                }
            }
        }
    }
    None
}

/// Parses the parameters inside join(...) function call
fn parse_join_parameters(meta_list: &syn::MetaList) -> Option<JoinConfig> {
    let mut config = JoinConfig::default();

    // Try parsing the tokens - if it fails, just return None instead of panicking
    match Punctuated::<Meta, Comma>::parse_terminated.parse2(meta_list.tokens.clone()) {
        Ok(nested_metas) => {
            for meta in nested_metas {
                match meta {
                    // Parse flags: one, all, on_one, on_all
                    Meta::Path(path) => {
                        if path.is_ident("one") || path.is_ident("on_one") {
                            config.on_one = true;
                        } else if path.is_ident("all") || path.is_ident("on_all") {
                            config.on_all = true;
                        }
                    }
                    // Parse named parameters: depth = 2, relation = "CustomRelation", path = "crate::path::to::module"
                    Meta::NameValue(nv) => {
                        if let syn::Expr::Lit(expr_lit) = &nv.value {
                            match &expr_lit.lit {
                                Lit::Int(int_lit) if nv.path.is_ident("depth") => {
                                    if let Ok(depth_val) = int_lit.base10_parse::<u8>() {
                                        config.depth = Some(depth_val);
                                    }
                                }
                                Lit::Str(str_lit) if nv.path.is_ident("relation") => {
                                    config.relation = Some(str_lit.value());
                                }
                                Lit::Str(str_lit) if nv.path.is_ident("path") => {
                                    config.path = Some(str_lit.value());
                                }
                                _ => {}
                            }
                        }
                    }
                    Meta::List(_) => {}
                }
            }
        }
        Err(_) => {
            // If parsing fails, return None - don't fail the entire macro
            return None;
        }
    }

    // Only return config if at least one join type is enabled
    if config.on_one || config.on_all {
        Some(config)
    } else {
        None
    }
}
