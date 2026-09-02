//! Field-level `#[crudcrate(...)]` attributes: flags, expressions and `exclude(...)`.

use syn::parse::Parser;
use syn::{Lit, Meta, punctuated::Punctuated, token::Comma};

/// Given a field and a key (e.g. `"create_model"` or `"update_model"`),
/// look for a `#[crudcrate(...)]` attribute on the field and return the boolean value
/// associated with that key, if present.
///
/// Supports multiple syntaxes:
/// - `#[crudcrate(non_db_attr = true)]` (explicit boolean)
/// - `#[crudcrate(non_db_attr)]` (implicit true)
/// - `#[crudcrate(exclude_create)]` → `create_model = false` (individual aliases)
/// - `#[crudcrate(exclude(create, update))]` → both `create_model` and `update_model` = false
pub(crate) fn get_crudcrate_bool(field: &syn::Field, key: &str) -> Option<bool> {
    // First check for exclude() configuration (most idiomatic)
    if let Some(result) = check_exclude_config(field, key) {
        return Some(result); // check_exclude_config already returns the correct boolean for the model
    }

    for attr in &field.attrs {
        if attr.path().is_ident("crudcrate")
            && let Meta::List(meta_list) = &attr.meta
        {
            let metas: Punctuated<Meta, Comma> = Punctuated::parse_terminated
                .parse2(meta_list.tokens.clone())
                .ok()?;
            for meta in metas {
                match meta {
                    // Explicit boolean: key = true/false (with deprecation warning for model exclusion)
                    Meta::NameValue(nv) if nv.path.is_ident(key) => {
                        if let syn::Expr::Lit(expr_lit) = &nv.value
                            && let Lit::Bool(b) = &expr_lit.lit
                        {
                            // Deprecated: key = false (should use exclude(...) instead)
                            // Note: We keep this for backward compatibility but warn users to migrate
                            // Cannot use compile_error!() here as that would break existing code
                            // eprintln!() during macro expansion is the standard way to emit deprecation warnings
                            if (key == "create_model"
                                || key == "update_model"
                                || key == "list_model")
                                && !b.value()
                            {
                                // Emit visible deprecation warning during compilation
                                eprintln!(
                                    "\nDEPRECATION WARNING: {}\n",
                                    create_deprecation_error(key, &nv.path)
                                );
                            }
                            return Some(b.value());
                        }
                    }
                    // Implicit boolean flag: just `key` means true
                    Meta::Path(path) if path.is_ident(key) => {
                        return Some(true);
                    }
                    _ => {}
                }
            }
        }
    }
    None
}

/// Check if field has an exclude(...) configuration that affects the given key
fn check_exclude_config(field: &syn::Field, key: &str) -> Option<bool> {
    for attr in &field.attrs {
        if attr.path().is_ident("crudcrate")
            && let Meta::List(meta_list) = &attr.meta
            && let Ok(metas) =
                Punctuated::<Meta, Comma>::parse_terminated.parse2(meta_list.tokens.clone())
        {
            for meta in metas {
                if let Meta::List(list_meta) = meta
                    && list_meta.path.is_ident("exclude")
                    && let Some(is_excluded) = parse_exclude_parameters(&list_meta, key)
                {
                    return Some(!is_excluded); // If excluded, return false for the model
                }
            }
        }
    }
    None
}

/// Create a deprecation message for old model exclusion syntax
///
/// Note: Returns `syn::Error` for consistent formatting, but we extract the message
/// rather than using `to_compile_error()` to avoid breaking backward compatibility.
fn create_deprecation_error(key: &str, path: &syn::Path) -> syn::Error {
    let new_syntax = match key {
        "create_model" => "exclude(create)",
        "update_model" => "exclude(update)",
        "list_model" => "exclude(list)",
        "one_model" => "exclude(one)",
        _ => "exclude(...)",
    };

    syn::Error::new_spanned(
        path,
        format!(
            "The `{key} = false` syntax is deprecated. Use `{new_syntax}` instead for cleaner, more idiomatic code."
        ),
    )
}

/// Parse exclude(...) parameters to check if a specific model type is excluded
fn parse_exclude_parameters(meta_list: &syn::MetaList, target_key: &str) -> Option<bool> {
    if let Ok(nested_metas) =
        Punctuated::<Meta, Comma>::parse_terminated.parse2(meta_list.tokens.clone())
    {
        for meta in nested_metas {
            if let Meta::Path(path) = meta {
                // Check for exclude(all) which means both list_model and one_model should be false
                if path.is_ident("all") && (target_key == "list_model" || target_key == "one_model")
                {
                    return Some(true); // exclude(all) excludes from both list and one
                }

                let excluded_type = if path.is_ident("create") {
                    "create_model"
                } else if path.is_ident("update") {
                    "update_model"
                } else if path.is_ident("list") {
                    "list_model"
                } else if path.is_ident("one") {
                    "one_model"
                } else if path.is_ident("scoped") {
                    "scoped_model"
                } else {
                    continue;
                };

                if excluded_type == target_key {
                    return Some(true); // This model type is excluded
                }
            }
        }
    }
    None // exclude() was found but target_key wasn't in it, so no exclusion for this key
}

/// Given a field and a key (e.g. `"on_create"` or `"on_update"`), returns the expression
/// provided in the `#[crudcrate(...)]` attribute for that key.
pub(crate) fn get_crudcrate_expr(field: &syn::Field, key: &str) -> Option<syn::Expr> {
    for attr in &field.attrs {
        if attr.path().is_ident("crudcrate")
            && let Meta::List(meta_list) = &attr.meta
        {
            let metas: Punctuated<Meta, Comma> = Punctuated::parse_terminated
                .parse2(meta_list.tokens.clone())
                .ok()?;
            for meta in metas {
                if let Meta::NameValue(nv) = meta
                    && nv.path.is_ident(key)
                {
                    return Some(nv.value);
                }
            }
        }
    }
    None
}

