//! Hook path parsing (`create::one::pre`) and the legacy `fn_*` deprecation errors.

/// Parse a path like `create::one::pre` into (operation, cardinality, phase)
pub(crate) fn parse_hook_path(path: &syn::Path) -> Option<(String, String, String)> {
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
    if !matches!(phase.as_str(), "pre" | "body" | "transform" | "post") {
        return None;
    }

    Some((operation.clone(), cardinality.clone(), phase.clone()))
}

/// Set a hook in the `CrudHooks` structure
pub(crate) fn set_hook(
    hooks: &mut crate::ir::CrudHooks,
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
        "transform" => card_hooks.transform = Some(fn_path),
        "post" => card_hooks.post = Some(fn_path),
        _ => {}
    }
}

/// Create a deprecation error for legacy fn_* syntax
pub(crate) fn create_fn_deprecation_error(
    old_attr: &str,
    new_syntax: &str,
    path: &syn::Path,
) -> syn::Error {
    syn::Error::new_spanned(
        path,
        format!(
            "The `{old_attr}` attribute is deprecated and no longer supported.\n\
             Use the new hook syntax instead: `{new_syntax} = your_function`\n\
             \n\
             Migration guide:\n\
             - fn_create      -> create::one::body\n\
             - fn_get_one     -> read::one::body\n\
             - fn_get_all     -> read::many::body\n\
             - fn_update      -> update::one::body\n\
             - fn_delete      -> delete::one::body\n\
             - fn_delete_many -> delete::many::body\n\
             \n\
             New hook phases available:\n\
             - ::pre  - runs before the operation (validation, auth)\n\
             - ::body - replaces the default implementation\n\
             - ::post - runs after the operation (notifications, side effects)"
        ),
    )
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
    fn test_parse_hook_path_valid_create_one_pre() {
        let path = make_path(quote!(create::one::pre));
        let result = parse_hook_path(&path);
        assert_eq!(
            result,
            Some(("create".to_string(), "one".to_string(), "pre".to_string()))
        );
    }

    #[test]
    fn test_parse_hook_path_valid_delete_many_body() {
        let path = make_path(quote!(delete::many::body));
        let result = parse_hook_path(&path);
        assert_eq!(
            result,
            Some(("delete".to_string(), "many".to_string(), "body".to_string()))
        );
    }

    #[test]
    fn test_parse_hook_path_valid_read_one_post() {
        let path = make_path(quote!(read::one::post));
        let result = parse_hook_path(&path);
        assert_eq!(
            result,
            Some(("read".to_string(), "one".to_string(), "post".to_string()))
        );
    }

    #[test]
    fn test_parse_hook_path_valid_update_many_pre() {
        let path = make_path(quote!(update::many::pre));
        let result = parse_hook_path(&path);
        assert_eq!(
            result,
            Some(("update".to_string(), "many".to_string(), "pre".to_string()))
        );
    }

    #[test]
    fn test_parse_hook_path_invalid_operation() {
        let path = make_path(quote!(invalid::one::pre));
        assert_eq!(parse_hook_path(&path), None);
    }

    #[test]
    fn test_parse_hook_path_invalid_cardinality() {
        let path = make_path(quote!(create::two::pre));
        assert_eq!(parse_hook_path(&path), None);
    }

    #[test]
    fn test_parse_hook_path_invalid_phase() {
        let path = make_path(quote!(create::one::during));
        assert_eq!(parse_hook_path(&path), None);
    }

    #[test]
    fn test_parse_hook_path_too_few_segments() {
        let path = make_path(quote!(create::one));
        assert_eq!(parse_hook_path(&path), None);
    }

    #[test]
    fn test_parse_hook_path_too_many_segments() {
        let path = make_path(quote!(create::one::pre::extra));
        assert_eq!(parse_hook_path(&path), None);
    }

    #[test]
    fn test_parse_hook_path_single_segment() {
        let path = make_path(quote!(create));
        assert_eq!(parse_hook_path(&path), None);
    }

    #[test]
    fn test_set_hook_create_one_pre() {
        let mut hooks = crate::ir::CrudHooks::default();
        let fn_path = make_path(quote!(my_validator));
        set_hook(&mut hooks, "create", "one", "pre", fn_path);
        assert!(hooks.create.one.pre.is_some());
        assert!(hooks.create.one.body.is_none());
        assert!(hooks.create.one.post.is_none());
    }

    #[test]
    fn test_set_hook_delete_many_body() {
        let mut hooks = crate::ir::CrudHooks::default();
        let fn_path = make_path(quote!(delete_handler));
        set_hook(&mut hooks, "delete", "many", "body", fn_path);
        assert!(hooks.delete.many.body.is_some());
    }

    #[test]
    fn test_set_hook_read_one_post() {
        let mut hooks = crate::ir::CrudHooks::default();
        let fn_path = make_path(quote!(post_read_hook));
        set_hook(&mut hooks, "read", "one", "post", fn_path);
        assert!(hooks.read.one.post.is_some());
    }

    #[test]
    fn test_set_hook_invalid_operation_no_effect() {
        let mut hooks = crate::ir::CrudHooks::default();
        let fn_path = make_path(quote!(some_fn));
        set_hook(&mut hooks, "invalid", "one", "pre", fn_path);
        // All should remain None
        assert!(hooks.create.one.pre.is_none());
        assert!(hooks.read.one.pre.is_none());
        assert!(hooks.update.one.pre.is_none());
        assert!(hooks.delete.one.pre.is_none());
    }

    #[test]
    fn test_set_hook_invalid_cardinality_no_effect() {
        let mut hooks = crate::ir::CrudHooks::default();
        let fn_path = make_path(quote!(some_fn));
        set_hook(&mut hooks, "create", "invalid", "pre", fn_path);
        assert!(hooks.create.one.pre.is_none());
        assert!(hooks.create.many.pre.is_none());
    }

    #[test]
    fn test_set_hook_invalid_phase_no_effect() {
        let mut hooks = crate::ir::CrudHooks::default();
        let fn_path = make_path(quote!(some_fn));
        set_hook(&mut hooks, "create", "one", "invalid", fn_path);
        assert!(hooks.create.one.pre.is_none());
        assert!(hooks.create.one.body.is_none());
        assert!(hooks.create.one.post.is_none());
    }

    #[test]
    fn test_fn_deprecation_error_contains_old_attr() {
        let path = make_path(quote!(fn_create));
        let error = create_fn_deprecation_error("fn_create", "create::one::body", &path);
        let msg = error.to_string();
        assert!(
            msg.contains("fn_create"),
            "Error should mention old attribute"
        );
    }

    #[test]
    fn test_fn_deprecation_error_contains_new_syntax() {
        let path = make_path(quote!(fn_delete));
        let error = create_fn_deprecation_error("fn_delete", "delete::one::body", &path);
        let msg = error.to_string();
        assert!(
            msg.contains("delete::one::body"),
            "Error should mention new syntax"
        );
    }

    #[test]
    fn test_fn_deprecation_error_contains_migration_guide() {
        let path = make_path(quote!(fn_get_all));
        let error = create_fn_deprecation_error("fn_get_all", "read::many::body", &path);
        let msg = error.to_string();
        assert!(
            msg.contains("Migration guide"),
            "Error should contain migration guide"
        );
    }

    #[test]
    fn test_all_operations_valid() {
        // Test each operation explicitly
        assert!(parse_hook_path(&make_path(quote!(create::one::pre))).is_some());
        assert!(parse_hook_path(&make_path(quote!(read::one::pre))).is_some());
        assert!(parse_hook_path(&make_path(quote!(update::one::pre))).is_some());
        assert!(parse_hook_path(&make_path(quote!(delete::one::pre))).is_some());
    }

    #[test]
    fn test_all_cardinalities_valid() {
        assert!(parse_hook_path(&make_path(quote!(create::one::pre))).is_some());
        assert!(parse_hook_path(&make_path(quote!(create::many::pre))).is_some());
    }

    #[test]
    fn test_all_phases_valid() {
        assert!(parse_hook_path(&make_path(quote!(create::one::pre))).is_some());
        assert!(parse_hook_path(&make_path(quote!(create::one::body))).is_some());
        assert!(parse_hook_path(&make_path(quote!(create::one::post))).is_some());
    }
}
