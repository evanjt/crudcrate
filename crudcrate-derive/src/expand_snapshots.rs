//! Snapshot test for the tokens each derive emits.
//!
//! Every `tests/expand/*.rs` file is parsed, each struct carrying a crudcrate
//! derive is expanded through the same entry points the proc macros use, and
//! the formatted output is compared with the sibling `.expanded.rs` file.
//! Only crudcrate's own output is captured: nothing downstream (sea-orm,
//! tracing, serde) is expanded, so the snapshot is independent of toolchain
//! and machine. Set `EXPAND_OVERWRITE=1` to refresh the snapshots.

use std::fs;
use std::path::{Path, PathBuf};

use quote::{ToTokens, format_ident, quote};
use syn::punctuated::Punctuated;
use syn::{DeriveInput, Item, ItemStruct, Token};

fn derive_names(attrs: &[syn::Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("derive"))
        .filter_map(|a| {
            a.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)
                .ok()
        })
        .flatten()
        .filter_map(|p| p.segments.last().map(|s| s.ident.to_string()))
        .collect()
}

fn collect_structs(items: &[Item], prefix: &str, out: &mut Vec<(String, ItemStruct)>) {
    for item in items {
        match item {
            Item::Struct(s) => out.push((format!("{prefix}{}", s.ident), s.clone())),
            Item::Mod(m) => {
                if let Some((_, items)) = &m.content {
                    collect_structs(items, &format!("{prefix}{}__", m.ident), out);
                }
            }
            _ => {}
        }
    }
}

fn expand_source(source: &str) -> String {
    let file = syn::parse_file(source).expect("expand input parses");
    let mut structs = Vec::new();
    collect_structs(&file.items, "", &mut structs);
    let mut modules = proc_macro2::TokenStream::new();
    for (path, item) in structs {
        let input = DeriveInput::from(item.clone()).to_token_stream();
        let mut tokens = proc_macro2::TokenStream::new();
        for derive in derive_names(&item.attrs) {
            let out = match derive.as_str() {
                "EntityToModels" => crate::expand::entity::entity_to_models_impl(input.clone()),
                "ToCreateModel" => {
                    crate::expand::simple_models::to_create_model_impl(input.clone())
                }
                "ToUpdateModel" => {
                    crate::expand::simple_models::to_update_model_impl(input.clone())
                }
                "ToListModel" => crate::expand::simple_models::to_list_model_impl(input.clone()),
                _ => continue,
            };
            tokens.extend(out);
        }
        if tokens.is_empty() {
            continue;
        }
        let ident = format_ident!("{path}");
        modules.extend(quote! { mod #ident { #tokens } });
    }
    let file: syn::File = syn::parse2(modules).expect("expanded output parses as items");
    prettyplease::unparse(&file)
}

fn inputs() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/expand");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("tests/expand exists")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "rs")
                && !p.to_string_lossy().ends_with(".expanded.rs")
        })
        .collect();
    files.sort();
    files
}

#[test]
fn expand_snapshots() {
    let overwrite = std::env::var_os("EXPAND_OVERWRITE").is_some();
    let mut mismatched = Vec::new();
    for input in inputs() {
        let expected_path = input.with_extension("expanded.rs");
        let actual = expand_source(&fs::read_to_string(&input).unwrap());
        if overwrite {
            fs::write(&expected_path, &actual).unwrap();
            continue;
        }
        let expected = fs::read_to_string(&expected_path).unwrap_or_default();
        if expected != actual {
            mismatched.push(expected_path.display().to_string());
        }
    }
    assert!(
        mismatched.is_empty(),
        "generated code changed for:\n  {}\nrun with EXPAND_OVERWRITE=1 to accept",
        mismatched.join("\n  ")
    );
}
