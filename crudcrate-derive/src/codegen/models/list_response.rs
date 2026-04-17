//! List and Response model generation orchestration
//!
//! This module coordinates the generation of both List and Response models
//! from entity definitions, using the dedicated list and response generators.

use crate::codegen::joins::get_join_config;
use crate::codegen::models::should_include_in_model;
use crate::codegen::type_resolution::{
    inner_list_type_of_option, inner_list_type_of_vec, is_option_type, is_vec_type,
    transform_type_to_scoped_list_variant,
};
use crate::fields;
use crate::traits::crudresource::structs::EntityFieldAnalysis;
use quote::{format_ident, quote};
use syn::DeriveInput;

/// Check if a type is `bool` (plain, not Option<bool>)
fn is_bool_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        type_path.path.is_ident("bool")
    } else {
        false
    }
}

/// Generates both List and Response models from entity definition
///
/// `struct_level_joins` are synthetic fields from struct-level `join(...)` attributes.
///
/// Returns a tuple of (`list_model_tokens`, `response_model_tokens`)
pub(crate) fn generate_list_and_response_models(
    input: &DeriveInput,
    api_struct_name: &syn::Ident,
    struct_name: &syn::Ident,
    field_analysis: &EntityFieldAnalysis,
    struct_level_joins: &[syn::Field],
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    // Generate List model
    let list_name = format_ident!("{}List", api_struct_name);
    let raw_fields = match fields::extract_named_fields(input) {
        Ok(f) => f,
        Err(_e) => {
            return (quote::quote! {}, quote::quote! {});
        }
    };

    // Combine real fields with synthetic join fields
    let mut all_fields = raw_fields.clone();
    for field in struct_level_joins {
        all_fields.push(field.clone());
    }

    let list_struct_fields = crate::codegen::models::list::generate_list_struct_fields(&all_fields, api_struct_name);
    let list_from_assignments =
        crate::codegen::models::list::generate_list_from_assignments(&all_fields);
    let list_from_model_assignments =
        crate::codegen::models::list::generate_list_from_model_assignments(field_analysis);

    let list_derives =
        quote! { Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema };

    let list_model = quote! {
        #[derive(#list_derives)]
        pub struct #list_name {
            #(#list_struct_fields),*
        }

        impl From<#api_struct_name> for #list_name {
            fn from(model: #api_struct_name) -> Self {
                Self {
                    #(#list_from_assignments),*
                }
            }
        }

        impl From<#struct_name> for #list_name {
            fn from(model: #struct_name) -> Self {
                Self {
                    #(#list_from_model_assignments),*
                }
            }
        }
    };

    // Generate Response model
    let response_name = format_ident!("{}Response", api_struct_name);
    let response_struct_fields = crate::codegen::models::response::generate_response_struct_fields(
        &all_fields,
        api_struct_name,
    );
    let response_from_assignments =
        crate::codegen::models::response::generate_response_from_assignments(&all_fields);

    let response_derives =
        quote! { Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema };

    let response_model = quote! {
        #[derive(#response_derives)]
        pub struct #response_name {
            #(#response_struct_fields),*
        }

        impl From<#api_struct_name> for #response_name {
            fn from(model: #api_struct_name) -> Self {
                Self {
                    #(#response_from_assignments),*
                }
            }
        }
    };

    // Always generate ScopedList/ScopedResponse so parent entities can reference
    // child scoped types in their own scoped models (join fields).
    let has_scoped_exclusions = all_fields
        .iter()
        .any(|f| !should_include_in_model(f, "scoped_model") && should_include_in_model(f, "list_model"));

    let scoped_list_name = format_ident!("{}ScopedList", api_struct_name);
    let scoped_response_name = format_ident!("{}ScopedResponse", api_struct_name);

    let scoped_models = if has_scoped_exclusions {
        // Entity has exclude(scoped) fields — generate distinct scoped structs

        // ScopedList: fields included in list AND not exclude(scoped)
        // Join(all) fields use the *ScopedList* child type so that excluded
        // fields on children are also stripped from nested responses.
        let scoped_list_fields: Vec<_> = all_fields
            .iter()
            .filter(|f| should_include_in_model(f, "list_model") && should_include_in_model(f, "scoped_model"))
            .map(|f| {
                let ident = &f.ident;
                let is_join_all = get_join_config(f).is_some_and(|c| c.on_all);
                if is_join_all {
                    let scoped_ty = transform_type_to_scoped_list_variant(&f.ty, api_struct_name);
                    quote! { pub #ident: #scoped_ty }
                } else {
                    let ty = &f.ty;
                    quote! { pub #ident: #ty }
                }
            })
            .collect();

        // From<ListModel> for ScopedList — join Vec fields need per-element conversion
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
            .filter(|f| should_include_in_model(f, "one_model") && should_include_in_model(f, "scoped_model"))
            .map(|f| {
                let ident = &f.ident;
                let is_join = get_join_config(f).is_some();
                if is_join {
                    let scoped_ty = transform_type_to_scoped_list_variant(&f.ty, api_struct_name);
                    quote! { pub #ident: #scoped_ty }
                } else {
                    let ty = &f.ty;
                    quote! { pub #ident: #ty }
                }
            })
            .collect();

        // From<ResponseModel> for ScopedResponse — join fields need chained conversion
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
        // No exclude(scoped) fields — generate type aliases so parents can
        // always reference ChildScopedList/ChildScopedResponse in their joins
        quote! {
            pub type #scoped_list_name = #list_name;
            pub type #scoped_response_name = #response_name;
        }
    };

    // Generate ScopeFilterable impls for ListModel and API struct.
    // If this entity has exclude(scoped) boolean fields, the impl returns false
    // when those fields are true (i.e., the record is private).
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

    let scope_filterable_impls = if scope_filter_fields.is_empty() {
        // No exclude(scoped) boolean fields — use default (always visible, no scope condition)
        quote! {
            impl crudcrate::ScopeFilterable for #list_name {}
            impl crudcrate::ScopeFilterable for #api_struct_name {}
        }
    } else {
        // Generate scope_condition() that returns a Condition filtering by the boolean fields.
        // E.g., for `is_private: bool` → `Condition::all().add(Column::IsPrivate.eq(false))`
        use convert_case::{Case, Casing};
        let scope_condition_adds: Vec<_> = scope_filter_fields
            .iter()
            .map(|field_name| {
                let col_pascal = quote::format_ident!("{}", field_name.to_string().to_case(Case::Pascal));
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
    };

    let combined_list = quote! { #list_model #scoped_models #scope_filterable_impls };
    (combined_list, response_model)
}
