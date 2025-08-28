
use super::structs::{CRUDResourceMeta, EntityFieldAnalysis};
use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::parse::Parser;
use syn::{Data, DeriveInput, Fields, Lit, Meta, punctuated::Punctuated, token::Comma};

pub(super) fn parse_crud_resource_meta(attrs: &[syn::Attribute]) -> CRUDResourceMeta {
    super::attribute_parser::parse_crud_resource_meta(attrs)
}

pub(super) fn extract_table_name(attrs: &[syn::Attribute]) -> Option<String> {
    super::attribute_parser::extract_table_name(attrs)
}

pub(super) fn field_is_optional(field: &syn::Field) -> bool {
    super::field_analyzer::field_is_optional(field)
}

pub(super) fn get_crudcrate_bool(field: &syn::Field, key: &str) -> Option<bool> {
    super::attribute_parser::get_crudcrate_bool(field, key)
}

pub(super) fn get_crudcrate_expr(field: &syn::Field, key: &str) -> Option<syn::Expr> {
    super::attribute_parser::get_crudcrate_expr(field, key)
}

pub(super) fn get_string_from_attr(attr: &syn::Attribute) -> Option<String> {
    super::attribute_parser::get_string_from_attr(attr)
}

pub(super) fn field_has_crudcrate_flag(field: &syn::Field, flag: &str) -> bool {
    super::attribute_parser::field_has_crudcrate_flag(field, flag)
}

/// Resolves the target models (Create/Update/List) for a field with `use_target_models` attribute.
/// Returns (`CreateModel`, `UpdateModel`, `ListModel`) types for the target `CRUDResource`.
pub(super) fn resolve_target_models_with_list(
    field_type: &syn::Type,
) -> Option<(syn::Type, syn::Type, syn::Type)> {
    super::field_analyzer::resolve_target_models_with_list(field_type)
}

/// Resolves the target models (Create/Update) for a field with `use_target_models` attribute.
/// Returns (`CreateModel`, `UpdateModel`) types for the target `CRUDResource`.
pub(super) fn resolve_target_models(field_type: &syn::Type) -> Option<(syn::Type, syn::Type)> {
    super::field_analyzer::resolve_target_models(field_type)
}

// ================================
// ToCreateModel helper functions
// ================================

pub(super) fn extract_active_model_type(
    input: &DeriveInput,
    name: &syn::Ident,
) -> proc_macro2::TokenStream {
    let mut active_model_override = None;
    for attr in &input.attrs {
        if attr.path().is_ident("active_model")
            && let Some(s) = get_string_from_attr(attr)
        {
            active_model_override =
                Some(syn::parse_str::<syn::Type>(&s).expect("Invalid active_model type"));
        }
    }
    if let Some(ty) = active_model_override {
        quote! { #ty }
    } else {
        let ident = format_ident!("{}ActiveModel", name);
        quote! { #ident }
    }
}

pub(super) fn extract_named_fields(
    input: &DeriveInput,
) -> syn::punctuated::Punctuated<syn::Field, syn::token::Comma> {
    if let Data::Struct(data) = &input.data {
        if let Fields::Named(named) = &data.fields {
            named.named.clone()
        } else {
            panic!("ToCreateModel only supports structs with named fields");
        }
    } else {
        panic!("ToCreateModel can only be derived for structs");
    }
}

pub(super) fn generate_create_struct_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    super::code_generator::generate_create_struct_fields(fields)
}

pub(super) fn generate_create_conversion_lines(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    super::code_generator::generate_create_conversion_lines(fields)
}

// ================================
// ToUpdateModel helper functions
// ================================

pub(super) fn filter_update_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<&syn::Field> {
    super::code_generator::filter_update_fields(fields)
}

pub(super) fn generate_update_struct_fields(
    included_fields: &[&syn::Field],
) -> Vec<proc_macro2::TokenStream> {
    super::code_generator::generate_update_struct_fields(included_fields)
}

pub(super) fn extract_inner_type_for_update(ty: &syn::Type) -> syn::Type {
    super::field_analyzer::extract_inner_type_for_update(ty)
}

pub(super) fn generate_update_merge_code(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    included_fields: &[&syn::Field],
) -> (Vec<proc_macro2::TokenStream>, Vec<proc_macro2::TokenStream>) {
    let included_merge = generate_included_merge_code(included_fields);
    let excluded_merge = generate_excluded_merge_code(fields);
    (included_merge, excluded_merge)
}

pub(super) fn generate_included_merge_code(
    included_fields: &[&syn::Field],
) -> Vec<proc_macro2::TokenStream> {
    included_fields
        .iter()
        .filter(|field| !get_crudcrate_bool(field, "non_db_attr").unwrap_or(false))
        .map(|field| {
            let ident = &field.ident;
            let is_optional = field_is_optional(field);

            if is_optional {
                quote! {
                    model.#ident = match self.#ident {
                        Some(Some(value)) => sea_orm::ActiveValue::Set(Some(value.into())),
                        Some(None)      => sea_orm::ActiveValue::Set(None),
                        None            => sea_orm::ActiveValue::NotSet,
                    };
                }
            } else {
                quote! {
                    model.#ident = match self.#ident {
                        Some(Some(value)) => sea_orm::ActiveValue::Set(value.into()),
                        Some(None) => {
                            return Err(sea_orm::DbErr::Custom(format!(
                                "Field '{}' is required and cannot be set to null",
                                stringify!(#ident)
                            )));
                        },
                        None => sea_orm::ActiveValue::NotSet,
                    };
                }
            }
        })
        .collect()
}

