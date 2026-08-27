//! Struct-level `#[crudcrate(...)]` attributes: resource metadata, hooks wiring, table name.

use crate::attrs::hooks::{create_fn_deprecation_error, parse_hook_path, set_hook};
use crate::attrs::join::parse_struct_level_join;
use crate::ir::CRUDResourceMeta;
use syn::parse::Parser;
use syn::{Lit, Meta, punctuated::Punctuated, token::Comma};

/// Parses CRUD resource metadata from struct-level attributes.
/// Looks for `#[crudcrate(...)]` attributes and extracts configuration.
///
/// Supports both legacy syntax and new hook syntax:
/// - Legacy: `fn_delete = my_fn`
/// - New: `create::one::pre = validate_fn`
pub(crate) fn parse_crud_resource_meta(attrs: &[syn::Attribute]) -> CRUDResourceMeta {
    let mut meta = CRUDResourceMeta::new();

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
                                    let ident =
                                        nv.path.get_ident().map(std::string::ToString::to_string);
                                    match ident.as_deref() {
                                        Some("name_singular") => meta.name_singular = Some(value),
                                        Some("name_plural") => meta.name_plural = Some(value),
                                        Some("description") => meta.description = Some(value),
                                        Some("fulltext_language") => {
                                            meta.fulltext_language = Some(value);
                                        }
                                        Some("security_profile") => {
                                            meta.security_profile = Some(value);
                                        }
                                        _ => {}
                                    }
                                }
                                Lit::Bool(b) => {
                                    let value = b.value();
                                    let ident =
                                        nv.path.get_ident().map(std::string::ToString::to_string);
                                    match ident.as_deref() {
                                        Some("generate_router") => meta.generate_router = value,
                                        Some("derive_partial_eq") => meta.derive_partial_eq = value,
                                        Some("derive_eq") => meta.derive_eq = value,
                                        _ => {}
                                    }
                                }
                                Lit::Int(i) => {
                                    let ident =
                                        nv.path.get_ident().map(std::string::ToString::to_string);
                                    match ident.as_deref() {
                                        Some("batch_limit") => {
                                            meta.batch_limit = i.base10_parse().ok();
                                        }
                                        Some("max_child_rows") => {
                                            meta.max_child_rows = i.base10_parse().ok();
                                        }
                                        Some("max_page_size") => {
                                            meta.max_page_size = i.base10_parse().ok();
                                        }
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
                                set_hook(
                                    &mut meta.hooks,
                                    &op,
                                    &cardinality,
                                    &phase,
                                    fn_path.clone(),
                                );
                            } else {
                                // Check for legacy fn_* syntax and emit deprecation errors
                                let ident =
                                    nv.path.get_ident().map(std::string::ToString::to_string);
                                match ident.as_deref() {
                                    Some("fn_get_one") => {
                                        meta.deprecation_errors.push(create_fn_deprecation_error(
                                            "fn_get_one",
                                            "read::one::body",
                                            &nv.path,
                                        ));
                                    }
                                    Some("fn_get_all") => {
                                        meta.deprecation_errors.push(create_fn_deprecation_error(
                                            "fn_get_all",
                                            "read::many::body",
                                            &nv.path,
                                        ));
                                    }
                                    Some("fn_create") => {
                                        meta.deprecation_errors.push(create_fn_deprecation_error(
                                            "fn_create",
                                            "create::one::body",
                                            &nv.path,
                                        ));
                                    }
                                    Some("fn_update") => {
                                        meta.deprecation_errors.push(create_fn_deprecation_error(
                                            "fn_update",
                                            "update::one::body",
                                            &nv.path,
                                        ));
                                    }
                                    Some("fn_delete") => {
                                        meta.deprecation_errors.push(create_fn_deprecation_error(
                                            "fn_delete",
                                            "delete::one::body",
                                            &nv.path,
                                        ));
                                    }
                                    Some("fn_delete_many") => {
                                        meta.deprecation_errors.push(create_fn_deprecation_error(
                                            "fn_delete_many",
                                            "delete::many::body",
                                            &nv.path,
                                        ));
                                    }
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
                            Some("require_scope") => meta.require_scope = true,
                            Some("deny_unknown_fields") => meta.deny_unknown_fields = true,
                            _ => {}
                        }
                    }
                    Meta::List(list) => {
                        if list.path.is_ident("join")
                            && let Some(join_def) = parse_struct_level_join(&list)
                        {
                            if join_def.depth == Some(0) {
                                meta.deprecation_errors.push(syn::Error::new_spanned(
                                    &list,
                                    "Join `depth = 0` is invalid (causes infinite recursion). \
                                         Use `depth = 1` for shallow loading.",
                                ));
                            }
                            meta.struct_level_joins.push(join_def);
                        }
                    }
                }
            }
        }
    }

    meta
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

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn test_parse_meta_deny_unknown_fields() {
        let input: syn::DeriveInput = syn::parse2(quote! {
            #[crudcrate(deny_unknown_fields)]
            pub struct Model {
                pub id: i32,
            }
        })
        .expect("parse struct");
        assert!(parse_crud_resource_meta(&input.attrs).deny_unknown_fields);

        let default: syn::DeriveInput = syn::parse2(quote! {
            pub struct Model {
                pub id: i32,
            }
        })
        .expect("parse struct");
        assert!(!parse_crud_resource_meta(&default.attrs).deny_unknown_fields);
    }
}
