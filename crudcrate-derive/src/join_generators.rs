use super::structs::EntityFieldAnalysis;
use convert_case::{Case, Casing};
use quote::{format_ident, quote};

/// Helper function to determine if a type is Vec<T>
pub fn is_vec_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Vec";
        }
    false
}

/// Generates join loading statements for direct queries (all join fields regardless of one/all flags)  
/// NOTE: This is a placeholder implementation - full join functionality is still under development
pub fn generate_join_loading_for_direct_query(_analysis: &EntityFieldAnalysis) -> Vec<proc_macro2::TokenStream> {
    // For now, return empty statements until proper relation loading is implemented
    vec![]
}

/// Generate single-level join loading implementation (no complex recursion for now)
/// NOTE: This is a simplified implementation - complex join loading is still under development

pub fn generate_recursive_loading_implementation(analysis: &EntityFieldAnalysis) -> proc_macro2::TokenStream {
    
    // Check if there are any join fields at all
    if analysis.join_on_one_fields.is_empty() && analysis.join_on_all_fields.is_empty() {
        return quote! {
            Ok(model.into())
        };
    }
    
    // Generate generic single-level loading for all join fields
    let mut loading_statements = Vec::new();
    let mut field_assignments = Vec::new();
    
    // Process ALL join fields for direct queries (both one and all)
    let mut all_join_fields = analysis.join_on_one_fields.clone();
    all_join_fields.extend(analysis.join_on_all_fields.iter());
    
    // Remove duplicates (in case a field has both join(one) and join(all))
    all_join_fields.sort_by_key(|f| f.ident.as_ref().map(std::string::ToString::to_string).unwrap_or_default());
    all_join_fields.dedup_by_key(|f| f.ident.as_ref().map(std::string::ToString::to_string).unwrap_or_default());
    
    for field in &all_join_fields {
        if let Some(field_name) = &field.ident {
            let is_vec_field = is_vec_type(&field.ty);
            let entity_path = get_entity_path_from_field_type(&field.ty);
            
            if is_vec_field {
                // Extract the inner type from Vec<T> to get the related entity name
                let inner_type = extract_vec_inner_type(&field.ty);
                
                // For Vec<T> fields (has_many relationships)
                loading_statements.push(quote! {
                    let related_models = model.find_related(#entity_path).all(db).await.unwrap_or_default();
                    let mut #field_name = Vec::new();
                    for related_model in related_models {
                        // Use recursive get_one to respect join loading for nested entities
                        match #inner_type::get_one(db, related_model.id).await {
                            Ok(loaded_entity) => #field_name.push(loaded_entity),
                            Err(_) => #field_name.push(related_model.into()), // Fallback to simple conversion
                        }
                    }
                });
                field_assignments.push(quote! { result.#field_name = #field_name; });
            } else {
                // Extract the inner type from Option<T> or T
                let inner_type = extract_option_or_direct_inner_type(&field.ty);
                
                // For single T or Option<T> fields (belongs_to/has_one relationships)
                loading_statements.push(quote! {
                    let #field_name = if let Ok(Some(related_model)) = model.find_related(#entity_path).one(db).await {
                        // Use recursive get_one to respect join loading for nested entities
                        match #inner_type::get_one(db, related_model.id).await {
                            Ok(loaded_entity) => Some(loaded_entity),
                            Err(_) => Some(related_model.into()), // Fallback to simple conversion
                        }
                    } else {
                        None
                    };
                });
                field_assignments.push(quote! { result.#field_name = #field_name; });
            }
        }
    }
    
    quote! {
        // Load all join fields (single level only for now)
        #(#loading_statements)*
        
        // Create result struct with loaded join data
        let mut result: Self = model.into();
        #(#field_assignments)*
        
        Ok(result)
    }
}

/// Extract Vec inner type for relationship analysis
pub fn extract_vec_inner_type(ty: &syn::Type) -> proc_macro2::TokenStream {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Vec" {
                if let syn::PathArguments::AngleBracketed(angle_bracketed) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_type)) = angle_bracketed.args.first() {
                        return quote! { #inner_type };
                    }
                }
            }
        }
    }
    quote! { () }
}

/// Extract Option or direct inner type for relationship analysis
pub fn extract_option_or_direct_inner_type(ty: &syn::Type) -> proc_macro2::TokenStream {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(angle_bracketed) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_type)) = angle_bracketed.args.first() {
                        return quote! { #inner_type };
                    }
                }
            }
        }
    }
    // Not an Option, return the type directly
    quote! { #ty }
}

