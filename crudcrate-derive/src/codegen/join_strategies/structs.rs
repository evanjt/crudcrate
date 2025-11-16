use syn::{Lit, Meta, parse::Parser, punctuated::Punctuated, token::Comma};

/// Configuration for join behavior on a field
#[derive(Debug, Clone, Default)]
pub struct JoinConfig {
    pub on_one: bool,
    pub on_all: bool,
    pub depth: Option<u8>,
    pub relation: Option<String>,
    pub path: Option<String>,
}

impl JoinConfig {
    /// Check if recursion is unlimited (no explicit depth set)
    pub fn is_unlimited_recursion(&self) -> bool {
        self.depth.is_none()
    }
}

/// Parses join configuration from a field's crudcrate attributes.
/// Looks for `#[crudcrate(join(...))]` syntax and extracts join parameters.
pub(crate) fn get_join_config(field: &syn::Field) -> Option<JoinConfig> {
    for attr in &field.attrs {
        if attr.path().is_ident("crudcrate")
            && let Meta::List(meta_list) = &attr.meta
            && let Ok(metas) =
                Punctuated::<Meta, Comma>::parse_terminated.parse2(meta_list.tokens.clone())
        {
            for meta in metas {
                if let Meta::List(list_meta) = meta
                    && list_meta.path.is_ident("join")
                {
                    return parse_join_parameters(&list_meta);
                }
            }
        }
    }
    None
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
