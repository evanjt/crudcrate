//! `join(...)` grammar: the field-level form and the struct-level `join(name = .., result = ..)` form.

use crate::ir::StructLevelJoin;
use syn::{Lit, Meta, parse::Parser, punctuated::Punctuated, token::Comma};

/// Configuration for join behavior on a field
#[derive(Debug, Clone, Default)]
pub(crate) struct JoinConfig {
    pub on_one: bool,
    pub on_all: bool,
    pub depth: Option<u8>,
    pub relation: Option<String>,
    pub path: Option<String>,
    /// Columns on the joined entity that can be filtered via dot-notation (e.g., "vehicles.make")
    pub filterable_columns: Vec<String>,
    /// Columns on the joined entity that can be sorted via dot-notation (e.g., "vehicles.year")
    pub sortable_columns: Vec<String>,
    /// Explicit FK column name override (e.g., "`OwnerUuid`" instead of convention-derived "`CustomerId`")
    pub fk_column: Option<String>,
}

/// Result of parsing join config - may contain deprecation errors
pub(crate) struct JoinConfigResult {
    pub config: Option<JoinConfig>,
    pub errors: Vec<syn::Error>,
}

impl JoinConfigResult {
    /// Returns true if a join config was found (regardless of errors)
    pub fn is_some(&self) -> bool {
        self.config.is_some()
    }

    /// Check if join config exists and satisfies a predicate
    pub fn is_some_and<F: FnOnce(&JoinConfig) -> bool>(&self, f: F) -> bool {
        self.config.as_ref().is_some_and(f)
    }

    /// Unwrap the config or return default
    pub fn unwrap_or_default(self) -> JoinConfig {
        self.config.unwrap_or_default()
    }
}

/// Parses join configuration from a field's crudcrate attributes.
/// Looks for `#[crudcrate(join(...))]` syntax and extracts join parameters.
///
/// New syntax (supported):
///   `join(one, all, depth = 1, filterable("make", "year"), sortable("year"))`
///
/// Old syntax (emits compile error with migration instructions):
///   `join_filterable("make", "year")` - use `filterable(...)` inside `join()` instead
///   `join_sortable("year")` - use `sortable(...)` inside `join()` instead
pub(crate) fn get_join_config(field: &syn::Field) -> JoinConfigResult {
    let mut config: Option<JoinConfig> = None;
    let mut errors: Vec<syn::Error> = Vec::new();

    for attr in &field.attrs {
        if attr.path().is_ident("crudcrate")
            && let Meta::List(meta_list) = &attr.meta
            && let Ok(metas) =
                Punctuated::<Meta, Comma>::parse_terminated.parse2(meta_list.tokens.clone())
        {
            for meta in metas {
                match &meta {
                    Meta::List(list_meta) if list_meta.path.is_ident("join") => {
                        config = parse_join_parameters(list_meta);
                        if let Some(ref c) = config
                            && c.depth == Some(0)
                        {
                            errors.push(syn::Error::new_spanned(
                                list_meta,
                                "Join `depth = 0` is invalid (causes infinite recursion). \
                                     Use `depth = 1` for shallow loading.",
                            ));
                        }
                    }
                    Meta::List(list_meta) if list_meta.path.is_ident("join_filterable") => {
                        errors.push(create_join_attr_deprecation_error(
                            "join_filterable",
                            "filterable",
                            list_meta,
                        ));
                    }
                    Meta::List(list_meta) if list_meta.path.is_ident("join_sortable") => {
                        errors.push(create_join_attr_deprecation_error(
                            "join_sortable",
                            "sortable",
                            list_meta,
                        ));
                    }
                    _ => {}
                }
            }
        }
    }

    JoinConfigResult { config, errors }
}

/// Create an error for deprecated `join_filterable/join_sortable` syntax
fn create_join_attr_deprecation_error(
    old_attr: &str,
    new_attr: &str,
    meta_list: &syn::MetaList,
) -> syn::Error {
    let columns = parse_string_list(meta_list);
    let columns_str = columns
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ");

    syn::Error::new_spanned(
        meta_list,
        format!(
            "The `{old_attr}(...)` attribute has been removed.\n\
             Move it inside the `join(...)` attribute as `{new_attr}(...)`.\n\
             \n\
             Migration:\n\
             Before: #[crudcrate(join(one, all), {old_attr}({columns_str}))]\n\
             After:  #[crudcrate(join(one, all, {new_attr}({columns_str})))]\n\
             \n\
             Example with all options:\n\
             #[crudcrate(join(one, all, depth = 1, filterable(\"make\", \"year\"), sortable(\"year\")))]"
        ),
    )
}

