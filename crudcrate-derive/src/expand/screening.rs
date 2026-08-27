//! Rejects inputs the derives must not expand on: the Sea-ORM `ModelEx` companion and misplaced relation wrappers.

use syn::DeriveInput;

/// Outcome of screening a derive target against Sea-ORM 2.0's dense entity
/// format expansion.
pub(crate) enum ModelExScreen {
    /// An ordinary column-backed model; expand normally.
    Normal,
    /// The `ModelEx` companion that `#[sea_orm::model]` emits alongside the
    /// scalar `Model`. The attribute macro copies every remaining derive
    /// (including ours) onto both structs; expanding on the companion would
    /// generate a colliding duplicate API, so it is skipped silently.
    Companion,
    /// A struct that carries relation wrapper fields (`HasMany<..>`,
    /// `HasOne<..>`, `BelongsTo<..>`) but is not the generated companion:
    /// the user wrote dense-format relations without `#[sea_orm::model]`.
    /// Expanding would treat the wrapper as a column; skipping silently would
    /// surface as confusing "cannot find type" errors downstream. Gate with
    /// a spanned error carrying the fix.
    MisplacedRelationField(Box<syn::Error>),
}

pub(crate) fn screen_model_ex(input: &DeriveInput) -> ModelExScreen {
    if input.ident == "ModelEx" {
        return ModelExScreen::Companion;
    }
    if let syn::Data::Struct(data) = &input.data {
        for field in &data.fields {
            if let syn::Type::Path(type_path) = &field.ty {
                let is_wrapper = type_path.path.segments.last().is_some_and(|segment| {
                    matches!(
                        segment.ident.to_string().as_str(),
                        "HasMany" | "HasOne" | "BelongsTo"
                    )
                });
                if is_wrapper {
                    return ModelExScreen::MisplacedRelationField(Box::new(
                        syn::Error::new_spanned(
                            field,
                            "relation wrapper fields (HasMany/HasOne/BelongsTo) require the \
                             Sea-ORM 2.0 dense entity format: add #[sea_orm::model] above the \
                             derives so relations move to the generated ModelEx companion. \
                             crudcrate derives operate on column-backed fields only; to expose \
                             related entities through the API, use a #[crudcrate(non_db_attr, \
                             join(...))] field.",
                        ),
                    ));
                }
            }
        }
    }
    ModelExScreen::Normal
}

/// Replacement tokens when screening rejects the input: empty output for the
/// `ModelEx` companion, a compile error for a misplaced relation wrapper,
/// `None` for a normal struct.
pub(crate) fn screen_tokens(input: &DeriveInput) -> Option<proc_macro2::TokenStream> {
    match screen_model_ex(input) {
        ModelExScreen::Companion => Some(proc_macro2::TokenStream::new()),
        ModelExScreen::MisplacedRelationField(err) => Some(err.to_compile_error()),
        ModelExScreen::Normal => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_screen_companion_returns_empty_tokens() {
        let input: DeriveInput = parse_quote! {
            pub struct ModelEx {
                pub id: i32,
            }
        };
        let tokens = screen_tokens(&input).expect("companion is screened out");
        assert!(tokens.is_empty(), "companion must expand to nothing");
    }

    #[test]
    fn test_screen_misplaced_wrapper_is_compile_error() {
        for wrapper in ["HasMany", "HasOne", "BelongsTo"] {
            let ty: syn::Type = syn::parse_str(&format!("{wrapper}<Other>")).unwrap();
            let input: DeriveInput = parse_quote! {
                pub struct Model {
                    pub id: i32,
                    pub related: #ty,
                }
            };
            let tokens = screen_tokens(&input).expect("wrapper field is screened out");
            let rendered = tokens.to_string();
            assert!(
                rendered.contains("compile_error"),
                "{wrapper} must produce a compile error, got: {rendered}"
            );
            assert!(
                rendered.contains("sea_orm :: model") || rendered.contains("dense entity"),
                "error must name the fix, got: {rendered}"
            );
        }
    }

    #[test]
    fn test_screen_normal_struct_passes() {
        let input: DeriveInput = parse_quote! {
            pub struct Model {
                pub id: i32,
                pub name: String,
                pub pair: (i32, i32),
            }
        };
        assert!(screen_tokens(&input).is_none());
    }

    #[test]
    fn test_screen_enum_passes() {
        let input: DeriveInput = parse_quote! {
            pub enum Status {
                Active,
            }
        };
        assert!(screen_tokens(&input).is_none());
    }
}
