//! Foreign key naming convention and the join depth cap.

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
            quote::format_ident!("{}_id", to_snake_case(&api_struct_name.to_string())),
            true, // Use runtime resolution: convention may not match
        )
    }
}