pub(super) fn generate_excluded_merge_code(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| {
            get_crudcrate_bool(field, "update_model") == Some(false)
                && !get_crudcrate_bool(field, "non_db_attr").unwrap_or(false)
        })
        .filter_map(|field| {
            if let Some(expr) = get_crudcrate_expr(field, "on_update") {
                let ident = &field.ident;
                if field_is_optional(field) {
                    Some(quote! {
                        model.#ident = sea_orm::ActiveValue::Set(Some((#expr).into()));
                    })
                } else {
                    Some(quote! {
                        model.#ident = sea_orm::ActiveValue::Set((#expr).into());
                    })
                }
            } else {
                None
            }
        })
        .collect()
}


// ================================
// EntityToModels helper functions
// ================================

pub(super) fn extract_entity_fields(
    input: &DeriveInput,
) -> Result<&syn::punctuated::Punctuated<syn::Field, syn::token::Comma>, TokenStream> {
    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => Ok(&fields.named),
            _ => Err(syn::Error::new_spanned(
                input,
                "EntityToModels only supports structs with named fields",
            )
            .to_compile_error()
            .into()),
        },
        _ => Err(
            syn::Error::new_spanned(input, "EntityToModels only supports structs")
                .to_compile_error()
                .into(),
        ),
    }
}

pub(super) fn parse_entity_attributes(
    input: &DeriveInput,
    struct_name: &syn::Ident,
) -> (syn::Ident, String) {
    let mut api_struct_name = None;
    let mut active_model_path = None;

    for attr in &input.attrs {
        if attr.path().is_ident("crudcrate")
            && let Meta::List(meta_list) = &attr.meta
                && let Ok(metas) =
                    Punctuated::<Meta, Comma>::parse_terminated.parse2(meta_list.tokens.clone())
                {
                    for meta in &metas {
                        if let Meta::NameValue(nv) = meta {
                            if nv.path.is_ident("api_struct") {
                                if let syn::Expr::Lit(expr_lit) = &nv.value
                                    && let Lit::Str(s) = &expr_lit.lit {
                                        api_struct_name = Some(format_ident!("{}", s.value()));
                                    }
                            } else if nv.path.is_ident("active_model")
                                && let syn::Expr::Lit(expr_lit) = &nv.value
                                    && let Lit::Str(s) = &expr_lit.lit {
                                        active_model_path = Some(s.value());
                                    }
                        }
                    }
                }
    }

    let table_name = extract_table_name(&input.attrs).unwrap_or_else(|| struct_name.to_string());
    let api_struct_name =
        api_struct_name.unwrap_or_else(|| format_ident!("{}", table_name.to_case(Case::Pascal)));
    let active_model_path = active_model_path.unwrap_or_else(|| "ActiveModel".to_string());

    (api_struct_name, active_model_path)
}

pub(super) fn analyze_entity_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> EntityFieldAnalysis<'_> {
    let mut analysis = EntityFieldAnalysis {
        db_fields: Vec::new(),
        non_db_fields: Vec::new(),
        primary_key_field: None,
        sortable_fields: Vec::new(),
        filterable_fields: Vec::new(),
        fulltext_fields: Vec::new(),
    };

    for field in fields {
        let is_non_db = get_crudcrate_bool(field, "non_db_attr").unwrap_or(false);
        if is_non_db {
            analysis.non_db_fields.push(field);
        } else {
            analysis.db_fields.push(field);

            if field_has_crudcrate_flag(field, "primary_key") {
                analysis.primary_key_field = Some(field);
            }
            if field_has_crudcrate_flag(field, "sortable") {
                analysis.sortable_fields.push(field);
            }
            if field_has_crudcrate_flag(field, "filterable") {
                analysis.filterable_fields.push(field);
            }
            if field_has_crudcrate_flag(field, "fulltext") {
                analysis.fulltext_fields.push(field);
            }
        }
    }

    analysis
}

pub(super) fn validate_field_analysis(analysis: &EntityFieldAnalysis) -> Result<(), TokenStream> {
    if analysis.primary_key_field.is_some()
        && analysis
            .db_fields
            .iter()
            .filter(|field| field_has_crudcrate_flag(field, "primary_key"))
            .count()
            > 1
    {
        return Err(syn::Error::new_spanned(
            analysis.primary_key_field.unwrap(),
            "Only one field can be marked with 'primary_key' attribute",
        )
        .to_compile_error()
        .into());
    }
    Ok(())
}

pub(super) fn generate_api_struct_content(
    analysis: &EntityFieldAnalysis,
) -> (Vec<proc_macro2::TokenStream>, Vec<proc_macro2::TokenStream>) {
    let mut api_struct_fields = Vec::new();
    let mut from_model_assignments = Vec::new();

    for field in &analysis.db_fields {
        let field_name = &field.ident;
        let field_type = &field.ty;

        let api_field_attrs: Vec<_> = field
            .attrs
            .iter()
            .filter(|attr| !attr.path().is_ident("sea_orm"))
            .collect();

        api_struct_fields.push(quote! {
            #(#api_field_attrs)*
            pub #field_name: #field_type
        });

        let assignment = if field_type
            .to_token_stream()
            .to_string()
            .contains("DateTimeWithTimeZone")
        {
            if field_is_optional(field) {
                quote! {
                    #field_name: model.#field_name.map(|dt| dt.with_timezone(&chrono::Utc))
                }
            } else {
                quote! {
                    #field_name: model.#field_name.with_timezone(&chrono::Utc)
                }
            }
        } else {
            quote! {
                #field_name: model.#field_name
            }
        };

        from_model_assignments.push(assignment);
    }

    for field in &analysis.non_db_fields {
        let field_name = &field.ident;
        let field_type = &field.ty;

        let default_expr = get_crudcrate_expr(field, "default")
            .unwrap_or_else(|| syn::parse_quote!(Default::default()));

        // Preserve all original crudcrate attributes while ensuring required ones are present
        let crudcrate_attrs: Vec<_> = field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("crudcrate"))
            .collect();

        api_struct_fields.push(quote! {
            #(#crudcrate_attrs)*
            pub #field_name: #field_type
        });

        from_model_assignments.push(quote! {
            #field_name: #default_expr
        });
    }

    (api_struct_fields, from_model_assignments)
}

