//! Procedural macros for generating CRUD operations from Sea-ORM entities.
//!
//! **Main macro**: `#[derive(EntityToModels)]` - see [`entity_to_models`]
//!
//! # Available Attributes
//!
//! ## Struct-Level Attributes
//!
//! Use on the struct with `#[crudcrate(...)]`:
//!
//! | Attribute | Type | Description |
//! |-----------|------|-------------|
//! | `generate_router` | flag | Generate Axum router function |
//! | `api_struct = "Name"` | string | Override generated struct name |
//! | `name_singular = "item"` | string | Singular resource name for errors/headers |
//! | `name_plural = "items"` | string | Plural resource name for routes |
//! | `description = "..."` | string | `OpenAPI` description |
//! | `fulltext_language = "english"` | string | `PostgreSQL` fulltext language |
//! | `batch_limit = 100` | integer | Max items for batch create/update/delete |
//! | `max_page_size = 1000` | integer | Max items per page for pagination |
//! | `operations = MyOps` | path | Custom `CRUDOperations` implementation |
//! | `derive_partial_eq` | flag | Derive `PartialEq` on generated structs |
//! | `derive_eq` | flag | Derive `Eq` on generated structs |
//!
//! ### Hook Attributes
//!
//! Format: `{operation}::{cardinality}::{phase} = function_name`
//!
//! | Operation | Cardinality | Phase | Description |
//! |-----------|-------------|-------|-------------|
//! | `create` | `one`, `many` | `pre`, `body`, `transform`, `post` | Create hooks |
//! | `read` | `one`, `many` | `pre`, `body`, `transform`, `post` | Read hooks |
//! | `update` | `one`, `many` | `pre`, `body`, `transform`, `post` | Update hooks |
//! | `delete` | `one`, `many` | `pre`, `body`, `transform`, `post` | Delete hooks |
//!
//! Example: `#[crudcrate(create::one::pre = validate_input)]`
//!
//! ## Field-Level Attributes
//!
//! Use on fields with `#[crudcrate(...)]`:
//!
//! | Attribute | Type | Description |
//! |-----------|------|-------------|
//! | `primary_key` | flag | Mark as primary key field |
//! | `filterable` | flag | Enable filtering on this field |
//! | `sortable` | flag | Enable sorting on this field |
//! | `fulltext` | flag | Include in fulltext search |
//! | `exclude(create)` | list | Exclude from create model |
//! | `exclude(update)` | list | Exclude from update model |
//! | `exclude(one)` | list | Exclude from `get_one` response |
//! | `exclude(list)` | list | Exclude from `get_all` response |
//! | `on_create = expr` | expr | Auto-generate value on create |
//! | `on_update = expr` | expr | Auto-generate value on update |
//! | `non_db_attr` | flag | Mark as non-database field (for joins) |
//! | `join(one)` | config | Load in `get_one` only |
//! | `join(all)` | config | Load in `get_all` only |
//! | `join(one, all)` | config | Load in both endpoints |
//! | `join(one, all, depth = N)` | config | With max recursion depth (1-5) |
//! | `join_filterable("col1", "col2")` | list | Enable filtering on join columns |
//! | `join_sortable("col1", "col2")` | list | Enable sorting on join columns |
//!
//! # Examples
//!
//! ```ignore
//! #[derive(EntityToModels)]
//! #[crudcrate(
//!     generate_router,
//!     batch_limit = 500,
//!     create::one::pre = validate_task,
//! )]
//! #[sea_orm(table_name = "tasks")]
//! pub struct Model {
//!     #[sea_orm(primary_key, auto_increment = false)]
//!     #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
//!     pub id: Uuid,
//!
//!     #[crudcrate(filterable, sortable, fulltext)]
//!     pub title: String,
//!
//!     #[crudcrate(exclude(create, update), on_create = chrono::Utc::now())]
//!     pub created_at: DateTime<Utc>,
//!
//!     #[sea_orm(ignore)]
//!     #[crudcrate(non_db_attr, join(one, all, depth = 2))]
//!     pub comments: Vec<Comment>,
//! }
//! ```
//!
//! **Module guide**: `fields/` (field processing) | `codegen/` (models, handlers, joins, routes)

mod attribute_parser;
mod codegen;
mod fields;
mod macro_implementation;
mod relation_validator;
mod traits;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, parse_macro_input};
use traits::crudresource::structs::CRUDResourceMeta;

