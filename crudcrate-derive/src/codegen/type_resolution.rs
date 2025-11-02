use convert_case::{Case, Casing};
use heck::ToPascalCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Type;

use crate::{CRUDResourceMeta, attribute_parser::get_crudcrate_bool};

/// Map field types to their corresponding entity paths
pub fn get_entity_path_from_field_type(field_type: &syn::Type) -> proc_macro2::TokenStream {
    let unwrapped_type = field_type;

    // Then, resolve the field type using the global registry to handle type aliases
    let resolved_type = if let Some(resolved_tokens) = resolve_join_type_globally(unwrapped_type) {
        if let Ok(parsed_type) = syn::parse2::<syn::Type>(resolved_tokens) {
            parsed_type
        } else {
            unwrapped_type.clone()
        }
    } else {
        unwrapped_type.clone()
    };

    // Extract the target type from Vec<T> or Option<T>
    let target_type = if let syn::Type::Path(type_path) = &resolved_type {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Vec" {
                // Vec<T> - extract T
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        inner_ty
                    } else {
                        field_type
                    }
                } else {
                    field_type
                }
            } else if segment.ident == "Option" {
                // Option<T> - extract T
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        inner_ty
                    } else {
                        field_type
                    }
                } else {
                    field_type
                }
            } else {
                // T (direct type)
                field_type
            }
        } else {
            field_type
        }
    } else {
        field_type
    };

    // Handle fully qualified paths like crate::path::to::module::StructName
    if let syn::Type::Path(type_path) = target_type {
        if type_path.path.segments.len() > 1 {
            // For paths like crate::path::to::module::StructName
            // Convert to crate::sites::replicates::db::Entity
            let mut path_segments = type_path.path.segments.clone();
            if let Some(last_segment) = path_segments.last_mut() {
                // Replace the last segment (e.g., StructName) with Entity
                last_segment.ident = syn::Ident::new("Entity", last_segment.ident.span());
                let modified_path = syn::Path {
                    leading_colon: type_path.path.leading_colon,
                    segments: path_segments,
                };
                return quote! { #modified_path };
            }
        } else if let Some(segment) = type_path.path.segments.last() {
            // Fallback: Convert TypeName to snake_case for simple paths
            // Handle API struct aliases by stripping common suffixes
            let type_name = segment.ident.to_string();
            let base_name = if type_name.ends_with("API") {
                // Convert ModuleAPI → Module
                type_name.strip_suffix("API").unwrap_or(&type_name)
            } else {
                &type_name
            };
            let entity_name = base_name.to_case(Case::Snake);
            let entity_path = format_ident!("{}", entity_name);
            return quote! { super::#entity_path::Entity };
        }
    }

    quote! { Entity } // Fallback
}

/// Map field types to their corresponding model paths
pub fn get_model_path_from_field_type(field_type: &syn::Type) -> proc_macro2::TokenStream {
    let unwrapped_type = field_type;

    // Then, resolve the field type using the global registry to handle type aliases
    let resolved_type = if let Some(resolved_tokens) = resolve_join_type_globally(unwrapped_type) {
        if let Ok(parsed_type) = syn::parse2::<syn::Type>(resolved_tokens) {
            parsed_type
        } else {
            unwrapped_type.clone()
        }
    } else {
        unwrapped_type.clone()
    };

    // Extract the target type from Vec<T> or Option<T>
    let target_type = if let syn::Type::Path(type_path) = &resolved_type {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Vec" {
                // Vec<T> - extract T
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        inner_ty
                    } else {
                        field_type
                    }
                } else {
                    field_type
                }
            } else if segment.ident == "Option" {
                // Option<T> - extract T
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        inner_ty
                    } else {
                        field_type
                    }
                } else {
                    field_type
                }
            } else {
                // T (direct type)
                field_type
            }
        } else {
            field_type
        }
    } else {
        field_type
    };

    // Handle fully qualified paths like crate::path::to::module::StructName
    if let syn::Type::Path(type_path) = target_type {
        if type_path.path.segments.len() > 1 {
            // For paths like crate::path::to::module::StructName
            // Convert to crate::sites::replicates::db::Model
            let mut path_segments = type_path.path.segments.clone();
            if let Some(last_segment) = path_segments.last_mut() {
                // Replace the last segment (e.g., StructName) with Model
                last_segment.ident = syn::Ident::new("Model", last_segment.ident.span());
                let modified_path = syn::Path {
                    leading_colon: type_path.path.leading_colon,
                    segments: path_segments,
                };
                return quote! { #modified_path };
            }
        } else if let Some(segment) = type_path.path.segments.last() {
            // Fallback: Convert TypeName to snake_case::Model for simple paths
            // Handle API struct aliases by stripping common suffixes
            let type_name = segment.ident.to_string();
            let base_name = if type_name.ends_with("API") {
                // Convert ModuleAPI → Module
                type_name.strip_suffix("API").unwrap_or(&type_name)
            } else {
                &type_name
            };
            let entity_name = base_name.to_case(Case::Snake);
            let model_path = format_ident!("{}", entity_name);
            return quote! { super::#model_path::Model };
        }
    }

    quote! { Model } // Fallback
}

/// Extract the API struct type for recursive `get_one()` calls from field types
pub fn extract_api_struct_type_for_recursive_call(
    field_type: &syn::Type,
) -> proc_macro2::TokenStream {
    fn extract_inner_type_from_type(ty: &syn::Type) -> proc_macro2::TokenStream {
        if let syn::Type::Path(type_path) = ty
            && let Some(segment) = type_path.path.segments.last()
            && (segment.ident == "Vec" || segment.ident == "Option")
            && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
            && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
        {
            return extract_inner_type_from_type(inner_ty);
        }
        if let syn::Type::Path(type_path) = ty {
            // Extract the module path (everything except the last segment "Model")
            let path_segments: Vec<_> = type_path
                .path
                .segments
                .iter()
                .take(type_path.path.segments.len().saturating_sub(1)) // All except last
                .collect();

            if !path_segments.is_empty()
                && type_path
                    .path
                    .segments
                    .last()
                    .is_some_and(|s| s.ident == "Model")
                && let Some(base_type_str) = extract_base_type_string(ty)
                && let Some(api_name) = find_api_struct_name(&base_type_str)
            {
                // We have a path like super::module::Model
                // Extract the base type and get the API struct name
                let api_ident = quote::format_ident!("{}", api_name);

                return quote! { #(#path_segments)::*::#api_ident };
            }
        }

        quote! { #ty } // Fallback: return the type as-is
    }

    let resolved_type = if let Some(resolved_tokens) = resolve_join_type_globally(field_type) {
        if let Ok(parsed_type) = syn::parse2::<syn::Type>(resolved_tokens) {
            if let syn::Type::Path(ref type_path) = parsed_type {
                if let Some(segment) = type_path.path.segments.last() {
                    if (segment.ident == "Vec" || segment.ident == "Option")
                        && matches!(segment.arguments, syn::PathArguments::None)
                    {
                        field_type.clone()
                    } else {
                        parsed_type
                    }
                } else {
                    parsed_type
                }
            } else {
                parsed_type
            }
        } else {
            field_type.clone()
        }
    } else {
        field_type.clone()
    };

    // Now extract the inner type from the resolved type
    if let syn::Type::Path(type_path) = &resolved_type
        && let Some(segment) = type_path.path.segments.last()
    {
        let type_name = segment.ident.to_string();

        if (segment.ident == "Vec" || segment.ident == "Option")
            && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
            && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
        {
            let inner_type = extract_inner_type_from_type(inner_ty);

            return inner_type;
        }

        // Handle type aliases that end with "Join" (ModuleJoin -> Module)
        // This handles cases where the type alias wasn't resolved to Vec<T> properly
        if type_name.ends_with("Join") {
            let base_name = type_name.strip_suffix("Join").unwrap_or(&type_name);
            let api_struct_name = base_name; // Most API structs have the same name as the entity
            return quote! { #api_struct_name };
        }

        // For direct types, use them as-is
        return quote! { #resolved_type };
    }

    // Fallback: extract inner type from the original field type directly
    extract_inner_type_from_type(field_type)
}

pub fn extract_vec_inner_type(ty: &syn::Type) -> proc_macro2::TokenStream {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && segment.ident == "Vec"
        && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
        && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
    {
        return quote! { #inner_ty };
    }
    quote! { () } // Fallback
}

pub fn extract_option_or_direct_inner_type(ty: &syn::Type) -> proc_macro2::TokenStream {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && segment.ident == "Option"
        && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
        && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
    {
        return quote! { #inner_ty };
    }
    quote! { #ty }
}
pub fn is_vec_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && segment.ident == "Vec"
    {
        return true;
    }
    false
}

pub fn generate_crud_type_aliases(
    api_struct_name: &syn::Ident,
    crud_meta: &CRUDResourceMeta,
    active_model_path: &str,
) -> (
    syn::Ident,
    syn::Ident,
    syn::Ident,
    syn::Type,
    syn::Type,
    syn::Type,
) {
    let create_model_name = format_ident!("{}Create", api_struct_name);
    let update_model_name = format_ident!("{}Update", api_struct_name);
    let list_model_name = format_ident!("{}List", api_struct_name);

    let entity_type: syn::Type = crud_meta
        .entity_type
        .as_ref()
        .and_then(|s| syn::parse_str(s).ok())
        .unwrap_or_else(|| syn::parse_quote!(Entity));

    let column_type: syn::Type = crud_meta
        .column_type
        .as_ref()
        .and_then(|s| syn::parse_str(s).ok())
        .unwrap_or_else(|| syn::parse_quote!(Column));

    let active_model_type: syn::Type =
        syn::parse_str(active_model_path).unwrap_or_else(|_| syn::parse_quote!(ActiveModel));

    (
        create_model_name,
        update_model_name,
        list_model_name,
        entity_type,
        column_type,
        active_model_type,
    )
}

pub fn generate_id_column(primary_key_field: Option<&syn::Field>) -> proc_macro2::TokenStream {
    if let Some(pk_field) = primary_key_field {
        let field_name = &pk_field.ident.as_ref().unwrap();
        let column_name = format_ident!("{}", ident_to_string(field_name).to_pascal_case());
        quote! { Self::ColumnType::#column_name }
    } else {
        quote! { Self::ColumnType::Id }
    }
}

pub fn generate_field_entries(fields: &[&syn::Field]) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let field_str = ident_to_string(field_name);
            let column_name = format_ident!("{}", field_str.to_pascal_case());
            quote! { (#field_str, Self::ColumnType::#column_name) }
        })
        .collect()
}

pub fn generate_like_filterable_entries(fields: &[&syn::Field]) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter_map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let field_str = ident_to_string(field_name);

            // Check if this field should use LIKE queries based on its type
            if is_text_type(&field.ty) {
                Some(quote! { #field_str })
            } else {
                None
            }
        })
        .collect()
}

pub fn generate_fulltext_field_entries(fields: &[&syn::Field]) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let field_str = ident_to_string(field_name);
            let column_name = format_ident!("{}", field_str.to_pascal_case());
            quote! { (#field_str, Self::ColumnType::#column_name) }
        })
        .collect()
}

/// Generate enum field checker using explicit annotations only
/// Users must mark enum fields with `#[crudcrate(enum_field)]` for enum filtering to work
pub fn generate_enum_field_checker(all_fields: &[&syn::Field]) -> proc_macro2::TokenStream {
    let field_checks: Vec<proc_macro2::TokenStream> = all_fields
        .iter()
        .filter_map(|field| {
            if let Some(field_name) = &field.ident {
                let field_name_str = ident_to_string(field_name);
                let is_enum = get_crudcrate_bool(field, "enum_field").unwrap_or(false);

                Some(quote! {
                    #field_name_str => #is_enum,
                })
            } else {
                None
            }
        })
        .collect();

    quote! {
        match field_name {
            #(#field_checks)*
            _ => false,
        }
    }
}

/// Helper function to handle raw identifiers properly by stripping the r# prefix
pub fn ident_to_string(ident: &syn::Ident) -> String {
    let ident_str = ident.to_string();
    if let Some(stripped) = ident_str.strip_prefix("r#") {
        stripped.to_string() // Strip "r#" prefix from raw identifiers
    } else {
        ident_str
    }
}

/// Check if a type is a text type (String or &str), handling Option<T> wrappers
pub fn is_text_type(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(type_path) => {
            if let Some(last_seg) = type_path.path.segments.last() {
                let ident = &last_seg.ident;

                // Handle Option<T> - check the inner type
                if ident == "Option"
                    && let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments
                    && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
                {
                    return is_text_type(inner_ty);
                }

                // Check if it's String (could be std::string::String or just String)
                ident == "String"
            } else {
                false
            }
        }
        syn::Type::Reference(type_ref) => {
            // Check if it's &str
            if let syn::Type::Path(path) = &*type_ref.elem {
                path.path.is_ident("str")
            } else {
                false
            }
        }
        _ => false,
    }
}

// Stub functions - these will be removed in Phase 3 when we extract common patterns
pub fn resolve_join_type_globally(_field_type: &Type) -> Option<TokenStream> {
    None // No complex resolution needed - types are used as-written
}

pub fn extract_base_type_string(_field_type: &Type) -> Option<String> {
    None // No base type extraction needed - types are explicit
}

pub fn find_api_struct_name(_base_type: &str) -> Option<String> {
    None // No API struct lookup needed - types are explicit
}
