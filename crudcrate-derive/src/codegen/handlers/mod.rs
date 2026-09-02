//! Emits the `CRUDResource` method bodies for each operation, with hook and join wiring.

pub(crate) mod create;
pub(crate) mod delete;
pub(crate) mod get;
pub(crate) mod update;

use crate::ir::OperationHooks;

/// The `transform` phase: rebinds `result` through the hook, if one is configured.
pub(crate) fn transform_hook(hooks: &OperationHooks) -> Option<proc_macro2::TokenStream> {
    hooks.transform.as_ref().map(|fn_path| {
        quote::quote! { let result = #fn_path(db, result).await?; }
    })
}