/// Generate join loading statements for all join(all) fields with recursive depth support
pub fn generate_join_loading_for_all_fields(
    join_fields: &[&syn::Field], 
    join_configs: &std::collections::HashMap<&syn::Field, super::attribute_parser::JoinConfig>
) -> proc_macro2::TokenStream {
    let mut loading_statements = Vec::new();

    // Load all join data BEFORE converting the model
    for field in join_fields {
        if let Some(field_name) = &field.ident {
            let is_vec_field = is_vec_type(&field.ty);
            let entity_path = get_entity_path_from_field_type(&field.ty);
            
            // Get join configuration for this field
            let default_config = Default::default();
            let config = join_configs.get(field).unwrap_or(&default_config);
            let depth = config.depth.unwrap_or(1);
            
            if is_vec_field {
                // For Vec<T> fields (has_many relationships)
                if depth > 1 {
                    // Recursive loading: load related models, then call their get_one method for deeper loading
                    let target_type = get_target_type_from_field(&field.ty);
                    loading_statements.push(quote! {
                        let #field_name: Vec<_> = {
                            let related_models = sea_orm::ModelTrait::find_related(&model, #entity_path).all(db).await
                                .unwrap_or_default();
                            let mut loaded_items = Vec::new();
                            for related_model in related_models {
                                // For recursive loading with depth > 1, call the target's get_one method
                                // This assumes the related model has an 'id' field (common case)
                                if let Ok(fully_loaded) = #target_type::get_one(db, related_model.id).await {
                                    loaded_items.push(fully_loaded);
                                } else {
                                    // Fallback to basic conversion if recursive loading fails
                                    loaded_items.push(related_model.into());
                                }
                            }
                            loaded_items
                        };
                    });
                } else {
                    // Single-level loading (current implementation)
                    loading_statements.push(quote! {
                        let #field_name: Vec<_> = sea_orm::ModelTrait::find_related(&model, #entity_path).all(db).await
                            .unwrap_or_default()
                            .into_iter().map(|related_model| related_model.into()).collect();
                    });
                }
            } else {
                // For single T or Option<T> fields (belongs_to/has_one relationships)  
                if depth > 1 {
                    // Recursive loading for single fields
                    let target_type = get_target_type_from_field(&field.ty);
                    loading_statements.push(quote! {
                        let #field_name = {
                            if let Some(related_model) = sea_orm::ModelTrait::find_related(&model, #entity_path).one(db).await.ok().flatten() {
                                // For recursive loading, call the target's get_one method
                                if let Ok(fully_loaded) = #target_type::get_one(db, related_model.id).await {
                                    Some(fully_loaded)
                                } else {
                                    Some(related_model.into())
                                }
                            } else {
                                None
                            }
                        };
                    });
                } else {
                    // Single-level loading
                    loading_statements.push(quote! {
                        let #field_name = sea_orm::ModelTrait::find_related(&model, #entity_path).one(db).await.ok()
                            .flatten().map(|related_model| related_model.into());
                    });
                }
            }
        }
    }
    
    // AFTER loading all join data, create the result struct and assign fields
    loading_statements.push(quote! {
        let mut loaded_model: Self = model.into();
    });
    
    // Assign all loaded join fields
    for field in join_fields {
        if let Some(field_name) = &field.ident {
            loading_statements.push(quote! {
                loaded_model.#field_name = #field_name;
            });
        }
    }

    quote! {
        #(#loading_statements)*
    }
}

/// Get target type from field for relationship analysis
pub fn get_target_type_from_field(field_type: &syn::Type) -> proc_macro2::TokenStream {
    // Extract inner type from Vec<T>, Option<T>, or T
    if let syn::Type::Path(type_path) = field_type {
        if let Some(segment) = type_path.path.segments.last() {
            match segment.ident.to_string().as_str() {
                "Vec" => {
                    if let syn::PathArguments::AngleBracketed(angle_bracketed) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_type)) = angle_bracketed.args.first() {
                            return quote! { #inner_type };
                        }
                    }
                }
                "Option" => {
                    if let syn::PathArguments::AngleBracketed(angle_bracketed) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_type)) = angle_bracketed.args.first() {
                            return quote! { #inner_type };
                        }
                    }
                }
                _ => return quote! { #field_type },
            }
        }
    }
    quote! { #field_type }
}

/// Get entity path from field type for relationship loading
pub fn get_entity_path_from_field_type(field_type: &syn::Type) -> proc_macro2::TokenStream {
    let _target_type = get_target_type_from_field(field_type);
    
    // Assume the entity is in a module with the same name as the type (lowercase)
    // This is a common pattern: Customer type -> customer::Entity
    if let syn::Type::Path(type_path) = field_type {
        if let Some(segment) = type_path.path.segments.last() {
            let type_name = segment.ident.to_string();
            let module_name = type_name.to_case(Case::Snake);
            let module_ident = format_ident!("{}", module_name);
            return quote! { super::#module_ident::Entity };
        }
    }
    
    quote! { super::Entity }
}