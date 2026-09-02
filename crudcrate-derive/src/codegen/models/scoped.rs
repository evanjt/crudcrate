//! `ScopedList` and `ScopedResponse` models and the `ScopeFilterable` impls behind `exclude(scoped)`.

use crate::attrs::get_join_config;
use crate::codegen::models::shared::wire_attrs;
use crate::codegen::models::should_include_in_model;
use crate::syn_type::{
    column_ident, inner_list_type_of_option, inner_list_type_of_vec, is_option_type, is_vec_type,
    transform_type_to_scoped_list_variant,
};
use quote::{format_ident, quote};

/// Check if a type is `bool` (plain, not Option<bool>)
fn is_bool_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        type_path.path.is_ident("bool")
    } else {
        false
    }
}

/// Distinct scoped structs when the entity has `exclude(scoped)` fields, otherwise type aliases
/// so parents can always name `ChildScopedList` and `ChildScopedResponse`.
pub(crate) fn generate_scoped_models(
    all_fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    api_struct_name: &syn::Ident,
    list_name: &syn::Ident,
    response_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    // Always generate ScopedList/ScopedResponse so parent entities can reference
    // child scoped types in their own scoped models (join fields).
    let has_scoped_exclusions = all_fields
        .iter()
        .any(crate::codegen::models::is_scoped_exclusion);

    let scoped_list_name = format_ident!("{}ScopedList", api_struct_name);
    let scoped_response_name = format_ident!("{}ScopedResponse", api_struct_name);

    if has_scoped_exclusions {
        // Entity has exclude(scoped) fields: generate distinct scoped structs

        // ScopedList: fields included in list AND not exclude(scoped)
        // Join(all) fields use the *ScopedList* child type so that excluded
        // fields on children are also stripped from nested responses.
        let scoped_list_fields: Vec<_> = all_fields
            .iter()
            .filter(|f| {
                should_include_in_model(f, "list_model")
                    && should_include_in_model(f, "scoped_model")
            })
            .map(|f| {
                let ident = &f.ident;
                let attrs = wire_attrs(f);
                let is_join_all = get_join_config(f).is_some_and(|c| c.on_all);
                if is_join_all {
                    let scoped_ty = transform_type_to_scoped_list_variant(&f.ty, api_struct_name);
                    quote! { #(#attrs)* pub #ident: #scoped_ty }
                } else {
                    let ty = &f.ty;
                    quote! { #(#attrs)* pub #ident: #ty }
                }
            })
            .collect();

        // From<ListModel> for ScopedList: join Vec fields need per-element conversion
        let scoped_list_from: Vec<_> = all_fields
            .iter()
            .filter(|f| should_include_in_model(f, "list_model") && should_include_in_model(f, "scoped_model"))
            .map(|f| {
                let ident = &f.ident;
                let is_join_all = get_join_config(f).is_some_and(|c| c.on_all);
                if is_join_all && is_vec_type(&f.ty) {
                    // ListModel.field is Vec<ChildList>, filter private children then convert to ChildScopedList
                    quote! { #ident: model.#ident.into_iter().filter(|x| crudcrate::ScopeFilterable::is_scope_visible(x)).map(Into::into).collect() }
                } else {
                    quote! { #ident: model.#ident }
                }
            })
            .collect();

        // ScopedResponse: fields included in response AND not exclude(scoped)
        // Join fields use ScopedList child type (same as in ScopedList)
        let scoped_response_fields: Vec<_> = all_fields
            .iter()
            .filter(|f| {
                should_include_in_model(f, "one_model")
                    && should_include_in_model(f, "scoped_model")
            })
            .map(|f| {
                let ident = &f.ident;
                let attrs = wire_attrs(f);
                let is_join = get_join_config(f).is_some();
                if is_join {
                    let scoped_ty = transform_type_to_scoped_list_variant(&f.ty, api_struct_name);
                    quote! { #(#attrs)* pub #ident: #scoped_ty }
                } else {
                    let ty = &f.ty;
                    quote! { #(#attrs)* pub #ident: #ty }
                }
            })
            .collect();

        // From<ResponseModel> for ScopedResponse: join fields need chained conversion
        // ResponseModel.field is Vec<Child> (raw type), need Vec<ChildScopedList>
        // Chain: Child → ChildList → ChildScopedList via two .into() calls
        let scoped_response_from: Vec<_> = all_fields
            .iter()
            .filter(|f| should_include_in_model(f, "one_model") && should_include_in_model(f, "scoped_model"))
            .map(|f| {
                let ident = &f.ident;
                let is_join = get_join_config(f).is_some();
                if is_join && is_vec_type(&f.ty) {
                    // Response.field is Vec<Child>, target is Vec<ChildScopedList>
                    // Filter private children, then chain: Child → ChildList → ChildScopedList
                    let inner_list_ty = inner_list_type_of_vec(&f.ty);
                    quote! {
                        #ident: model.#ident.into_iter().filter(|x| crudcrate::ScopeFilterable::is_scope_visible(x)).map(|x| {
                            let list_item: #inner_list_ty = x.into();
                            list_item.into()
                        }).collect()
                    }
                } else if is_join && is_option_type(&f.ty) {
                    // Response.field is Option<Child>, target is Option<ChildScopedList>
                    // Filter private children via ScopeFilterable before conversion
                    let inner_list_ty = inner_list_type_of_option(&f.ty);
                    quote! {
                        #ident: model.#ident
                            .filter(|x| crudcrate::ScopeFilterable::is_scope_visible(x))
                            .map(|x| {
                                let list_item: #inner_list_ty = x.into();
                                list_item.into()
                            })
                    }
                } else {
                    quote! { #ident: model.#ident }
                }
            })
            .collect();

        let derives = quote! { Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema };

        quote! {
            #[derive(#derives)]
            pub struct #scoped_list_name {
                #(#scoped_list_fields),*
            }

            impl From<#list_name> for #scoped_list_name {
                fn from(model: #list_name) -> Self {
                    Self {
                        #(#scoped_list_from),*
                    }
                }
            }

            #[derive(#derives)]
            pub struct #scoped_response_name {
                #(#scoped_response_fields),*
            }

            impl From<#response_name> for #scoped_response_name {
                fn from(model: #response_name) -> Self {
                    Self {
                        #(#scoped_response_from),*
                    }
                }
            }
        }
    } else {
        // No exclude(scoped) fields: generate type aliases so parents can
        // always reference ChildScopedList/ChildScopedResponse in their joins
        quote! {
            pub type #scoped_list_name = #list_name;
            pub type #scoped_response_name = #response_name;
        }
    }
}

/// `ScopeFilterable` for the list model and the API struct: hidden when any `exclude(scoped)` bool is set.
pub(crate) fn generate_scope_filterable_impls(
    all_fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    api_struct_name: &syn::Ident,
    list_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    // Generate ScopeFilterable impls for ListModel and API struct.
    // If this entity has exclude(scoped) boolean fields, the impl returns false
    // when those fields are true (ie. the record is private).
    // Parent entities use this trait to filter private children out of Vec joins
    // during scoped From conversions.
    let scope_filter_fields: Vec<_> = all_fields
        .iter()
        .filter(|f| {
            // Field must be exclude(scoped) AND included in list
            !should_include_in_model(f, "scoped_model")
                && should_include_in_model(f, "list_model")
                && is_bool_type(&f.ty)
        })
        .filter_map(|f| f.ident.as_ref())
        .collect();

    if scope_filter_fields.is_empty() {
        // No exclude(scoped) boolean fields: use default (always visible, no scope condition)
        quote! {
            impl crudcrate::ScopeFilterable for #list_name {}
            impl crudcrate::ScopeFilterable for #api_struct_name {}
        }
    } else {
        // Generate scope_condition() that returns a Condition filtering by the boolean fields.
        // E.g., for `is_private: bool` → `Condition::all().add(Column::IsPrivate.eq(false))`
        let scope_condition_adds: Vec<_> = scope_filter_fields
            .iter()
            .map(|field_name| {
                let col_pascal = column_ident(&field_name.to_string());
                quote! { .add(Column::#col_pascal.eq(false)) }
            })
            .collect();

        // Generate impl that checks all exclude(scoped) boolean fields
        // Record is visible only when ALL privacy booleans are false
        quote! {
            impl crudcrate::ScopeFilterable for #list_name {
                fn is_scope_visible(&self) -> bool {
                    #(!self.#scope_filter_fields)&&*
                }
                fn scope_condition() -> Option<sea_orm::Condition> {
                    use sea_orm::ColumnTrait;
                    Some(sea_orm::Condition::all() #(#scope_condition_adds)*)
                }
            }
            impl crudcrate::ScopeFilterable for #api_struct_name {
                fn is_scope_visible(&self) -> bool {
                    #(!self.#scope_filter_fields)&&*
                }
                fn scope_condition() -> Option<sea_orm::Condition> {
                    use sea_orm::ColumnTrait;
                    Some(sea_orm::Condition::all() #(#scope_condition_adds)*)
                }
            }
        }
    }
}
