//! `EntityToModels` expansion: the API struct, `CRUDResource` impl, router, list and response models.

use quote::quote;
use syn::DeriveInput;

use crate::expand::screening::screen_tokens;
use crate::{attrs, codegen, fields, macro_implementation, relation_validator};

pub(crate) fn entity_to_models_impl(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let input = match syn::parse2::<DeriveInput>(input) {
        Ok(input) => input,
        Err(e) => return e.to_compile_error(),
    };
    if let Some(tokens) = screen_tokens(&input) {
        return tokens;
    }
    let struct_name = &input.ident;

    // Parse and validate attributes
    let (api_struct_name, active_model_path) = fields::parse_entity_attributes(&input, struct_name);
    let table_name =
        attrs::extract_table_name(&input.attrs).unwrap_or_else(|| struct_name.to_string());
    let meta = attrs::parse_crud_resource_meta(&input.attrs);

    // Check for deprecation errors (legacy fn_* syntax)
    if !meta.deprecation_errors.is_empty() {
        let errors: proc_macro2::TokenStream = meta
            .deprecation_errors
            .iter()
            .map(syn::Error::to_compile_error)
            .collect();
        return errors;
    }

    let crud_meta = meta.with_defaults(&table_name);

    // Validate active model path
    if syn::parse_str::<syn::Type>(&active_model_path).is_err() {
        return syn::Error::new_spanned(
            &input,
            format!("Invalid active_model path: {active_model_path}"),
        )
        .to_compile_error();
    }

    // Extract fields and create field analysis
    let fields = match fields::extract_entity_fields(&input) {
        Ok(f) => f,
        Err(e) => return e,
    };

    // Create synthetic fields from struct-level join definitions.
    // These exist ONLY on the generated API struct, not on the SeaORM Model.
    let mut synthetic_join_fields: Vec<syn::Field> = Vec::new();
    for j in &crud_meta.struct_level_joins {
        match attrs::join::synthetic_join_field(j, &input) {
            Ok(Some(field)) => synthetic_join_fields.push(field),
            Ok(None) => {}
            Err(e) => return e,
        }
    }

    let field_analysis = match fields::analyze_entity_fields(fields, &synthetic_join_fields) {
        Ok(a) => a,
        Err(e) => return e,
    };
    if let Err(e) = fields::validate_field_analysis(&field_analysis) {
        return e;
    }

    // Generate core API model components
    let (api_struct_fields, from_model_assignments) =
        codegen::models::api_struct::generate_api_struct_content(&field_analysis, &api_struct_name);
    let api_struct = codegen::models::api_struct::generate_api_struct(
        &api_struct_name,
        &api_struct_fields,
        &active_model_path,
        &crud_meta,
        &field_analysis,
    );
    let from_impl = quote! {
        impl From<#struct_name> for #api_struct_name {
            fn from(model: #struct_name) -> Self {
                Self {
                    #(#from_model_assignments),*
                }
            }
        }
    };

    // Generate CRUD implementation
    let has_crud_resource_fields = field_analysis.primary_key_field.is_some()
        || !field_analysis.sortable_fields.is_empty()
        || !field_analysis.filterable_fields.is_empty()
        || !field_analysis.fulltext_fields.is_empty();

    let crud_impl_inner = if has_crud_resource_fields {
        macro_implementation::generate_crud_resource_impl(
            &api_struct_name,
            &crud_meta,
            &active_model_path,
            &field_analysis,
            &table_name,
        )
    } else {
        quote! {}
    };

    // Detect exclude(scoped) fields with the same predicate model generation
    // uses, so the router only wires scoped structs that actually exist.
    let has_scoped_fields = field_analysis
        .db_fields
        .iter()
        .chain(field_analysis.non_db_fields.iter())
        .any(|f| crate::codegen::models::is_scoped_exclusion(f));

    let router_impl = if crud_meta.generate_router && has_crud_resource_fields {
        crate::codegen::router::generate_router_impl(&api_struct_name, has_scoped_fields)
    } else {
        quote! {}
    };

    let crud_impl = quote! {
        #crud_impl_inner
        #router_impl
    };

    // Generate list and response models
    let (list_model, response_model) = codegen::models::emit::generate_list_and_response_models(
        &input,
        &api_struct_name,
        struct_name,
        &field_analysis,
        &synthetic_join_fields,
    );

    // Generate compile-time bidirectional relation detection
    let bidirectional_checks = relation_validator::generate_bidirectional_checks(
        &field_analysis,
        &api_struct_name.to_string(),
    );

    // Generate final output
    let expanded = quote! {
        #api_struct
        #from_impl
        #crud_impl
        #list_model
        #response_model
        #bidirectional_checks
    };

    expanded
}
