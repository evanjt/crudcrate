//! Pure syn helpers: type inspection, path rewriting and ident derivation.

use convert_case::{Case, Casing};
use heck::ToPascalCase;
use quote::{ToTokens, format_ident, quote};

/// Map field types to their corresponding entity or model paths
/// This function replaces both `get_entity_path_from_field_type` and `get_model_path_from_field_type`
pub(crate) fn get_path_from_field_type(
    field_type: &syn::Type,
    target_suffix: &str,
) -> proc_macro2::TokenStream {
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
pub(crate) fn extract_api_struct_type_for_recursive_call(
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

/// Transform a field type to use the List variant of its inner API struct.
///
/// Appends "List" to the last path segment of the inner type, preserving the full
/// module path and any Vec/Option wrapper.
///
/// Examples:
/// - `Vec<VehiclePart>` → `Vec<VehiclePartList>`
/// - `Vec<crate::isolates::db::Isolate>` → `Vec<crate::isolates::db::IsolateList>`
/// - `Option<Site>` → `Option<SiteList>`
///
/// For self-referencing joins (where the inner type matches `self_api_struct_name`),
/// returns the original type unchanged.
pub(crate) fn transform_type_to_list_variant(
    ty: &syn::Type,
    self_api_struct_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    // Check for self-referencing
    let inner_type = extract_api_struct_type_for_recursive_call(ty);
    let inner_str = inner_type.to_string();
    let self_str = self_api_struct_name.to_string();
    if inner_str.trim() == self_str.trim() {
        return quote! { #ty };
    }

    // Unwrap Vec/Option to get the inner type, transform it, and re-wrap
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && (segment.ident == "Vec" || segment.ident == "Option")
        && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
        && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
    {
        let list_inner = append_list_to_type(inner_ty);
        let wrapper = &segment.ident;
        return quote! { #wrapper<#list_inner> };
    }

    // Not wrapped - transform directly
    append_list_to_type(ty)
}

/// Derive the List type path for a given type, using module path resolution
/// to ensure the List type is always reachable.
///
/// For fully qualified paths (e.g., `crate::isolates::db::Isolate`), appends "List"
/// to the last segment: `crate::isolates::db::IsolateList`.
///
/// For short names (e.g., `VehiclePart`), uses `get_path_from_field_type` to resolve
/// the module path: `super::vehicle_part::VehiclePartList`.
fn append_list_to_type(ty: &syn::Type) -> proc_macro2::TokenStream {
    if let syn::Type::Path(type_path) = ty {
        if type_path.path.segments.len() > 1 {
            // Fully qualified path - just append "List" to last segment
            let mut new_path = type_path.clone();
            if let Some(last_segment) = new_path.path.segments.last_mut() {
                let new_name = format!("{}List", last_segment.ident);
                last_segment.ident = syn::Ident::new(&new_name, last_segment.ident.span());
            }
            quote! { #new_path }
        } else if let Some(segment) = type_path.path.segments.last() {
            // Short name - resolve via get_path_from_field_type for a proper module path
            let list_name = format!("{}List", segment.ident);
            get_path_from_field_type(ty, &list_name)
        } else {
            quote! { #ty }
        }
    } else {
        quote! { #ty }
    }
}

/// Transform a field type to use the `ScopedList` variant of its inner API struct.
///
/// Like `transform_type_to_list_variant`, but appends "`ScopedList`" instead of "List".
/// Used in scoped models so that joined children also have their scoped fields excluded.
///
/// Examples:
/// - `Vec<Sample>` → `Vec<SampleScopedList>`
/// - `Vec<crate::isolates::db::Isolate>` → `Vec<crate::isolates::db::IsolateScopedList>`
pub(crate) fn transform_type_to_scoped_list_variant(
    ty: &syn::Type,
    self_api_struct_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    // Check for self-referencing
    let inner_type = extract_api_struct_type_for_recursive_call(ty);
    let inner_str = inner_type.to_string();
    let self_str = self_api_struct_name.to_string();
    if inner_str.trim() == self_str.trim() {
        return quote! { #ty };
    }

    // Unwrap Vec/Option to get the inner type, transform it, and re-wrap
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && (segment.ident == "Vec" || segment.ident == "Option")
        && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
        && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
    {
        let scoped_inner = append_suffix_to_type(inner_ty, "ScopedList");
        let wrapper = &segment.ident;
        return quote! { #wrapper<#scoped_inner> };
    }

    append_suffix_to_type(ty, "ScopedList")
}

/// Append an arbitrary suffix to the last segment of a type path.
fn append_suffix_to_type(ty: &syn::Type, suffix: &str) -> proc_macro2::TokenStream {
    if let syn::Type::Path(type_path) = ty {
        if type_path.path.segments.len() > 1 {
            let mut new_path = type_path.clone();
            if let Some(last_segment) = new_path.path.segments.last_mut() {
                let new_name = format!("{}{suffix}", last_segment.ident);
                last_segment.ident = syn::Ident::new(&new_name, last_segment.ident.span());
            }
            quote! { #new_path }
        } else if let Some(segment) = type_path.path.segments.last() {
            let name = format!("{}{suffix}", segment.ident);
            get_path_from_field_type(ty, &name)
        } else {
            quote! { #ty }
        }
    } else {
        quote! { #ty }
    }
}

/// For a Vec<T> type, return the "`TList`" inner type token (not wrapped in Vec).
/// Used when generating chained conversions for scoped response join fields.
pub(crate) fn inner_list_type_of_vec(ty: &syn::Type) -> proc_macro2::TokenStream {
    let inner = extract_vec_inner_type_ref(ty);
    append_suffix_to_type(inner, "List")
}

pub(crate) fn extract_option_or_direct_inner_type(ty: &syn::Type) -> proc_macro2::TokenStream {
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

/// The `Column` enum variant sea-orm generates for a field name. sea-orm
/// derives variants with heck, which does not split on digit boundaries
/// (`is_2fa_enabled` -> `Is2faEnabled`).
pub(crate) fn column_ident(field_name: &str) -> syn::Ident {
    format_ident!("{}", field_name.to_pascal_case())
}

pub(crate) fn is_vec_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && segment.ident == "Vec"
    {
        return true;
    }
    false
}

pub(crate) fn is_option_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && segment.ident == "Option"
    {
        return true;
    }
    false
}

/// For an Option<T> type, return the "`TList`" inner type token (not wrapped in Option).
pub(crate) fn inner_list_type_of_option(ty: &syn::Type) -> proc_macro2::TokenStream {
    let inner = extract_option_inner_type_ref(ty);
    append_suffix_to_type(inner, "List")
}

/// Extract inner type from Vec<T>, or return the type itself if not a Vec
/// This is the canonical implementation used across the codebase
/// Returns a reference to the inner `syn::Type`
pub(crate) fn extract_vec_inner_type_ref(ty: &syn::Type) -> &syn::Type {
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
/// Returns a reference to the inner `syn::Type`
pub(crate) fn extract_option_inner_type_ref(ty: &syn::Type) -> &syn::Type {
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

/// Helper function to handle raw identifiers properly by stripping the r# prefix
pub(crate) fn ident_to_string(ident: &syn::Ident) -> String {
    let ident_str = ident.to_string();
    if let Some(stripped) = ident_str.strip_prefix("r#") {
        stripped.to_string() // Strip "r#" prefix from raw identifiers
    } else {
        ident_str
    }
}

/// Check if a type is a text type (String or &str), handling Option<T> wrappers
pub(crate) fn is_text_type(ty: &syn::Type) -> bool {
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

/// Returns true if the field's type is `Option<…>` (including `std::option::Option<…>`).
pub(crate) fn field_is_optional(field: &syn::Field) -> bool {
    is_option_type(&field.ty)
}

/// Resolves the target models (Create/Update) for a field with `use_target_models` attribute.
/// Returns (`CreateModel`, `UpdateModel`) types for the target `CRUDResource`.
pub(crate) fn resolve_target_models(field_type: &syn::Type) -> Option<(syn::Type, syn::Type)> {
    // Extract the inner type if it's Vec<T>
    let target_type = extract_vec_inner_type_ref(field_type);

    // Convert target type to Create and Update models
    // For example: crate::path::to::models::Entity -> (EntityCreate, EntityUpdate)
    if let syn::Type::Path(type_path) = target_type
        && let Some(last_seg) = type_path.path.segments.last()
    {
        let base_name = &last_seg.ident;

        // Keep the module path but replace the struct name
        let mut create_path = type_path.clone();
        let mut update_path = type_path.clone();

        // Update the last segment to be the Create/Update versions
        if let Some(last_seg_mut) = create_path.path.segments.last_mut() {
            last_seg_mut.ident = quote::format_ident!("{}Create", base_name);
        }
        if let Some(last_seg_mut) = update_path.path.segments.last_mut() {
            last_seg_mut.ident = quote::format_ident!("{}Update", base_name);
        }

        let create_model = syn::Type::Path(create_path);
        let update_model = syn::Type::Path(update_path);

        return Some((create_model, update_model));
    }
    None
}

/// Resolves `DateTimeWithTimeZone` to `chrono::DateTime<chrono::FixedOffset>` in a type.
///
/// `SeaORM`'s `DateTimeWithTimeZone` is a type alias for `chrono::DateTime<chrono::FixedOffset>`,
/// but utoipa's `ToSchema` derive only recognizes `DateTime` (the bare ident), not the alias.
/// This function rewrites the type so utoipa's chrono feature can recognize it, while keeping
/// the same underlying Rust type (no runtime conversion needed).
///
/// Returns the original token stream unchanged if `DateTimeWithTimeZone` is not present.
pub(crate) fn resolve_dtwtz(ty: &impl ToTokens) -> proc_macro2::TokenStream {
    let type_str = ty.to_token_stream().to_string();
    if !type_str.contains("DateTimeWithTimeZone") {
        return ty.to_token_stream();
    }
    let resolved = type_str.replace(
        "DateTimeWithTimeZone",
        "chrono::DateTime<chrono::FixedOffset>",
    );
    syn::parse_str::<syn::Type>(&resolved).map_or_else(|_| ty.to_token_stream(), |t| quote! { #t })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;
    use syn::parse_quote;

    #[test]
    fn test_field_is_optional_with_option_type() {
        let field: syn::Field = parse_quote! { pub field: Option<String> };
        assert!(field_is_optional(&field));
    }

    #[test]
    fn test_field_is_optional_with_std_option() {
        let field: syn::Field = parse_quote! { pub field: std::option::Option<i32> };
        assert!(field_is_optional(&field));
    }

    #[test]
    fn test_field_is_optional_with_non_option_type() {
        let field: syn::Field = parse_quote! { pub field: String };
        assert!(!field_is_optional(&field));
    }

    #[test]
    fn test_field_is_optional_with_vec() {
        let field: syn::Field = parse_quote! { pub field: Vec<String> };
        assert!(!field_is_optional(&field));
    }

    #[test]
    fn test_resolve_target_models_simple_type() {
        let field_type: syn::Type = parse_quote! { Entity };
        let result = resolve_target_models(&field_type);

        assert!(result.is_some());
        let (create, update) = result.unwrap();

        // Verify the model names are correct
        let create_str = quote!(#create).to_string();
        let update_str = quote!(#update).to_string();

        assert!(create_str.contains("EntityCreate"));
        assert!(update_str.contains("EntityUpdate"));
    }

    #[test]
    fn test_resolve_target_models_vec_type() {
        let field_type: syn::Type = parse_quote! { Vec<Product> };
        let result = resolve_target_models(&field_type);

        assert!(result.is_some());
        let (create, update) = result.unwrap();

        let create_str = quote!(#create).to_string();
        let update_str = quote!(#update).to_string();

        assert!(create_str.contains("ProductCreate"));
        assert!(update_str.contains("ProductUpdate"));
    }

    #[test]
    fn test_resolve_target_models_with_path() {
        let field_type: syn::Type = parse_quote! { crate::entities::Customer };
        let result = resolve_target_models(&field_type);

        assert!(result.is_some());
        let (create, update) = result.unwrap();

        let create_str = quote!(#create).to_string();
        let update_str = quote!(#update).to_string();

        // Should preserve the path
        assert!(create_str.contains("crate :: entities"));
        assert!(create_str.contains("CustomerCreate"));
        assert!(update_str.contains("CustomerUpdate"));
    }

    #[test]
    fn test_resolve_target_models_invalid_type() {
        // Non-path types should return None
        let field_type: syn::Type = parse_quote! { &str };
        let result = resolve_target_models(&field_type);
        assert!(result.is_none());
    }
}
