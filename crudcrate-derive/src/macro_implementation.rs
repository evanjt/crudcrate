use super::attribute_parser::{get_crudcrate_bool, get_join_config};
use super::structs::{CRUDResourceMeta, EntityFieldAnalysis};
use crate::codegen::{
    handler::{create, delete, get, update},
    type_resolution::{
        extract_api_struct_type_for_recursive_call, extract_option_or_direct_inner_type,
        extract_vec_inner_type, generate_crud_type_aliases, generate_enum_field_checker,
        generate_field_entries, generate_fulltext_field_entries, generate_id_column,
        generate_like_filterable_entries, get_entity_path_from_field_type,
        get_model_path_from_field_type, is_vec_type, resolve_join_type_globally,
    },
};
use heck::ToPascalCase;
use quote::{format_ident, quote};

/// Filters fields that should be included in update model
pub(crate) fn filter_update_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<&syn::Field> {
    fields
        .iter()
        .filter(|field| {
            // Exclude fields from update model if update_model = false
            let include_in_update = get_crudcrate_bool(field, "update_model").unwrap_or(true);

            // Exclude join fields entirely from Update models - they're populated by recursive loading
            let is_join_field = get_join_config(field).is_some();

            include_in_update && !is_join_field
        })
        .collect()
}

pub(crate) fn generate_crud_resource_impl(
    api_struct_name: &syn::Ident,
    crud_meta: &CRUDResourceMeta,
    active_model_path: &str,
    analysis: &EntityFieldAnalysis,
    table_name: &str,
) -> proc_macro2::TokenStream {
    let (
        create_model_name,
        update_model_name,
        list_model_name,
        entity_type,
        column_type,
        active_model_type,
    ) = generate_crud_type_aliases(api_struct_name, crud_meta, active_model_path);

    let id_column = generate_id_column(analysis.primary_key_field);
    let sortable_entries = generate_field_entries(&analysis.sortable_fields);
    let filterable_entries = generate_field_entries(&analysis.filterable_fields);
    let like_filterable_entries = generate_like_filterable_entries(&analysis.filterable_fields);
    let fulltext_entries = generate_fulltext_field_entries(&analysis.fulltext_fields);
    let enum_field_checker = generate_enum_field_checker(&analysis.db_fields);
    let name_singular = crud_meta.name_singular.as_deref().unwrap_or("resource");
    let description = crud_meta.description.as_deref().unwrap_or("");
    let fulltext_language = crud_meta.fulltext_language.as_deref().unwrap_or("english");

    let (get_one_impl, get_all_impl, create_impl, update_impl, delete_impl, delete_many_impl) =
        generate_method_impls(crud_meta, analysis);

    // Generate registration lazy static and auto-registration call only for models without join fields
    // Models with join fields may have circular dependencies that prevent CRUDResource compilation
    let has_join_fields =
        !analysis.join_on_one_fields.is_empty() || !analysis.join_on_all_fields.is_empty();

    let (registration_static, auto_register_call) = if has_join_fields {
        // Skip registration for models with join fields to avoid circular dependency issues
        (quote! {}, quote! {})
    } else {
        (
            quote! {
                // Lazy static that ensures registration happens on first trait usage
                static __REGISTER_LAZY: std::sync::LazyLock<()> = std::sync::LazyLock::new(|| {
                    crudcrate::register_analyser::<#api_struct_name>();
                });
            },
            quote! {
                std::sync::LazyLock::force(&__REGISTER_LAZY);
            },
        )
    };

    // Generate resource name plural constant
    let resource_name_plural_impl = {
        let name_plural = crud_meta.name_plural.clone().unwrap_or_default();
        quote! {
            const RESOURCE_NAME_PLURAL: &'static str = #name_plural;
        }
    };

    quote! {
        #registration_static

        #[async_trait::async_trait]
        impl crudcrate::CRUDResource for #api_struct_name {
            type EntityType = #entity_type;
            type ColumnType = #column_type;
            type ActiveModelType = #active_model_type;
            type CreateModel = #create_model_name;
            type UpdateModel = #update_model_name;
            type ListModel = #list_model_name;

            const ID_COLUMN: Self::ColumnType = #id_column;
            const RESOURCE_NAME_SINGULAR: &'static str = #name_singular;
            #resource_name_plural_impl
            const TABLE_NAME: &'static str = #table_name;
            const RESOURCE_DESCRIPTION: &'static str = #description;
            const FULLTEXT_LANGUAGE: &'static str = #fulltext_language;

            fn sortable_columns() -> Vec<(&'static str, Self::ColumnType)> {
                #auto_register_call
                vec![#(#sortable_entries),*]
            }

            fn filterable_columns() -> Vec<(&'static str, Self::ColumnType)> {
                #auto_register_call
                vec![#(#filterable_entries),*]
            }

            fn is_enum_field(field_name: &str) -> bool {
                #enum_field_checker
            }

            fn like_filterable_columns() -> Vec<&'static str> {
                vec![#(#like_filterable_entries),*]
            }

            fn fulltext_searchable_columns() -> Vec<(&'static str, Self::ColumnType)> {
                #auto_register_call
                vec![#(#fulltext_entries),*]
            }

            #get_one_impl
            #get_all_impl
            #create_impl
            #update_impl
            #delete_impl
            #delete_many_impl
        }

    }
}

/// Generates join loading statements for direct queries (all join fields regardless of one/all flags)
pub fn generate_join_loading_for_direct_query(
    analysis: &EntityFieldAnalysis,
) -> Vec<proc_macro2::TokenStream> {
    let mut statements = Vec::new();

    // Process ALL join fields for direct queries (both one and all)
    let mut all_join_fields = analysis.join_on_one_fields.clone();
    all_join_fields.extend(analysis.join_on_all_fields.iter());

    // Remove duplicates (in case a field has both join(one) and join(all))
    all_join_fields.sort_by_key(|f| {
        f.ident
            .as_ref()
            .map(std::string::ToString::to_string)
            .unwrap_or_default()
    });
    all_join_fields.dedup_by_key(|f| {
        f.ident
            .as_ref()
            .map(std::string::ToString::to_string)
            .unwrap_or_default()
    });

    // Generate loading statements for all join fields
    for field in &all_join_fields {
        if let Some(field_name) = &field.ident {
            let join_config = get_join_config(field).unwrap_or_default();
            let _depth = join_config.depth.unwrap_or(3);

            // Generate code to load related entities for this field
            let relation_name = if let Some(custom_relation) = &join_config.relation {
                format_ident!("{}", custom_relation)
            } else {
                format_ident!("{}", field_name.to_string().to_pascal_case())
            };

            // Generate the entity path based on custom path or default super:: prefix
            let entity_path = if let Some(custom_path) = &join_config.path {
                // Parse custom path string into a token stream
                let path_tokens: proc_macro2::TokenStream = custom_path.parse().unwrap();
                quote! { #path_tokens::Entity }
            } else {
                quote! { super::#relation_name::Entity }
            };

            // Check if this is a Vec<T> field or a single T field by analyzing the type
            let is_vec_field = is_vec_type(&field.ty);

            if is_vec_field {
                // Generate code for Vec<T> fields (has_many relationships)
                let loading_stmt = quote! {
                    // Load related entities for #field_name field
                    if let Ok(related_models) = model.find_related(#entity_path).all(db).await {
                        // Convert related models to API structs (recursive loading happens via their own joins)
                        let mut related_with_joins = Vec::new();
                        for related_model in related_models {
                            let related_api_struct = related_model.into();
                            related_with_joins.push(related_api_struct);
                        }
                        result.#field_name = related_with_joins;
                    }
                };
                statements.push(loading_stmt);
            } else {
                // Generate code for single T or Option<T> fields (belongs_to/has_one relationships)
                let loading_stmt = quote! {
                    // Load related entity for #field_name field
                    if let Ok(Some(related_model)) = model.find_related(#entity_path).one(db).await {
                        result.#field_name = Some(related_model.into());
                    }
                };
                statements.push(loading_stmt);
            }
        }
    }

    statements
}

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
                get_entity_path_from_field_type(&field.ty)
            };
            let _model_path = get_model_path_from_field_type(&field.ty);

            // Check if this join should stop recursion at this level
            // depth=1 means "load this level but don't recurse into nested joins"
            // depth=2+ means "load this level AND recurse into nested joins"
            // None means "unlimited recursion"
            let stop_recursion = join_config.depth == Some(1);
            let _should_recurse = join_config.depth.is_none() || join_config.depth.unwrap_or(1) > 1;

            if is_vec_field {
                let _inner_type = if false
                    && let Some(resolved_tokens) = resolve_join_type_globally(&field.ty)
                {
                    if let Ok(resolved_type) = syn::parse2::<syn::Type>(resolved_tokens) {
                        if let syn::Type::Path(type_path) = &resolved_type {
                            if let Some(segment) = type_path.path.segments.last()
                                && segment.ident == "Vec"
                                && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
                                && let Some(syn::GenericArgument::Type(inner_ty)) =
                                    args.args.first()
                            {
                                inner_ty.clone()
                            } else {
                                // Not Vec<T>, use the resolved type directly
                                resolved_type
                            }
                        } else {
                            syn::parse2::<syn::Type>(extract_vec_inner_type(&field.ty))
                                .unwrap_or_else(|_| field.ty.clone()) // Fallback
                        }
                    } else {
                        syn::parse2::<syn::Type>(extract_vec_inner_type(&field.ty))
                            .unwrap_or_else(|_| field.ty.clone()) // Fallback
                    }
                } else {
                    syn::parse2::<syn::Type>(extract_vec_inner_type(&field.ty))
                        .unwrap_or_else(|_| field.ty.clone())
                };

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

fn generate_method_impls(
    crud_meta: &CRUDResourceMeta,
    analysis: &EntityFieldAnalysis,
) -> (
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
) {
    let get_one_impl = get::generate_get_one_impl(crud_meta, analysis);
    let get_all_impl = get::generate_get_all_impl(crud_meta, analysis);
    let create_impl = create::generate_create_impl(crud_meta, analysis);
    let update_impl = update::generate_update_impl(crud_meta, analysis);
    let delete_impl = delete::generate_delete_impl(crud_meta);
    let delete_many_impl = delete::generate_delete_many_impl(crud_meta);

    (
        get_one_impl,
        get_all_impl,
        create_impl,
        update_impl,
        delete_impl,
        delete_many_impl,
    )
}
