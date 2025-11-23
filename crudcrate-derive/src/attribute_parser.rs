use crate::traits::crudresource::structs::CRUDResourceMeta;
use syn::parse::Parser;
use syn::{Lit, Meta, punctuated::Punctuated, token::Comma};

/// Parses CRUD resource metadata from struct-level attributes.
/// Looks for `#[crudcrate(...)]` attributes and extracts configuration.
///
/// Supports both legacy syntax and new hook syntax:
/// - Legacy: `fn_delete = my_fn`
/// - New: `create::one::pre = validate_fn`
pub(crate) fn parse_crud_resource_meta(attrs: &[syn::Attribute]) -> CRUDResourceMeta {
    let mut meta = CRUDResourceMeta::default();

    for attr in attrs {
        if attr.path().is_ident("crudcrate")
            && let Meta::List(meta_list) = &attr.meta
            && let Ok(metas) =
                Punctuated::<Meta, Comma>::parse_terminated.parse2(meta_list.tokens.clone())
        {
            for item in metas {
                match item {
                    Meta::NameValue(nv) => {
                        // Handle literal values (strings, booleans, etc.)
                        if let syn::Expr::Lit(expr_lit) = &nv.value {
                            match &expr_lit.lit {
                                Lit::Str(s) => {
                                    let value = s.value();
                                    let ident = nv.path.get_ident().map(std::string::ToString::to_string);
                                    match ident.as_deref() {
                                        Some("name_singular") => meta.name_singular = Some(value),
                                        Some("name_plural") => meta.name_plural = Some(value),
                                        Some("description") => meta.description = Some(value),
                                        Some("fulltext_language") => meta.fulltext_language = Some(value),
                                        _ => {}
                                    }
                                }
                                Lit::Bool(b) => {
                                    let value = b.value();
                                    let ident = nv.path.get_ident().map(std::string::ToString::to_string);
                                    match ident.as_deref() {
                                        Some("generate_router") => meta.generate_router = value,
                                        Some("derive_partial_eq") => meta.derive_partial_eq = value,
                                        Some("derive_eq") => meta.derive_eq = value,
                                        _ => {}
                                    }
                                }
                                _ => {}
                            }
                        } else if let syn::Expr::Path(expr_path) = &nv.value {
                            // Handle function path values
                            let fn_path = &expr_path.path;

                            // Try to parse as new hook syntax (create::one::pre = fn)
                            if let Some((op, cardinality, phase)) = parse_hook_path(&nv.path) {
                                set_hook(&mut meta.hooks, &op, &cardinality, &phase, fn_path.clone());
                            } else {
                                // Fall back to legacy fn_* syntax
                                let ident = nv.path.get_ident().map(std::string::ToString::to_string);
                                match ident.as_deref() {
                                    Some("fn_get_one") => meta.fn_get_one = Some(fn_path.clone()),
                                    Some("fn_get_all") => meta.fn_get_all = Some(fn_path.clone()),
                                    Some("fn_create") => meta.fn_create = Some(fn_path.clone()),
                                    Some("fn_update") => meta.fn_update = Some(fn_path.clone()),
                                    Some("fn_delete") => meta.fn_delete = Some(fn_path.clone()),
                                    Some("fn_delete_many") => meta.fn_delete_many = Some(fn_path.clone()),
                                    Some("operations") => meta.operations = Some(fn_path.clone()),
                                    _ => {}
                                }
                            }
                        }
                    }
                    // Handle boolean flags (like generate_router)
                    Meta::Path(path) => {
                        let ident = path.get_ident().map(std::string::ToString::to_string);
                        match ident.as_deref() {
                            Some("generate_router") => meta.generate_router = true,
                            Some("derive_partial_eq") => meta.derive_partial_eq = true,
                            Some("derive_eq") => meta.derive_eq = true,
                            Some("no_partial_eq") => meta.derive_partial_eq = false,
                            Some("no_eq") => meta.derive_eq = false,
                            _ => {}
                        }
                    }
                    Meta::List(_) => {}
                }
            }
        }
    }

    // Apply backward compatibility: map legacy fn_* to new hooks
    apply_legacy_fn_mappings(&mut meta);

    meta
}

/// Parse a path like `create::one::pre` into (operation, cardinality, phase)
fn parse_hook_path(path: &syn::Path) -> Option<(String, String, String)> {
    let segments: Vec<_> = path.segments.iter().map(|s| s.ident.to_string()).collect();

    if segments.len() != 3 {
        return None;
    }

    let operation = &segments[0];
    let cardinality = &segments[1];
    let phase = &segments[2];

    // Validate operation
    if !matches!(operation.as_str(), "create" | "read" | "update" | "delete") {
        return None;
    }

    // Validate cardinality
    if !matches!(cardinality.as_str(), "one" | "many") {
        return None;
    }

    // Validate phase
    if !matches!(phase.as_str(), "pre" | "body" | "post") {
        return None;
    }

    Some((operation.clone(), cardinality.clone(), phase.clone()))
}

/// Set a hook in the CrudHooks structure
fn set_hook(
    hooks: &mut crate::traits::crudresource::structs::CrudHooks,
    operation: &str,
    cardinality: &str,
    phase: &str,
    fn_path: syn::Path,
) {
    let op_hooks = match operation {
        "create" => &mut hooks.create,
        "read" => &mut hooks.read,
        "update" => &mut hooks.update,
        "delete" => &mut hooks.delete,
        _ => return,
    };

    let card_hooks = match cardinality {
        "one" => &mut op_hooks.one,
        "many" => &mut op_hooks.many,
        _ => return,
    };

    match phase {
        "pre" => card_hooks.pre = Some(fn_path),
        "body" => card_hooks.body = Some(fn_path),
        "post" => card_hooks.post = Some(fn_path),
        _ => {}
    }
}

/// Map legacy fn_* attributes to new hook system for backward compatibility
fn apply_legacy_fn_mappings(meta: &mut CRUDResourceMeta) {
    // fn_create -> create::one::body
    if let Some(ref path) = meta.fn_create {
        if meta.hooks.create.one.body.is_none() {
            meta.hooks.create.one.body = Some(path.clone());
        }
    }

    // fn_get_one -> read::one::body
    if let Some(ref path) = meta.fn_get_one {
        if meta.hooks.read.one.body.is_none() {
            meta.hooks.read.one.body = Some(path.clone());
        }
    }

    // fn_get_all -> read::many::body
    if let Some(ref path) = meta.fn_get_all {
        if meta.hooks.read.many.body.is_none() {
            meta.hooks.read.many.body = Some(path.clone());
        }
    }

    // fn_update -> update::one::body
    if let Some(ref path) = meta.fn_update {
        if meta.hooks.update.one.body.is_none() {
            meta.hooks.update.one.body = Some(path.clone());
        }
    }

    // fn_delete -> delete::one::body
    if let Some(ref path) = meta.fn_delete {
        if meta.hooks.delete.one.body.is_none() {
            meta.hooks.delete.one.body = Some(path.clone());
        }
    }

    // fn_delete_many -> delete::many::body
    if let Some(ref path) = meta.fn_delete_many {
        if meta.hooks.delete.many.body.is_none() {
            meta.hooks.delete.many.body = Some(path.clone());
        }
    }
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
                                eprintln!("\n⚠️  DEPRECATION WARNING: {}\n", create_deprecation_error(key, &nv.path));
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
/// Note: Returns syn::Error for consistent formatting, but we extract the message
/// rather than using to_compile_error() to avoid breaking backward compatibility.
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
                    && path.is_ident(flag)
                {
                    return true;
                }
            }
        }
    }
    false
}

