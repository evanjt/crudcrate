use super::structs::CRUDResourceMeta;
use syn::parse::Parser;
use syn::{Lit, Meta, punctuated::Punctuated, token::Comma};

/// Configuration for join behavior on a field
#[derive(Debug, Clone, Default)]
pub(crate) struct JoinConfig {
    pub on_one: bool,
    pub on_all: bool,
    pub depth: Option<u8>,
    pub relation: Option<String>,
}

impl JoinConfig {
    /// Get the effective depth for recursive loading (default 3 if not specified)
    pub fn effective_depth(&self) -> u8 {
        self.depth.unwrap_or(3)
    }
    
    /// Check if depth was explicitly specified (for cyclic dependency warnings)
    pub fn has_explicit_depth(&self) -> bool {
        self.depth.is_some()
    }
}

/// Parses CRUD resource metadata from struct-level attributes.
/// Looks for `#[crudcrate(...)]` attributes and extracts configuration.
pub(crate) fn parse_crud_resource_meta(attrs: &[syn::Attribute]) -> Result<CRUDResourceMeta, syn::Error> {
    let mut meta = CRUDResourceMeta::default();

    for attr in attrs {
        if attr.path().is_ident("crudcrate")
            && let Meta::List(meta_list) = &attr.meta
            && let Ok(metas) =
                Punctuated::<Meta, Comma>::parse_terminated.parse2(meta_list.tokens.clone())
        {
            for item in metas {
                if let Meta::NameValue(nv) = item {
                    // Handle literal values (strings, booleans, etc.)
                    if let syn::Expr::Lit(expr_lit) = &nv.value {
                        match &expr_lit.lit {
                            Lit::Str(s) => {
                                let value = s.value();
                                if nv.path.is_ident("name_singular") {
                                    meta.name_singular = Some(value);
                                } else if nv.path.is_ident("name_plural") {
                                    meta.name_plural = Some(value);
                                } else if nv.path.is_ident("description") {
                                    meta.description = Some(value);
                                } else if nv.path.is_ident("entity_type") {
                                    meta.entity_type = Some(value);
                                } else if nv.path.is_ident("column_type") {
                                    meta.column_type = Some(value);
                                } else if nv.path.is_ident("fulltext_language") {
                                    meta.fulltext_language = Some(value);
                                }
                            }
                            Lit::Bool(b) => {
                                let value = b.value();
                                if nv.path.is_ident("generate_router") {
                                    meta.generate_router = value;
                                }
                            }
                            _ => {}
                        }
                    } else if let syn::Expr::Path(expr_path) = &nv.value {
                        // Handle function path values
                        if nv.path.is_ident("fn_get_one") {
                            meta.fn_get_one = Some(expr_path.path.clone());
                        } else if nv.path.is_ident("fn_get_all") {
                            meta.fn_get_all = Some(expr_path.path.clone());
                        } else if nv.path.is_ident("fn_create") {
                            meta.fn_create = Some(expr_path.path.clone());
                        } else if nv.path.is_ident("fn_update") {
                            meta.fn_update = Some(expr_path.path.clone());
                        } else if nv.path.is_ident("fn_delete") {
                            meta.fn_delete = Some(expr_path.path.clone());
                        } else if nv.path.is_ident("fn_delete_many") {
                            meta.fn_delete_many = Some(expr_path.path.clone());
                        }
                    }
                }
                // Handle boolean flags (like generate_router)
                else if let Meta::Path(path) = item {
                    if path.is_ident("generate_router") {
                        meta.generate_router = true;
                    } else if path.is_ident("debug_output") {
                        #[cfg(feature = "debug")]
                        {
                            meta.debug_output = true;
                        }
                        #[cfg(not(feature = "debug"))]
                        {
                            return Err(syn::Error::new_spanned(path, "debug_output requires --features debug"));
                        }
                    }
                }
            }
        }
    }
    Ok(meta)
}

/// Extracts the table name from Sea-ORM attributes.
/// Looks for `#[sea_orm(table_name = "...")]` attribute.
pub(crate) fn extract_table_name(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("sea_orm")
            && let Meta::List(meta_list) = &attr.meta
            && let Ok(metas) =
                Punctuated::<Meta, Comma>::parse_terminated.parse2(meta_list.tokens.clone())
        {
            for meta in metas {
                if let Meta::NameValue(nv) = meta
                    && nv.path.is_ident("table_name")
                    && let syn::Expr::Lit(expr_lit) = &nv.value
                    && let Lit::Str(s) = &expr_lit.lit
                {
                    return Some(s.value());
                }
            }
        }
    }
    None
}

/// Given a field and a key (e.g. `"create_model"` or `"update_model"`),
/// look for a `#[crudcrate(...)]` attribute on the field and return the boolean value
/// associated with that key, if present.
pub(crate) fn get_crudcrate_bool(field: &syn::Field, key: &str) -> Option<bool> {
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
                    && let syn::Expr::Lit(expr_lit) = &nv.value
                    && let Lit::Bool(b) = &expr_lit.lit
                {
                    return Some(b.value());
                }
            }
        }
    }
    None
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

/// Extracts a string literal from a struct‐level attribute of the form:
///   `#[active_model = "some::path"]`
pub(crate) fn get_string_from_attr(attr: &syn::Attribute) -> Option<String> {
    if let Meta::NameValue(nv) = &attr.meta
        && let syn::Expr::Lit(expr_lit) = &nv.value
        && let Lit::Str(s) = &expr_lit.lit
    {
        return Some(s.value());
    }
    None
}

/// Checks if a field has a specific flag attribute.
/// For example, `#[crudcrate(primary_key)]` or `#[crudcrate(sortable, filterable)]`.
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
    
    if let Ok(nested_metas) = Punctuated::<Meta, Comma>::parse_terminated
        .parse2(meta_list.tokens.clone())
    {
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
                // Parse named parameters: depth = 2, relation = "Vehicle"
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
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }
    
    // Only return config if at least one join type is enabled
    if config.on_one || config.on_all {
        Some(config)
    } else {
        None
    }
}
