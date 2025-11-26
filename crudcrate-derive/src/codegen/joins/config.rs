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

/// Parses join configuration from a field's crudcrate attributes.
/// Looks for `#[crudcrate(join(...))]` syntax and extracts join parameters.
/// Also looks for `join_filterable(...)` and `join_sortable(...)` at the same level.
pub(crate) fn get_join_config(field: &syn::Field) -> Option<JoinConfig> {
    let mut config: Option<JoinConfig> = None;
    let mut filterable_columns: Vec<String> = Vec::new();
    let mut sortable_columns: Vec<String> = Vec::new();

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
                        filterable_columns = parse_string_list(list_meta);
                    }
                    Meta::List(list_meta) if list_meta.path.is_ident("join_sortable") => {
                        sortable_columns = parse_string_list(list_meta);
                    }
                    _ => {}
                }
            }
        }
    }

    // Merge filterable/sortable columns into config if join was found
    if let Some(mut cfg) = config {
        cfg.filterable_columns = filterable_columns;
        cfg.sortable_columns = sortable_columns;
        Some(cfg)
    } else {
        None
    }
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
                    Meta::List(_) => {}
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
