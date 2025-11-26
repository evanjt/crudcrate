use syn::{Lit, Meta, parse::Parser, punctuated::Punctuated, token::Comma};

/// Configuration for join behavior on a field
#[derive(Debug, Clone, Default)]
pub struct JoinConfig {
    pub on_one: bool,
    pub on_all: bool,
    pub depth: Option<u8>,
    pub relation: Option<String>,
    pub path: Option<String>,
    /// Columns on the joined entity that can be filtered via dot-notation (e.g., "vehicles.make")
    pub filterable_columns: Vec<String>,
    /// Columns on the joined entity that can be sorted via dot-notation (e.g., "vehicles.year")
    pub sortable_columns: Vec<String>,
}

/// Result of parsing join config - may contain deprecation errors
pub struct JoinConfigResult {
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

/// Create an error for deprecated join_filterable/join_sortable syntax
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
    if let Ok(exprs) = Punctuated::<syn::Expr, Comma>::parse_terminated.parse2(meta_list.tokens.clone()) {
        for expr in exprs {
            if let syn::Expr::Lit(expr_lit) = expr {
                if let Lit::Str(lit_str) = expr_lit.lit {
                    result.push(lit_str.value());
                }
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