/// Checks if a field has a specific flag attribute.
/// For example, `#[crudcrate(primary_key)]` or `#[crudcrate(sortable, filterable)]`.
///
/// Also supports convenience aliases for clearer semantics:
/// - `exclude_create` → `create_model = false`
/// - `exclude_update` → `update_model = false`
/// - `exclude_list` → `list_model = false`
pub(crate) fn field_has_crudcrate_flag(field: &syn::Field, flag: &str) -> bool {
    for attr in &field.attrs {
        if attr.path().is_ident("crudcrate")
            && let Meta::List(meta_list) = &attr.meta
            && let Ok(metas) =
                Punctuated::<Meta, Comma>::parse_terminated.parse2(meta_list.tokens.clone())
        {
            for meta in metas {
                if let Meta::Path(path) = meta
                    && path.is_ident(flag)
                {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    // Helper to create a syn::Path from tokens
    fn make_path(tokens: proc_macro2::TokenStream) -> syn::Path {
        syn::parse2(tokens).expect("Failed to parse path")
    }

    #[test]
    fn test_deprecation_error_create_model() {
        let path = make_path(quote!(create_model));
        let error = create_deprecation_error("create_model", &path);
        let msg = error.to_string();
        assert!(
            msg.contains("exclude(create)"),
            "Should suggest exclude(create)"
        );
    }

    #[test]
    fn test_deprecation_error_update_model() {
        let path = make_path(quote!(update_model));
        let error = create_deprecation_error("update_model", &path);
        let msg = error.to_string();
        assert!(
            msg.contains("exclude(update)"),
            "Should suggest exclude(update)"
        );
    }

    #[test]
    fn test_deprecation_error_list_model() {
        let path = make_path(quote!(list_model));
        let error = create_deprecation_error("list_model", &path);
        let msg = error.to_string();
        assert!(
            msg.contains("exclude(list)"),
            "Should suggest exclude(list)"
        );
    }

    #[test]
    fn test_deprecation_error_unknown_key() {
        let path = make_path(quote!(unknown));
        let error = create_deprecation_error("unknown_key", &path);
        let msg = error.to_string();
        assert!(
            msg.contains("exclude(...)"),
            "Should suggest generic exclude syntax"
        );
    }

    #[test]
    fn test_parse_exclude_create() {
        let tokens = quote!(exclude(create));
        let meta_list: syn::MetaList = syn::parse2(tokens).expect("Failed to parse");
        assert_eq!(
            parse_exclude_parameters(&meta_list, "create_model"),
            Some(true)
        );
        assert_eq!(parse_exclude_parameters(&meta_list, "update_model"), None);
    }

    #[test]
    fn test_parse_exclude_update() {
        let tokens = quote!(exclude(update));
        let meta_list: syn::MetaList = syn::parse2(tokens).expect("Failed to parse");
        assert_eq!(
            parse_exclude_parameters(&meta_list, "update_model"),
            Some(true)
        );
        assert_eq!(parse_exclude_parameters(&meta_list, "create_model"), None);
    }

    #[test]
    fn test_parse_exclude_multiple() {
        let tokens = quote!(exclude(create, update, list));
        let meta_list: syn::MetaList = syn::parse2(tokens).expect("Failed to parse");
        assert_eq!(
            parse_exclude_parameters(&meta_list, "create_model"),
            Some(true)
        );
        assert_eq!(
            parse_exclude_parameters(&meta_list, "update_model"),
            Some(true)
        );
        assert_eq!(
            parse_exclude_parameters(&meta_list, "list_model"),
            Some(true)
        );
    }

    #[test]
    fn test_parse_exclude_all_affects_list_and_one() {
        let tokens = quote!(exclude(all));
        let meta_list: syn::MetaList = syn::parse2(tokens).expect("Failed to parse");
        assert_eq!(
            parse_exclude_parameters(&meta_list, "list_model"),
            Some(true)
        );
        assert_eq!(
            parse_exclude_parameters(&meta_list, "one_model"),
            Some(true)
        );
        // exclude(all) doesn't affect create/update
        assert_eq!(parse_exclude_parameters(&meta_list, "create_model"), None);
    }

    #[test]
    fn test_parse_exclude_empty() {
        let tokens = quote!(exclude());
        let meta_list: syn::MetaList = syn::parse2(tokens).expect("Failed to parse");
        assert_eq!(parse_exclude_parameters(&meta_list, "create_model"), None);
    }

    /// Deprecated `key = false` model exclusion still parses (with a stderr
    /// deprecation warning); the flag and explicit `= true` forms parse silently.
    #[test]
    fn test_get_crudcrate_bool_forms() {
        let deprecated: syn::Field = syn::Field::parse_named
            .parse2(quote! {
                #[crudcrate(create_model = false)]
                pub name: String
            })
            .expect("parse field");
        assert_eq!(get_crudcrate_bool(&deprecated, "create_model"), Some(false));

        let explicit: syn::Field = syn::Field::parse_named
            .parse2(quote! {
                #[crudcrate(sortable = true)]
                pub name: String
            })
            .expect("parse field");
        assert_eq!(get_crudcrate_bool(&explicit, "sortable"), Some(true));

        let flag: syn::Field = syn::Field::parse_named
            .parse2(quote! {
                #[crudcrate(filterable)]
                pub name: String
            })
            .expect("parse field");
        assert_eq!(get_crudcrate_bool(&flag, "filterable"), Some(true));
        assert_eq!(get_crudcrate_bool(&flag, "sortable"), None);
    }
}
