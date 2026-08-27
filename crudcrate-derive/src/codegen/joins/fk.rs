//! Foreign key naming convention and the join depth cap.

use crate::attrs::JoinConfig;
use crate::syn_type::{extract_api_struct_type_for_recursive_call, get_path_from_field_type};
use quote::quote;

// Security: Maximum join depth to prevent infinite recursion and resource exhaustion
// Users cannot exceed this limit - values > 5 are automatically capped
pub(crate) const MAX_JOIN_DEPTH: u8 = 5;

/// Convert `PascalCase` to `snake_case`
pub(crate) fn to_snake_case(s: &str) -> String {
    use convert_case::{Case, Casing};
    s.to_case(Case::Snake)
}

/// Derive FK column identifiers for a join field.
///
/// Returns `(pascal_ident, snake_ident)`, e.g., `(CustomerId, customer_id)`.
///
/// Resolution order:
/// 1. Explicit `fk_column = "..."` from join config (highest priority)
/// 2. Self-referencing: `ParentId` / `parent_id`
/// 3. Convention: `{ParentStructName}Id` / `{parent_struct_name}_id`
///
/// Returns `(fk_column_pascal, fk_field_snake, use_runtime)`.
/// When `use_runtime` is true, the FK column should be resolved from
/// `RelationDef` at runtime instead of using the static identifiers.
pub(crate) fn derive_fk_idents(
    join_config: &crate::attrs::JoinConfig,
    api_struct_name: &syn::Ident,
    is_self_referencing: bool,
) -> (proc_macro2::Ident, proc_macro2::Ident, bool) {
    if let Some(ref fk) = join_config.fk_column {
        (
            quote::format_ident!("{}", fk),
            quote::format_ident!("{}", to_snake_case(fk)),
            false,
        )
    } else if is_self_referencing {
        (
            quote::format_ident!("ParentId"),
            quote::format_ident!("parent_id"),
            false,
        )
    } else {
        (
            quote::format_ident!("{}Id", api_struct_name),
            quote::format_ident!("{}", fk_field_name(api_struct_name)),
            true, // Use runtime resolution: convention may not match
        )
    }
}

/// `{Child}List` path for a join field: the list model of the field's inner type.
pub(crate) fn list_type_of_child(field: &syn::Field) -> proc_macro2::TokenStream {
    let inner_type_string = extract_api_struct_type_for_recursive_call(&field.ty).to_string();
    let struct_name = inner_type_string
        .split("::")
        .last()
        .unwrap_or(&inner_type_string)
        .trim();
    get_path_from_field_type(&field.ty, &format!("{struct_name}List"))
}

/// Exact match of the field's inner type against the API struct, so `VehiclePart` does not
/// count as self-referencing for `Vehicle`.
pub(crate) fn self_referencing(field_ty: &syn::Type, api_struct_name: &syn::Ident) -> bool {
    extract_api_struct_type_for_recursive_call(field_ty)
        .to_string()
        .trim()
        == api_struct_name.to_string().trim()
}

/// Convention foreign key field on the child: `Customer` -> `customer_id`.
pub(crate) fn fk_field_name(api_struct_name: &syn::Ident) -> String {
    format!("{}_id", to_snake_case(&api_struct_name.to_string()))
}

/// `(Entity, Column)` paths of a join's child, honouring `path = ".."`.
pub(crate) fn child_paths(
    field: &syn::Field,
    field_name: &str,
    join_config: &JoinConfig,
) -> Result<(proc_macro2::TokenStream, proc_macro2::TokenStream), proc_macro2::TokenStream> {
    if let Some(custom_path) = &join_config.path {
        if let Ok(path_tokens) = custom_path.parse::<proc_macro2::TokenStream>() {
            Ok((
                quote! { #path_tokens::Entity },
                quote! { #path_tokens::Column },
            ))
        } else {
            let error_msg = format!("Invalid join path '{custom_path}' for field '{field_name}'");
            Err(quote! { compile_error!(#error_msg); })
        }
    } else {
        Ok((
            get_path_from_field_type(&field.ty, "Entity"),
            get_path_from_field_type(&field.ty, "Column"),
        ))
    }
}

/// Sub-query reference to the child FK column. Static and self-referencing joins use the
/// typed column; convention-derived joins resolve the name from the `RelationDef` at runtime.
pub(crate) fn fk_column_ref(
    is_self_referencing: bool,
    use_runtime_filter: bool,
    entity_path: &proc_macro2::TokenStream,
    column_path: &proc_macro2::TokenStream,
    fk_column_pascal: &syn::Ident,
) -> proc_macro2::TokenStream {
    if is_self_referencing || !use_runtime_filter {
        quote! {
            {
                let (__t, __c) = sea_orm::ColumnTrait::as_column_ref(
                    &#column_path::#fk_column_pascal
                );
                crudcrate::table_column_ref(__t, __c)
            }
        }
    } else {
        quote! {
            {
                let __rel_def = <#entity_path as sea_orm::Related<
                    <Self as crudcrate::traits::CRUDResource>::EntityType
                >>::to();
                let __fk_col_name = sea_orm::Iden::to_string(&__rel_def.from_col);
                let __child_tbl = sea_orm::EntityName::table_name(&#entity_path).to_string();
                crudcrate::table_column_ref(__child_tbl, __fk_col_name)
            }
        }
    }
}