fn extract_active_model_type(
    input: &DeriveInput,
    name: &syn::Ident,
) -> Result<proc_macro2::TokenStream, proc_macro2::TokenStream> {
    for attr in &input.attrs {
        if attr.path().is_ident("active_model")
            && let Some(s) = attribute_parser::get_string_from_attr(attr)
        {
            return match syn::parse_str::<syn::Type>(&s) {
                Ok(ty) => Ok(quote! { #ty }),
                Err(_) => Err(syn::Error::new_spanned(
                    attr,
                    format!("Invalid active_model type: '{s}'. Expected a valid Rust type path."),
                )
                .to_compile_error()),
            };
        }
    }
    let ident = format_ident!("{}ActiveModel", name);
    Ok(quote! { #ident })
}

/// Generates `<Name>Create` struct with fields not excluded by `exclude(create)`.
/// Fields with `on_create` become `Option<T>` to allow user override.
/// Implements `From<NameCreate>` for `ActiveModel` with automatic value generation.
#[proc_macro_derive(ToCreateModel, attributes(crudcrate, active_model))]
pub fn to_create_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let create_name = format_ident!("{}Create", name);

    let active_model_type = match extract_active_model_type(&input, name) {
        Ok(ty) => ty,
        Err(e) => return e.into(),
    };
    let fields = match fields::extract_named_fields(&input) {
        Ok(f) => f,
        Err(e) => return e,
    };
    let create_struct_fields = codegen::models::create::generate_create_struct_fields(&fields);
    let conv_lines = codegen::models::create::generate_create_conversion_lines(&fields);

    // Always include ToSchema for Create models
    // Circular dependencies are handled by schema(no_recursion) on join fields in the main model
    let create_derives =
        quote! { Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema };

    let expanded = quote! {
        #[derive(#create_derives)]
        pub struct #create_name {
            #(#create_struct_fields),*
        }

        impl From<#create_name> for #active_model_type {
            fn from(create: #create_name) -> Self {
                #active_model_type {
                    #(#conv_lines),*
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// Generates `<Name>Update` struct with fields not excluded by `exclude(update)`.
/// All fields are `Option<Option<T>>` to support partial updates and explicit null.
/// Implements `MergeIntoActiveModel` trait with `on_update` expression handling.
#[proc_macro_derive(ToUpdateModel, attributes(crudcrate, active_model))]
pub fn to_update_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let update_name = format_ident!("{}Update", name);

    let active_model_type = match extract_active_model_type(&input, name) {
        Ok(ty) => ty,
        Err(e) => return e.into(),
    };
    let fields = match fields::extract_named_fields(&input) {
        Ok(f) => f,
        Err(e) => return e,
    };
    let included_fields = crate::codegen::models::update::filter_update_fields(&fields);
    let update_struct_fields =
        crate::codegen::models::update::generate_update_struct_fields(&included_fields);
    let included_merge = codegen::models::merge::generate_included_merge_code(&included_fields);
    let excluded_merge = codegen::models::merge::generate_excluded_merge_code(&fields);

    // Always include ToSchema for Update models
    // Circular dependencies are handled by schema(no_recursion) on join fields in the main model
    let update_derives =
        quote! { Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema };

    let expanded = quote! {
        #[derive(#update_derives)]
        pub struct #update_name {
            #(#update_struct_fields),*
        }

        impl #update_name {
            pub fn merge_fields(self, mut model: #active_model_type) -> Result<#active_model_type, crudcrate::ApiError> {
                #(#included_merge)*
                #(#excluded_merge)*
                Ok(model)
            }
        }

        impl crudcrate::traits::MergeIntoActiveModel<#active_model_type> for #update_name {
            fn merge_into_activemodel(self, model: #active_model_type) -> Result<#active_model_type, crudcrate::ApiError> {
                Self::merge_fields(self, model)
            }
        }
    };

    TokenStream::from(expanded)
}

/// Generates `<Name>List` struct with fields not excluded by `exclude(list)`.
/// Optimizes API payloads by excluding heavy fields (joins, large text) from list endpoints.
/// Implements `From<Name>` and `From<Model>` conversions.
#[proc_macro_derive(ToListModel, attributes(crudcrate))]
pub fn to_list_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let list_name = format_ident!("{}List", name);

    let fields = match fields::extract_named_fields(&input) {
        Ok(f) => f,
        Err(e) => return e,
    };
    let list_struct_fields = crate::codegen::models::list::generate_list_struct_fields(&fields);
    let list_from_assignments =
        crate::codegen::models::list::generate_list_from_assignments(&fields);

    // Always include ToSchema for List models
    // Circular dependencies are handled by schema(no_recursion) on join fields in the main model
    let list_derives = quote! { Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema };

    let expanded = quote! {
        #[derive(#list_derives)]
        pub struct #list_name {
            #(#list_struct_fields),*
        }

        impl From<#name> for #list_name {
            fn from(model: #name) -> Self {
                Self {
                    #(#list_from_assignments),*
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// Generates complete CRUD API structures from Sea-ORM entities.
///
/// Creates API struct, List/Response models, and `CRUDResource` implementation.
/// Supports custom functions, joins, filtering, sorting, and fulltext search.
///
/// Key attributes: `api_struct`, `generate_router`, `exclude()`, `join()`, `on_create/update`.
/// See crate documentation for full attribute reference and examples.
/// # Panics
///
/// This function will panic in the following cases:
/// - When deprecated syntax is used (e.g., `create_model = false` instead of `exclude(create)`)
/// - When there are cyclic join dependencies without explicit depth specification
/// - When required Sea-ORM relation enums are missing for join fields
#[proc_macro_derive(EntityToModels, attributes(crudcrate))]
pub fn entity_to_models(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    // Parse and validate attributes
    let (api_struct_name, active_model_path) = fields::parse_entity_attributes(&input, struct_name);
    let table_name = attribute_parser::extract_table_name(&input.attrs)
        .unwrap_or_else(|| struct_name.to_string());
    let meta = attribute_parser::parse_crud_resource_meta(&input.attrs);

    // Check for deprecation errors (legacy fn_* syntax)
    if !meta.deprecation_errors.is_empty() {
        let errors: proc_macro2::TokenStream = meta
            .deprecation_errors
            .iter()
            .map(syn::Error::to_compile_error)
            .collect();
        return errors.into();
    }

    let crud_meta = meta.with_defaults(&table_name);

    // Validate active model path
    if syn::parse_str::<syn::Type>(&active_model_path).is_err() {
        return syn::Error::new_spanned(
            &input,
            format!("Invalid active_model path: {active_model_path}"),
        )
        .to_compile_error()
        .into();
    }

    // Extract fields and create field analysis
    let fields = match fields::extract_entity_fields(&input) {
        Ok(f) => f,
        Err(e) => return e,
    };
    let field_analysis = match fields::analyze_entity_fields(fields) {
        Ok(a) => a,
        Err(e) => return e,
    };
    if let Err(e) = fields::validate_field_analysis(&field_analysis) {
        return e;
    }

    // Validate aggregate config against actual entity fields
    if let Some(ref agg_config) = crud_meta.aggregate {
        if let Err(e) = fields::validate_aggregate_config(agg_config, &field_analysis) {
            return e;
        }
    }

    // Setup join validation - check for cyclic dependencies
    let cyclic_dependency_check = relation_validator::generate_cyclic_dependency_check(
        &field_analysis,
        &api_struct_name.to_string(),
    );
    if !cyclic_dependency_check.is_empty() {
        return cyclic_dependency_check.into();
    }

    // Build filterable column info for aggregate codegen
    use convert_case::{Case, Casing};
    let filterable_columns: codegen::handlers::aggregate::FilterableColumnInfo = field_analysis
        .filterable_fields
        .iter()
        .map(|f| {
            let name = f.ident.as_ref().unwrap().to_string();
            let pascal = name.to_case(Case::Pascal);
            (name, pascal)
        })
        .collect();

    // Detect aggregate-only mode: has aggregate config but no generate_router
    let aggregate_only = crud_meta.aggregate.is_some() && !crud_meta.generate_router;

    if aggregate_only {
        // Aggregate-only: generate a unit struct with aggregate_query + aggregate_router
        // NO CRUDResource impl, NO Create/Update/List models, NO CRUD handlers
        let aggregate_router = crate::codegen::router::axum::generate_aggregate_router_impl(
            &api_struct_name,
            &crud_meta,
            &filterable_columns,
        );

        let expanded = quote! {
            /// Aggregate-only API struct (no CRUD endpoints).
            pub struct #api_struct_name;

            #aggregate_router
        };

        return TokenStream::from(expanded);
    }

    // Full mode: generate API struct, CRUDResource impl, models, router
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

    let router_impl = if crud_meta.generate_router && has_crud_resource_fields {
        crate::codegen::router::axum::generate_router_impl(
            &api_struct_name,
            &crud_meta,
            &filterable_columns,
        )
    } else {
        quote! {}
    };

    let crud_impl = quote! {
        #crud_impl_inner
        #router_impl
    };

    // Generate list and response models
    let (list_model, response_model) =
        codegen::models::list_response::generate_list_and_response_models(
            &input,
            &api_struct_name,
            struct_name,
            &field_analysis,
        );

    // Generate final output
    let expanded = quote! {
        #api_struct
        #from_impl
        #crud_impl
        #list_model
        #response_model
    };

    TokenStream::from(expanded)
}