pub(super) fn generate_api_struct(
    api_struct_name: &syn::Ident,
    api_struct_fields: &[proc_macro2::TokenStream],
    active_model_path: &str,
) -> proc_macro2::TokenStream {
    quote! {
        use sea_orm::ActiveValue;
        use utoipa::ToSchema;
        use serde::{Serialize, Deserialize};
        use crudcrate::{ToUpdateModel, ToCreateModel};

        #[derive(Clone, Debug, PartialEq, Eq, ToSchema, Serialize, Deserialize, ToUpdateModel, ToCreateModel)]
        #[active_model = #active_model_path]
        pub struct #api_struct_name {
            #(#api_struct_fields),*
        }
    }
}

pub(super) fn generate_from_impl(
    struct_name: &syn::Ident,
    api_struct_name: &syn::Ident,
    from_model_assignments: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    quote! {
        impl From<#struct_name> for #api_struct_name {
            fn from(model: #struct_name) -> Self {
                Self {
                    #(#from_model_assignments),*
                }
            }
        }
    }
}

pub(super) fn generate_conditional_crud_impl(
    api_struct_name: &syn::Ident,
    crud_meta: &CRUDResourceMeta,
    active_model_path: &str,
    analysis: &EntityFieldAnalysis,
    table_name: &str,
) -> proc_macro2::TokenStream {
    let has_crud_resource_fields = analysis.primary_key_field.is_some()
        || !analysis.sortable_fields.is_empty()
        || !analysis.filterable_fields.is_empty()
        || !analysis.fulltext_fields.is_empty();

    let crud_impl = if has_crud_resource_fields {
        generate_crud_resource_impl(api_struct_name, crud_meta, active_model_path, analysis, table_name)
    } else {
        quote! {}
    };

    let router_impl = if crud_meta.generate_router && has_crud_resource_fields {
        generate_router_impl(api_struct_name)
    } else {
        quote! {}
    };

    // Auto-registration now happens automatically for all models,
    // so we don't need the manual registration method anymore

    quote! {
        #crud_impl
        #router_impl
    }
}

