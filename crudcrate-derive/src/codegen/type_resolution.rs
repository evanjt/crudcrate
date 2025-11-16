use convert_case::{Case, Casing};
use heck::ToPascalCase;
use quote::{format_ident, quote};

use crate::{CRUDResourceMeta, attribute_parser::get_crudcrate_bool};

/// Map field types to their corresponding entity or model paths
/// This function replaces both get_entity_path_from_field_type and get_model_path_from_field_type
pub fn get_path_from_field_type(field_type: &syn::Type, target_suffix: &str) -> proc_macro2::TokenStream {
    // Extract the target type from Vec<T> or Option<T> using canonical helpers
    let target_type = extract_vec_inner_type_ref(field_type);
    let target_type = extract_option_inner_type_ref(target_type);

    // Handle fully qualified paths like crate::path::to::module::StructName
    if let syn::Type::Path(type_path) = target_type {
        if type_path.path.segments.len() > 1 {
            // For paths like crate::path::to::module::StructName
            // Convert to crate::sites::replicates::db::{Entity|Model}
            let mut path_segments = type_path.path.segments.clone();
            if let Some(last_segment) = path_segments.last_mut() {
                // Replace the last segment (e.g., StructName) with target
                last_segment.ident = syn::Ident::new(target_suffix, last_segment.ident.span());
                let modified_path = syn::Path {
                    leading_colon: type_path.path.leading_colon,
                    segments: path_segments,
                };
                return quote! { #modified_path };
            }
        } else if let Some(segment) = type_path.path.segments.last() {
            // Fallback: Convert TypeName to snake_case::{Entity|Model} for simple paths
            // Handle API struct aliases by stripping common suffixes
            let type_name = segment.ident.to_string();
            let base_name = if type_name.ends_with("API") {
                // Convert ModuleAPI → Module
                type_name.strip_suffix("API").unwrap_or(&type_name)
            } else {
                &type_name
            };
            let entity_name = base_name.to_case(Case::Snake);
            let path_name = format_ident!("{}", entity_name);
            let target_ident = syn::Ident::new(target_suffix, proc_macro2::Span::call_site());
            return quote! { super::#path_name::#target_ident };
        }
    }

    let target_ident = syn::Ident::new(target_suffix, proc_macro2::Span::call_site());
    quote! { #target_ident } // Fallback
}

/// Extract the API struct type for recursive `get_one()` calls from field types
/// Recursively unwraps Vec/Option wrappers and handles Join type aliases
pub fn extract_api_struct_type_for_recursive_call(
    field_type: &syn::Type,
) -> proc_macro2::TokenStream {
    // Recursively unwrap Vec and Option wrappers using canonical helpers
    let mut current_type = field_type;
    loop {
        let unwrapped_vec = extract_vec_inner_type_ref(current_type);
        let unwrapped_option = extract_option_inner_type_ref(unwrapped_vec);

        // If no more unwrapping happened, we've reached the inner type
        if std::ptr::eq(unwrapped_option, current_type) {
            break;
        }
        current_type = unwrapped_option;
    }

    // Handle type aliases that end with "Join" (ModuleJoin -> Module)
    if let syn::Type::Path(type_path) = current_type
        && let Some(segment) = type_path.path.segments.last()
    {
        let type_name = segment.ident.to_string();
        if type_name.ends_with("Join") {
            let base_name = type_name.strip_suffix("Join").unwrap_or(&type_name);
            return quote! { #base_name };
        }
    }

    // Return the fully unwrapped type
    quote! { #current_type }
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

/// Extract inner type from Vec<T>, or return the type itself if not a Vec
/// This is the canonical implementation used across the codebase
/// Returns a reference to the inner syn::Type
pub fn extract_vec_inner_type_ref(ty: &syn::Type) -> &syn::Type {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && segment.ident == "Vec"
        && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
        && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
    {
        return inner_ty;
    }
    ty
}

/// Extract inner type from Option<T>, or return the type itself if not an Option
/// Returns a reference to the inner syn::Type
pub fn extract_option_inner_type_ref(ty: &syn::Type) -> &syn::Type {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && segment.ident == "Option"
        && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
        && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
    {
        return inner_ty;
    }
    ty
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