/// Parse a list of string literals from an attribute like `join_filterable("col1", "col2")`
fn parse_string_list(meta_list: &syn::MetaList) -> Vec<String> {
    let mut result = Vec::new();

    // Try to parse the tokens as a list of expressions (string literals)
    if let Ok(exprs) =
        Punctuated::<syn::Expr, Comma>::parse_terminated.parse2(meta_list.tokens.clone())
    {
        for expr in exprs {
            if let syn::Expr::Lit(expr_lit) = expr
                && let Lit::Str(lit_str) = expr_lit.lit
            {
                result.push(lit_str.value());
            }
        }
    }

    result
}

/// Parses the parameters inside join(...) function call
///
/// Supports:
/// - Flags: `one`, `all`, `on_one`, `on_all`
/// - Named: `depth = 2`, `relation = "Name"`, `path = "crate::path"`
/// - Nested lists: `filterable("col1", "col2")`, `sortable("col1")`
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
                                Lit::Str(str_lit) if nv.path.is_ident("fk_column") => {
                                    config.fk_column = Some(str_lit.value());
                                }
                                _ => {}
                            }
                        }
                    }
                    // Parse nested lists: filterable("col1", "col2"), sortable("col1")
                    Meta::List(nested_list) => {
                        if nested_list.path.is_ident("filterable") {
                            config.filterable_columns = parse_string_list(&nested_list);
                        } else if nested_list.path.is_ident("sortable") {
                            config.sortable_columns = parse_string_list(&nested_list);
                        }
                    }
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

/// Parse a struct-level `join(name = "field", result = "Type", one, all, depth = N)` attribute.
/// Returns `Some` only if both `name` and `result` are present (distinguishing from field-level joins).
pub(crate) fn parse_struct_level_join(
    meta_list: &syn::MetaList,
) -> Option<crate::ir::StructLevelJoin> {
    use crate::ir::StructLevelJoin;

    let mut name = None;
    let mut result_type = None;
    let mut on_one = false;
    let mut on_all = false;
    let mut depth = None;
    let mut relation = None;
    let mut path = None;
    let mut filterable_columns = Vec::new();
    let mut sortable_columns = Vec::new();
    let mut fk_column = None;
    let metas = Punctuated::<Meta, Comma>::parse_terminated
        .parse2(meta_list.tokens.clone())
        .ok()?;

    for meta in metas {
        match meta {
            Meta::NameValue(nv) => {
                if let syn::Expr::Lit(expr_lit) = &nv.value {
                    match &expr_lit.lit {
                        Lit::Str(s) => {
                            if nv.path.is_ident("name") {
                                name = Some(s.value());
                            } else if nv.path.is_ident("result") {
                                result_type = Some(s.value());
                            } else if nv.path.is_ident("relation") {
                                relation = Some(s.value());
                            } else if nv.path.is_ident("path") {
                                path = Some(s.value());
                            } else if nv.path.is_ident("fk_column") {
                                fk_column = Some(s.value());
                            }
                        }
                        Lit::Int(i) if nv.path.is_ident("depth") => {
                            depth = i.base10_parse().ok();
                        }
                        _ => {}
                    }
                }
            }
            Meta::Path(p) => {
                if p.is_ident("one") || p.is_ident("on_one") {
                    on_one = true;
                } else if p.is_ident("all") || p.is_ident("on_all") {
                    on_all = true;
                }
            }
            Meta::List(nested) => {
                if nested.path.is_ident("filterable") {
                    filterable_columns = parse_string_list(&nested);
                } else if nested.path.is_ident("sortable") {
                    sortable_columns = parse_string_list(&nested);
                }
            }
        }
    }

    let name = name?;
    let result_type = result_type?;
    if !on_one && !on_all {
        return None;
    }

    Some(StructLevelJoin {
        name,
        result_type,
        on_one,
        on_all,
        depth,
        relation,
        path,
        filterable_columns,
        sortable_columns,
        fk_column,
    })
}