pub(super) fn generate_router_impl(api_struct_name: &syn::Ident) -> proc_macro2::TokenStream {
    let create_model_name = format_ident!("{}Create", api_struct_name);
    let update_model_name = format_ident!("{}Update", api_struct_name);
    let list_model_name = format_ident!("{}List", api_struct_name);

    quote! {
        // Generate CRUD handlers using the crudcrate macro
        crudcrate::crud_handlers!(#api_struct_name, #update_model_name, #create_model_name, #list_model_name);

        /// Generate router with all CRUD endpoints
        pub fn router(db: &sea_orm::DatabaseConnection) -> utoipa_axum::router::OpenApiRouter
        where
            #api_struct_name: crudcrate::traits::CRUDResource,
        {
            use utoipa_axum::{router::OpenApiRouter, routes};

            OpenApiRouter::new()
                .routes(routes!(get_one_handler))
                .routes(routes!(get_all_handler))
                .routes(routes!(create_one_handler))
                .routes(routes!(update_one_handler))
                .routes(routes!(delete_one_handler))
                .routes(routes!(delete_many_handler))
                .with_state(db.clone())
        }
    }
}

pub(super) fn generate_crud_type_aliases(
    api_struct_name: &syn::Ident,
    crud_meta: &CRUDResourceMeta,
    active_model_path: &str,
) -> (
    syn::Ident,
    syn::Ident,
    syn::Ident,
    syn::Type,
    syn::Type,
    syn::Type,
) {
    let create_model_name = format_ident!("{}Create", api_struct_name);
    let update_model_name = format_ident!("{}Update", api_struct_name);
    let list_model_name = format_ident!("{}List", api_struct_name);

    let entity_type: syn::Type = crud_meta
        .entity_type
        .as_ref()
        .and_then(|s| syn::parse_str(s).ok())
        .unwrap_or_else(|| syn::parse_quote!(Entity));

    let column_type: syn::Type = crud_meta
        .column_type
        .as_ref()
        .and_then(|s| syn::parse_str(s).ok())
        .unwrap_or_else(|| syn::parse_quote!(Column));

    let active_model_type: syn::Type =
        syn::parse_str(active_model_path).unwrap_or_else(|_| syn::parse_quote!(ActiveModel));

    (
        create_model_name,
        update_model_name,
        list_model_name,
        entity_type,
        column_type,
        active_model_type,
    )
}

pub(super) fn generate_id_column(
    primary_key_field: Option<&syn::Field>,
) -> proc_macro2::TokenStream {
    if let Some(pk_field) = primary_key_field {
        let field_name = &pk_field.ident.as_ref().unwrap();
        let column_name = format_ident!("{}", ident_to_string(field_name).to_case(Case::Pascal));
        quote! { Self::ColumnType::#column_name }
    } else {
        quote! { Self::ColumnType::Id }
    }
}

pub(super) fn generate_field_entries(fields: &[&syn::Field]) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let field_str = ident_to_string(field_name);
            let column_name = format_ident!("{}", field_str.to_case(Case::Pascal));
            quote! { (#field_str, Self::ColumnType::#column_name) }
        })
        .collect()
}

pub(super) fn generate_like_filterable_entries(
    fields: &[&syn::Field],
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter_map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let field_str = ident_to_string(field_name);

            // Check if this field should use LIKE queries based on its type
            if is_text_type(&field.ty) {
                Some(quote! { #field_str })
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn generate_fulltext_field_entries(
    fields: &[&syn::Field],
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let field_str = ident_to_string(field_name);
            let column_name = format_ident!("{}", field_str.to_case(Case::Pascal));
            quote! { (#field_str, Self::ColumnType::#column_name) }
        })
        .collect()
}

/// Generate enum field checker using explicit annotations only
/// Users must mark enum fields with `#[crudcrate(enum_field)]` for enum filtering to work
fn generate_enum_field_checker(all_fields: &[&syn::Field]) -> proc_macro2::TokenStream {
    let field_checks: Vec<proc_macro2::TokenStream> = all_fields
        .iter()
        .filter_map(|field| {
            if let Some(field_name) = &field.ident {
                let field_name_str = ident_to_string(field_name);
                let is_enum = field_has_crudcrate_flag(field, "enum_field");

                Some(quote! {
                    #field_name_str => #is_enum,
                })
            } else {
                None
            }
        })
        .collect();

    quote! {
        match field_name {
            #(#field_checks)*
            _ => false,
        }
    }
}

/// Helper function to handle raw identifiers properly by stripping the r# prefix
fn ident_to_string(ident: &syn::Ident) -> String {
    let ident_str = ident.to_string();
    if let Some(stripped) = ident_str.strip_prefix("r#") {
        stripped.to_string() // Strip "r#" prefix from raw identifiers
    } else {
        ident_str
    }
}

/// Check if a type is a text type (String or &str), handling Option<T> wrappers
fn is_text_type(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(type_path) => {
            if let Some(last_seg) = type_path.path.segments.last() {
                let ident = &last_seg.ident;

                // Handle Option<T> - check the inner type
                if ident == "Option"
                    && let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments
                    && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
                {
                    return is_text_type(inner_ty);
                }

                // Check if it's String (could be std::string::String or just String)
                ident == "String"
            } else {
                false
            }
        }
        syn::Type::Reference(type_ref) => {
            // Check if it's &str
            if let syn::Type::Path(path) = &*type_ref.elem {
                path.path.is_ident("str")
            } else {
                false
            }
        }
        _ => false,
    }
}

pub(super) fn generate_method_impls(
    crud_meta: &CRUDResourceMeta,
    analysis: &EntityFieldAnalysis,
) -> (
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
) {
    let get_one_impl = if let Some(fn_path) = &crud_meta.fn_get_one {
        quote! {
            async fn get_one(db: &sea_orm::DatabaseConnection, id: uuid::Uuid) -> Result<Self, sea_orm::DbErr> {
                #fn_path(db, id).await
            }
        }
    } else {
        quote! {}
    };

    let get_all_impl = if let Some(fn_path) = &crud_meta.fn_get_all {
        quote! {
            async fn get_all(
                db: &sea_orm::DatabaseConnection,
                condition: &sea_orm::Condition,
                order_column: Self::ColumnType,
                order_direction: sea_orm::Order,
                offset: u64,
                limit: u64,
            ) -> Result<Vec<Self::ListModel>, sea_orm::DbErr> {
                #fn_path(db, condition, order_column, order_direction, offset, limit).await
            }
        }
    } else {
        // Check if we need to generate an optimized get_all with selective column fetching
        generate_optimized_get_all_impl(analysis)
    };

    let create_impl = if let Some(fn_path) = &crud_meta.fn_create {
        quote! {
            async fn create(db: &sea_orm::DatabaseConnection, create_data: Self::CreateModel) -> Result<Self, sea_orm::DbErr> {
                #fn_path(db, create_data).await
            }
        }
    } else {
        quote! {}
    };

    let update_impl = if let Some(fn_path) = &crud_meta.fn_update {
        quote! {
            async fn update(
                db: &sea_orm::DatabaseConnection,
                id: uuid::Uuid,
                update_data: Self::UpdateModel,
            ) -> Result<Self, sea_orm::DbErr> {
                #fn_path(db, id, update_data).await
            }
        }
    } else {
        quote! {}
    };

    let delete_impl = if let Some(fn_path) = &crud_meta.fn_delete {
        quote! {
            async fn delete(db: &sea_orm::DatabaseConnection, id: uuid::Uuid) -> Result<uuid::Uuid, sea_orm::DbErr> {
                #fn_path(db, id).await
            }
        }
    } else {
        quote! {}
    };

    let delete_many_impl = if let Some(fn_path) = &crud_meta.fn_delete_many {
        quote! {
            async fn delete_many(db: &sea_orm::DatabaseConnection, ids: Vec<uuid::Uuid>) -> Result<Vec<uuid::Uuid>, sea_orm::DbErr> {
                #fn_path(db, ids).await
            }
        }
    } else {
        quote! {}
    };

    (
        get_one_impl,
        get_all_impl,
        create_impl,
        update_impl,
        delete_impl,
        delete_many_impl,
    )
}

/// Generates optimized `get_all` implementation with selective column fetching when needed
fn generate_optimized_get_all_impl(analysis: &EntityFieldAnalysis) -> proc_macro2::TokenStream {
    // Check if there are fields excluded from ListModel (list_model = false)
    let has_excluded_list_fields = analysis
        .db_fields
        .iter()
        .any(|field| get_crudcrate_bool(field, "list_model") == Some(false))
        || analysis
            .non_db_fields
            .iter()
            .any(|field| get_crudcrate_bool(field, "list_model") == Some(false));

    if !has_excluded_list_fields {
        // If no fields are excluded, use default trait implementation
        return quote! {};
    }

    // Generate selective column list for ListModel (only db_fields included in list)
    let list_columns: Vec<proc_macro2::TokenStream> = analysis
        .db_fields
        .iter()
        .filter(|field| get_crudcrate_bool(field, "list_model").unwrap_or(true))
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let column_name =
                format_ident!("{}", ident_to_string(field_name).to_case(Case::Pascal));
            quote! { Self::ColumnType::#column_name }
        })
        .collect();

    // Generate FromQueryResult struct fields (only db fields included in ListModel)
    let query_result_fields: Vec<proc_macro2::TokenStream> = analysis
        .db_fields
        .iter()
        .filter(|field| get_crudcrate_bool(field, "list_model").unwrap_or(true))
        .map(|field| {
            let field_name = &field.ident;
            let field_type = &field.ty;
            quote! { pub #field_name: #field_type }
        })
        .collect();

    // Generate field assignments for creating the full struct from query result
    let full_struct_assignments: Vec<proc_macro2::TokenStream> = analysis
        .db_fields
        .iter()
        .map(|field| {
            let field_name = &field.ident;
            if get_crudcrate_bool(field, "list_model").unwrap_or(true) {
                // Field is included in ListModel - use actual data
                quote! { #field_name: query_data.#field_name }
            } else {
                // Field is excluded from ListModel - provide default/dummy value
                if let Some(default_expr) = get_crudcrate_expr(field, "default") {
                    quote! { #field_name: #default_expr }
                } else {
                    // For excluded fields, use Default::default() if no explicit default
                    quote! { #field_name: Default::default() }
                }
            }
        })
        .collect();

    // Generate assignments for non-db fields using their defaults
    let non_db_assignments: Vec<proc_macro2::TokenStream> = analysis
        .non_db_fields
        .iter()
        .map(|field| {
            let field_name = &field.ident;
            let default_expr = get_crudcrate_expr(field, "default")
                .unwrap_or_else(|| syn::parse_quote!(Default::default()));
            quote! { #field_name: #default_expr }
        })
        .collect();

    quote! {
        async fn get_all(
            db: &sea_orm::DatabaseConnection,
            condition: &sea_orm::Condition,
            order_column: Self::ColumnType,
            order_direction: sea_orm::Order,
            offset: u64,
            limit: u64,
        ) -> Result<Vec<Self::ListModel>, sea_orm::DbErr> {
            use sea_orm::{QuerySelect, QueryOrder, SelectColumns};

            #[derive(sea_orm::FromQueryResult)]
            struct QueryData {
                #(#query_result_fields),*
            }

            let query_results = Self::EntityType::find()
                .select_only()
                #(.select_column(#list_columns))*
                .filter(condition.clone())
                .order_by(order_column, order_direction)
                .offset(offset)
                .limit(limit)
                .into_model::<QueryData>()
                .all(db)
                .await?;

            Ok(query_results.into_iter().map(|query_data| {
                let full_model = Self {
                    #(#full_struct_assignments,)*
                    #(#non_db_assignments,)*
                };
                Self::ListModel::from(full_model)
            }).collect())
        }
    }
}

/// Generates the `CRUDResource` implementation based on the provided metadata and field analysis
pub(super) fn generate_crud_resource_impl(
    api_struct_name: &syn::Ident,
    crud_meta: &CRUDResourceMeta,
    active_model_path: &str,
    analysis: &EntityFieldAnalysis,
    table_name: &str,
) -> proc_macro2::TokenStream {
    let (
        create_model_name,
        update_model_name,
        list_model_name,
        entity_type,
        column_type,
        active_model_type,
    ) = generate_crud_type_aliases(api_struct_name, crud_meta, active_model_path);

    let id_column = generate_id_column(analysis.primary_key_field);
    let sortable_entries = generate_field_entries(&analysis.sortable_fields);
    let filterable_entries = generate_field_entries(&analysis.filterable_fields);
    let like_filterable_entries = generate_like_filterable_entries(&analysis.filterable_fields);
    let fulltext_entries = generate_fulltext_field_entries(&analysis.fulltext_fields);
    let enum_field_checker = generate_enum_field_checker(&analysis.db_fields);

    let name_singular = crud_meta.name_singular.as_deref().unwrap_or("resource");
    let _name_plural = crud_meta.name_plural.as_deref().unwrap_or("resources");
    let description = crud_meta.description.as_deref().unwrap_or("");
    let enum_case_sensitive = crud_meta.enum_case_sensitive;
    let fulltext_language = crud_meta.fulltext_language.as_deref().unwrap_or("english");

    let (get_one_impl, get_all_impl, create_impl, update_impl, delete_impl, delete_many_impl) =
        generate_method_impls(crud_meta, analysis);

    // Generate registration lazy static and auto-registration call for all models
    let (registration_static, auto_register_call) = (
        quote! {
            // Lazy static that ensures registration happens on first trait usage
            static __REGISTER_LAZY: std::sync::LazyLock<()> = std::sync::LazyLock::new(|| {
                crudcrate::register_analyser::<#api_struct_name>();
            });
        },
        quote! {
            std::sync::LazyLock::force(&__REGISTER_LAZY);
        }
    );

    // Generate resource name plural constant
    let resource_name_plural_impl = {
        let name_plural = crud_meta.name_plural.clone().unwrap_or_default();
        quote! {
            const RESOURCE_NAME_PLURAL: &'static str = #name_plural;
        }
    };

    quote! {
        #registration_static

        #[async_trait::async_trait]
        impl crudcrate::CRUDResource for #api_struct_name {
            type EntityType = #entity_type;
            type ColumnType = #column_type;
            type ActiveModelType = #active_model_type;
            type CreateModel = #create_model_name;
            type UpdateModel = #update_model_name;
            type ListModel = #list_model_name;

            const ID_COLUMN: Self::ColumnType = #id_column;
            const RESOURCE_NAME_SINGULAR: &'static str = #name_singular;
            #resource_name_plural_impl
            const TABLE_NAME: &'static str = #table_name;
            const RESOURCE_DESCRIPTION: &'static str = #description;
            const FULLTEXT_LANGUAGE: &'static str = #fulltext_language;

            fn sortable_columns() -> Vec<(&'static str, Self::ColumnType)> {
                #auto_register_call
                vec![#(#sortable_entries),*]
            }

            fn filterable_columns() -> Vec<(&'static str, Self::ColumnType)> {
                #auto_register_call
                vec![#(#filterable_entries),*]
            }

            fn enum_case_sensitive() -> bool {
                #enum_case_sensitive
            }


            fn is_enum_field(field_name: &str) -> bool {
                #enum_field_checker
            }

            fn like_filterable_columns() -> Vec<&'static str> {
                vec![#(#like_filterable_entries),*]
            }

            fn fulltext_searchable_columns() -> Vec<(&'static str, Self::ColumnType)> {
                #auto_register_call
                vec![#(#fulltext_entries),*]
            }

            #get_one_impl
            #get_all_impl
            #create_impl
            #update_impl
            #delete_impl
            #delete_many_impl
        }
    }
}

// ===================
// List Model Helpers
// ===================

/// Generate struct fields for List models, including only fields where `list_model` != false
pub(super) fn generate_list_struct_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| get_crudcrate_bool(field, "list_model").unwrap_or(true))
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;

            // Check if this field uses target models
            let final_ty = if field_has_crudcrate_flag(field, "use_target_models") {
                if let Some((_, _, list_model)) = resolve_target_models_with_list(ty) {
                    // Replace the type with the target's List model
                    if let syn::Type::Path(type_path) = ty {
                        if let Some(last_seg) = type_path.path.segments.last() {
                            if last_seg.ident == "Vec" {
                                // Vec<Treatment> -> Vec<TreatmentList>
                                quote! { Vec<#list_model> }
                            } else {
                                // Treatment -> TreatmentList
                                quote! { #list_model }
                            }
                        } else {
                            quote! { #ty }
                        }
                    } else {
                        quote! { #ty }
                    }
                } else {
                    quote! { #ty }
                }
            } else {
                quote! { #ty }
            };

            quote! {
                pub #ident: #final_ty
            }
        })
        .collect()
}

/// Generate From assignments for List models
pub(super) fn generate_list_from_assignments(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| get_crudcrate_bool(field, "list_model").unwrap_or(true))
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;

            // Check if this field uses target models
            if field_has_crudcrate_flag(field, "use_target_models") {
                if let Some((_, _, _)) = resolve_target_models_with_list(ty) {
                    // For Vec<T>, convert each item using From trait
                    if let syn::Type::Path(type_path) = ty
                        && let Some(last_seg) = type_path.path.segments.last()
                            && last_seg.ident == "Vec" {
                                return quote! {
                                    #ident: model.#ident.into_iter().map(Into::into).collect()
                                };
                            }
                    // For single item, use direct conversion
                    quote! {
                        #ident: model.#ident.into()
                    }
                } else {
                    quote! {
                        #ident: model.#ident
                    }
                }
            } else {
                quote! {
                    #ident: model.#ident
                }
            }
        })
        .collect()
}

/// Generate field assignments for List model From<Model> implementation (direct from DB Model)
pub(super) fn generate_list_from_model_assignments(
    analysis: &EntityFieldAnalysis,
) -> Vec<proc_macro2::TokenStream> {
    let mut assignments = Vec::new();

    // Handle DB fields that are included in ListModel
    for field in &analysis.db_fields {
        let field_name = &field.ident;

        if get_crudcrate_bool(field, "list_model").unwrap_or(true) {
            // Field is included in ListModel - use actual data from Model
            if field_has_crudcrate_flag(field, "use_target_models") {
                let field_type = &field.ty;
                if let Some((_, _, list_type)) = resolve_target_models_with_list(field_type) {
                    // For Vec<T>, convert each item using From trait to ListModel
                    if let syn::Type::Path(type_path) = field_type
                        && let Some(last_seg) = type_path.path.segments.last()
                            && last_seg.ident == "Vec" {
                                assignments.push(quote! {
                                    #field_name: model.#field_name.into_iter().map(|item| #list_type::from(item)).collect()
                                });
                                continue;
                            }
                    // For single item, use direct conversion to ListModel
                    assignments.push(quote! {
                        #field_name: #list_type::from(model.#field_name)
                    });
                    continue;
                }
            }

            // Handle DateTime conversion for Model -> ListModel
            let field_type = &field.ty;
            if field_type
                .to_token_stream()
                .to_string()
                .contains("DateTimeWithTimeZone")
            {
                if field_is_optional(field) {
                    assignments.push(quote! {
                        #field_name: model.#field_name.map(|dt| dt.with_timezone(&chrono::Utc))
                    });
                } else {
                    assignments.push(quote! {
                        #field_name: model.#field_name.with_timezone(&chrono::Utc)
                    });
                }
            } else {
                // Standard field - use directly from Model
                assignments.push(quote! {
                    #field_name: model.#field_name
                });
            }
        }
        // Fields with list_model = false are not included in ListModel struct, so skip them
    }

    // Handle non-DB fields - use defaults since they don't exist in Model
    for field in &analysis.non_db_fields {
        let field_name = &field.ident;

        if get_crudcrate_bool(field, "list_model").unwrap_or(true) {
            // Field is included in ListModel - use default since it's not in DB Model
            let default_expr = get_crudcrate_expr(field, "default")
                .unwrap_or_else(|| syn::parse_quote!(Default::default()));

            assignments.push(quote! {
                #field_name: #default_expr
            });
        }
        // Fields with list_model = false are not included in ListModel struct, so skip them
    }

    assignments
}


#[cfg(test)]
mod tests {
    use super::*;
    use syn::{Attribute, Field, parse_quote};

    #[test]
    fn test_field_is_optional_with_option() {
        let field: Field = parse_quote! {
            pub field: Option<String>
        };
        assert!(field_is_optional(&field));
    }

    #[test]
    fn test_field_is_optional_with_std_option() {
        let field: Field = parse_quote! {
            pub field: std::option::Option<String>
        };
        assert!(field_is_optional(&field));
    }

    #[test]
    fn test_field_is_optional_with_non_option() {
        let field: Field = parse_quote! {
            pub field: String
        };
        assert!(!field_is_optional(&field));
    }

    #[test]
    fn test_field_is_optional_with_complex_type() {
        let field: Field = parse_quote! {
            pub field: Vec<String>
        };
        assert!(!field_is_optional(&field));
    }

    #[test]
    fn test_get_crudcrate_bool_create_model_false() {
        let field: Field = parse_quote! {
            #[crudcrate(create_model = false)]
            pub field: String
        };
        assert_eq!(get_crudcrate_bool(&field, "create_model"), Some(false));
    }

    #[test]
    fn test_get_crudcrate_bool_create_model_true() {
        let field: Field = parse_quote! {
            #[crudcrate(create_model = true)]
            pub field: String
        };
        assert_eq!(get_crudcrate_bool(&field, "create_model"), Some(true));
    }

    #[test]
    fn test_get_crudcrate_bool_update_model_false() {
        let field: Field = parse_quote! {
            #[crudcrate(update_model = false)]
            pub field: String
        };
        assert_eq!(get_crudcrate_bool(&field, "update_model"), Some(false));
    }

    #[test]
    fn test_get_crudcrate_bool_multiple_attributes() {
        let field: Field = parse_quote! {
            #[crudcrate(create_model = false, update_model = true)]
            pub field: String
        };
        assert_eq!(get_crudcrate_bool(&field, "create_model"), Some(false));
        assert_eq!(get_crudcrate_bool(&field, "update_model"), Some(true));
    }

    #[test]
    fn test_get_crudcrate_bool_no_attribute() {
        let field: Field = parse_quote! {
            pub field: String
        };
        assert_eq!(get_crudcrate_bool(&field, "create_model"), None);
    }

    #[test]
    fn test_get_crudcrate_bool_wrong_attribute() {
        let field: Field = parse_quote! {
            #[serde(skip)]
            pub field: String
        };
        assert_eq!(get_crudcrate_bool(&field, "create_model"), None);
    }

    #[test]
    fn test_get_crudcrate_bool_non_db_attr() {
        let field: Field = parse_quote! {
            #[crudcrate(non_db_attr = true)]
            pub field: String
        };
        assert_eq!(get_crudcrate_bool(&field, "non_db_attr"), Some(true));
    }

    #[test]
    fn test_get_crudcrate_expr_on_create() {
        let field: Field = parse_quote! {
            #[crudcrate(on_create = Uuid::new_v4())]
            pub field: String
        };
        let expr = get_crudcrate_expr(&field, "on_create");
        assert!(expr.is_some());
        // Test that it contains the expected expression
        let expr_str = quote::quote!(#expr).to_string();
        assert!(expr_str.contains("Uuid :: new_v4"));
    }

    #[test]
    fn test_get_crudcrate_expr_on_update() {
        let field: Field = parse_quote! {
            #[crudcrate(on_update = Utc::now())]
            pub field: DateTime<Utc>
        };
        let expr = get_crudcrate_expr(&field, "on_update");
        assert!(expr.is_some());
        let expr_str = quote::quote!(#expr).to_string();
        assert!(expr_str.contains("Utc :: now"));
    }

    #[test]
    fn test_get_crudcrate_expr_default() {
        let field: Field = parse_quote! {
            #[crudcrate(default = "default_value".to_string())]
            pub field: String
        };
        let expr = get_crudcrate_expr(&field, "default");
        assert!(expr.is_some());
        let expr_str = quote::quote!(#expr).to_string();
        assert!(expr_str.contains("default_value"));
    }

    #[test]
    fn test_get_crudcrate_expr_no_match() {
        let field: Field = parse_quote! {
            #[crudcrate(create_model = false)]
            pub field: String
        };
        assert!(get_crudcrate_expr(&field, "on_create").is_none());
    }

    #[test]
    fn test_get_string_from_attr_simple() {
        let attr: Attribute = parse_quote! {
            #[active_model = "test::ActiveModel"]
        };
        let result = get_string_from_attr(&attr);
        assert_eq!(result, Some("test::ActiveModel".to_string()));
    }

    #[test]
    fn test_get_string_from_attr_no_match() {
        let attr: Attribute = parse_quote! {
            #[other_attr = "value"]
        };
        let result = get_string_from_attr(&attr);
        // This function extracts any string value regardless of attribute name
        assert_eq!(result, Some("value".to_string()));
    }

    #[test]
    fn test_get_string_from_attr_wrong_type() {
        let attr: Attribute = parse_quote! {
            #[active_model = true]
        };
        let result = get_string_from_attr(&attr);
        assert_eq!(result, None);
    }

    #[test]
    fn test_ident_to_string_normal_identifier() {
        let ident = syn::parse_quote! { regular_field };
        assert_eq!(ident_to_string(&ident), "regular_field");
    }

    #[test]
    fn test_ident_to_string_raw_identifier() {
        let ident = syn::parse_quote! { r#type };
        assert_eq!(ident_to_string(&ident), "type");
    }

    #[test]
    fn test_ident_to_string_raw_identifier_complex() {
        let ident = syn::parse_quote! { r#async };
        assert_eq!(ident_to_string(&ident), "async");
    }

    #[test]
    fn test_get_crudcrate_bool_with_complex_attributes() {
        // Test the exact attribute pattern from the failing test case
        let field: Field = parse_quote! {
            #[crudcrate(non_db_attr = true, default = vec![], create_model = false)]
            pub field: Vec<String>
        };
        assert_eq!(get_crudcrate_bool(&field, "create_model"), Some(false));
        assert_eq!(get_crudcrate_bool(&field, "non_db_attr"), Some(true));

        // Also test that default expr parsing works
        let default_expr = get_crudcrate_expr(&field, "default");
        assert!(default_expr.is_some());

        // Test the filtering logic that should exclude this field
        let should_include = get_crudcrate_bool(&field, "create_model").unwrap_or(true);
        assert!(!should_include, "Field with create_model=false should be excluded");
    }

    #[test]
    fn test_parse_crud_resource_meta() {
        // Test basic string attributes
        let attrs: Vec<syn::Attribute> = vec![
            parse_quote! { #[crudcrate(name_singular = "todo")] },
            parse_quote! { #[crudcrate(name_plural = "todos")] },
            parse_quote! { #[crudcrate(description = "Manages todo items")] },
        ];
        let meta = parse_crud_resource_meta(&attrs);
        assert_eq!(meta.name_singular, Some("todo".to_string()));
        assert_eq!(meta.name_plural, Some("todos".to_string()));
        assert_eq!(meta.description, Some("Manages todo items".to_string()));

        // Test complex attributes in single declaration
        let attrs: Vec<syn::Attribute> = vec![
            parse_quote! { #[crudcrate(entity_type = "Entity", column_type = "Column")] }
        ];
        let meta = parse_crud_resource_meta(&attrs);
        assert_eq!(meta.entity_type, Some("Entity".to_string()));
        assert_eq!(meta.column_type, Some("Column".to_string()));

        // Test empty/no attributes
        let attrs: Vec<syn::Attribute> = vec![];
        let meta = parse_crud_resource_meta(&attrs);
        assert_eq!(meta.name_singular, None);
        assert_eq!(meta.description, None);

        // Test non-crudcrate attributes (should be ignored)
        let attrs: Vec<syn::Attribute> = vec![
            parse_quote! { #[serde(skip)] },
            parse_quote! { #[derive(Debug)] },
        ];
        let meta = parse_crud_resource_meta(&attrs);
        assert_eq!(meta.name_singular, None);
    }

    #[test]
    fn test_extract_table_name() {
        // Test basic table name extraction
        let attrs: Vec<syn::Attribute> = vec![
            parse_quote! { #[sea_orm(table_name = "todos")] }
        ];
        assert_eq!(extract_table_name(&attrs), Some("todos".to_string()));

        // Test complex sea_orm attributes
        let attrs: Vec<syn::Attribute> = vec![
            parse_quote! { #[sea_orm(table_name = "user_profiles", primary_key = "id")] }
        ];
        assert_eq!(extract_table_name(&attrs), Some("user_profiles".to_string()));

        // Test no table_name attribute
        let attrs: Vec<syn::Attribute> = vec![
            parse_quote! { #[sea_orm(primary_key = "id")] }
        ];
        assert_eq!(extract_table_name(&attrs), None);

        // Test empty attributes
        let attrs: Vec<syn::Attribute> = vec![];
        assert_eq!(extract_table_name(&attrs), None);

        // Test non-sea_orm attributes (should be ignored)
        let attrs: Vec<syn::Attribute> = vec![
            parse_quote! { #[derive(Debug)] },
            parse_quote! { #[crudcrate(api_struct = "Todo")] },
        ];
        assert_eq!(extract_table_name(&attrs), None);
    }

    #[test]
    fn test_field_has_crudcrate_flag() {
        // Test field with the flag
        let field: Field = parse_quote! {
            #[crudcrate(primary_key)]
            pub id: Uuid
        };
        assert!(field_has_crudcrate_flag(&field, "primary_key"));

        // Test field with multiple flags
        let field: Field = parse_quote! {
            #[crudcrate(sortable, filterable)]
            pub name: String
        };
        assert!(field_has_crudcrate_flag(&field, "sortable"));
        assert!(field_has_crudcrate_flag(&field, "filterable"));
        assert!(!field_has_crudcrate_flag(&field, "primary_key"));

        // Test field with mixed attributes
        let field: Field = parse_quote! {
            #[crudcrate(filterable, create_model = false)]
            pub title: String
        };
        assert!(field_has_crudcrate_flag(&field, "filterable"));
        assert!(!field_has_crudcrate_flag(&field, "create_model")); // This is name-value, not a flag

        // Test field without crudcrate attributes
        let field: Field = parse_quote! {
            #[serde(skip)]
            pub field: String
        };
        assert!(!field_has_crudcrate_flag(&field, "primary_key"));

        // Test field with no attributes
        let field: Field = parse_quote! {
            pub field: String
        };
        assert!(!field_has_crudcrate_flag(&field, "sortable"));
    }
}
