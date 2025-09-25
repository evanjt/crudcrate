use super::structs::CRUDResourceMeta;
use syn::parse::Parser;
use syn::{Lit, Meta, punctuated::Punctuated, token::Comma};

/// Configuration for join behavior on a field
#[derive(Debug, Clone, Default)]
pub struct JoinConfig {
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
pub(crate) fn parse_crud_resource_meta(
    attrs: &[syn::Attribute],
) -> Result<CRUDResourceMeta, syn::Error> {
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
                                } else if nv.path.is_ident("derive_partial_eq") {
                                    meta.derive_partial_eq = value;
                                } else if nv.path.is_ident("derive_eq") {
                                    meta.derive_eq = value;
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
                    } else if path.is_ident("derive_partial_eq") {
                        meta.derive_partial_eq = true;
                    } else if path.is_ident("derive_eq") {
                        meta.derive_eq = true;
                    } else if path.is_ident("no_partial_eq") {
                        meta.derive_partial_eq = false;
                    } else if path.is_ident("no_eq") {
                        meta.derive_eq = false;
                    } else if path.is_ident("debug_output") {
                        #[cfg(feature = "debug")]
                        {
                            meta.debug_output = true;
                        }
                        #[cfg(not(feature = "debug"))]
                        {
                            return Err(syn::Error::new_spanned(
                                path,
                                "debug_output requires --features debug",
                            ));
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
                            // Emit deprecation error for old model exclusion syntax
                            if (key == "create_model" || key == "update_model" || key == "list_model") && !b.value() {
                                let error = create_deprecation_error(key, &nv.path);
                                // Convert to compile error by panicking with structured error message
                                panic!("Compilation failed: {error}");
                            }
                            return Some(b.value());
                        }
                    }
                    // Implicit boolean flag: just `key` means true
                    Meta::Path(path) if path.is_ident(key) => {
                        return Some(true);
                    }
                    // Check for positive logic aliases that set model flags to false
                    Meta::Path(path) => {
                        if let Some(false_value) = check_positive_logic_alias(key, &path) {
                            return Some(false_value);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    None
}

/// Check if a path represents a positive logic alias for model exclusion
fn check_positive_logic_alias(key: &str, path: &syn::Path) -> Option<bool> {
    match key {
        "create_model" => {
            if path.is_ident("exclude_create")
                || path.is_ident("skip_create")
                || path.is_ident("no_create")
            {
                Some(false) // These aliases mean create_model = false
            } else {
                None
            }
        }
        "update_model" => {
            if path.is_ident("exclude_update")
                || path.is_ident("skip_update")
                || path.is_ident("no_update")
            {
                Some(false) // These aliases mean update_model = false
            } else {
                None
            }
        }
        "list_model" => {
            if path.is_ident("exclude_list")
                || path.is_ident("skip_list")
                || path.is_ident("no_list")
            {
                Some(false) // These aliases mean list_model = false
            } else {
                None
            }
        }
        _ => None,
    }
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

/// Create a deprecation error for old model exclusion syntax
fn create_deprecation_error(key: &str, path: &syn::Path) -> syn::Error {
    let new_syntax = match key {
        "create_model" => "exclude(create)",
        "update_model" => "exclude(update)", 
        "list_model" => "exclude(list)",
        _ => "exclude(...)",
    };
    
    syn::Error::new_spanned(
        path,
        format!(
            "The `{key} = false` syntax is deprecated. Use `{new_syntax}` instead for cleaner, more idiomatic code."
        )
    )
}

/// Parse exclude(...) parameters to check if a specific model type is excluded
fn parse_exclude_parameters(meta_list: &syn::MetaList, target_key: &str) -> Option<bool> {
    if let Ok(nested_metas) =
        Punctuated::<Meta, Comma>::parse_terminated.parse2(meta_list.tokens.clone())
    {
        for meta in nested_metas {
            if let Meta::Path(path) = meta {
                let excluded_type = if path.is_ident("create") {
                    "create_model"
                } else if path.is_ident("update") {
                    "update_model"
                } else if path.is_ident("list") {
                    "list_model"
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
                    && (path.is_ident(flag) || is_alias_for(flag, &path))
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Check if a path identifier is an alias for the given flag
fn is_alias_for(flag: &str, path: &syn::Path) -> bool {
    match flag {
        // Support positive logic aliases that map to negative model flags
        "create_model_false" => {
            path.is_ident("exclude_create")
                || path.is_ident("skip_create")
                || path.is_ident("no_create")
        }
        "update_model_false" => {
            path.is_ident("exclude_update")
                || path.is_ident("skip_update")
                || path.is_ident("no_update")
        }
        "list_model_false" => {
            path.is_ident("exclude_list") || path.is_ident("skip_list") || path.is_ident("no_list")
        }
        _ => false,
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