/// The synthetic field a struct-level `join(name = .., result = ..)` adds to the API struct.
/// It exists only on the generated API struct, not on the Sea-ORM model.
pub(crate) fn synthetic_join_field(
    j: &StructLevelJoin,
    input: &syn::DeriveInput,
) -> Result<Option<syn::Field>, proc_macro2::TokenStream> {
    let mut parts = Vec::new();
    if j.on_one {
        parts.push("one".to_string());
    }
    if j.on_all {
        parts.push("all".to_string());
    }
    if let Some(d) = j.depth {
        parts.push(format!("depth = {d}"));
    }
    if let Some(ref r) = j.relation {
        parts.push(format!("relation = \"{r}\""));
    }
    if let Some(ref p) = j.path {
        parts.push(format!("path = \"{p}\""));
    }
    if !j.filterable_columns.is_empty() {
        let cols = j
            .filterable_columns
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(", ");
        parts.push(format!("filterable({cols})"));
    }
    if !j.sortable_columns.is_empty() {
        let cols = j
            .sortable_columns
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(", ");
        parts.push(format!("sortable({cols})"));
    }
    if let Some(ref fk) = j.fk_column {
        parts.push(format!("fk_column = \"{fk}\""));
    }
    let join_attr = parts.join(", ");
    let struct_str = format!(
        "struct S {{ #[sea_orm(ignore)] #[crudcrate(non_db_attr, exclude(create, update), join({join_attr}))] pub {}: {} }}",
        j.name, j.result_type
    );
    match syn::parse_str::<syn::ItemStruct>(&struct_str) {
        Ok(s) => {
            if let syn::Fields::Named(named) = s.fields
                && let Some(field) = named.named.into_iter().next()
            {
                return Ok(Some(field));
            }
            Ok(None)
        }
        Err(e) => Err(syn::Error::new_spanned(
            input,
            format!("Invalid struct-level join '{}': {e}", j.name),
        )
        .to_compile_error()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn field_from(attr: &proc_macro2::TokenStream) -> syn::Field {
        let input: syn::DeriveInput = syn::parse2(quote! {
            pub struct Model {
                #attr
                pub children: Vec<Child>,
            }
        })
        .expect("parse struct");
        match input.data {
            syn::Data::Struct(s) => s.fields.into_iter().next().expect("one field"),
            _ => unreachable!(),
        }
    }

    fn messages(result: &JoinConfigResult) -> Vec<String> {
        result
            .errors
            .iter()
            .map(std::string::ToString::to_string)
            .collect()
    }

    #[test]
    fn test_join_parses_every_parameter() {
        let result = get_join_config(&field_from(&quote! {
            #[crudcrate(non_db_attr, join(
                one,
                all,
                depth = 3,
                relation = "Children",
                path = "super::child",
                fk_column = "OwnerUuid",
                filterable("make", "year"),
                sortable("year")
            ))]
        }));
        assert!(result.errors.is_empty(), "{:?}", messages(&result));
        let config = result.unwrap_or_default();
        assert!(config.on_one);
        assert!(config.on_all);
        assert_eq!(config.depth, Some(3));
        assert_eq!(config.relation.as_deref(), Some("Children"));
        assert_eq!(config.path.as_deref(), Some("super::child"));
        assert_eq!(config.fk_column.as_deref(), Some("OwnerUuid"));
        assert_eq!(config.filterable_columns, ["make", "year"]);
        assert_eq!(config.sortable_columns, ["year"]);
    }

    /// `depth = 0` would recurse without a base case, so it is rejected at the
    /// attribute rather than expanded into a loader.
    #[test]
    fn test_join_depth_zero_is_rejected() {
        let result = get_join_config(&field_from(&quote! {
            #[crudcrate(non_db_attr, join(one, depth = 0))]
        }));
        let messages = messages(&result);
        assert_eq!(messages.len(), 1);
        assert!(
            messages[0].contains("depth = 0"),
            "error must name the offending depth, got {messages:?}"
        );
    }

    /// The removed `join_filterable` / `join_sortable` attributes name their
    /// replacement and echo the columns, so the migration is mechanical.
    #[test]
    fn test_legacy_join_attributes_report_their_replacement() {
        for (legacy, replacement) in [
            ("join_filterable", "filterable"),
            ("join_sortable", "sortable"),
        ] {
            let ident = syn::Ident::new(legacy, proc_macro2::Span::call_site());
            let result = get_join_config(&field_from(&quote! {
                #[crudcrate(non_db_attr, #ident("make", "year"))]
            }));
            let messages = messages(&result);
            assert_eq!(messages.len(), 1, "{legacy} must report one error");
            assert!(
                messages[0].contains(replacement)
                    && messages[0].contains("\"make\"")
                    && messages[0].contains("\"year\""),
                "{legacy} error must name {replacement} and echo the columns, got {messages:?}"
            );
        }
    }

    #[test]
    fn test_field_without_join_has_no_config() {
        let result = get_join_config(&field_from(&quote! {
            #[crudcrate(non_db_attr)]
        }));
        assert!(result.config.is_none());
        assert!(result.errors.is_empty());
    }
}
