use super::attribute_parser::{
    field_has_crudcrate_flag, get_crudcrate_bool, get_crudcrate_expr, get_join_config,
};
use super::field_analyzer::{
    extract_inner_type_for_update, field_is_optional, resolve_target_models,
    resolve_target_models_with_list,
};
// join_generators functionality consolidated into this file to avoid duplicate/stub implementations
use super::structs::{CRUDResourceMeta, EntityFieldAnalysis};
use convert_case::{Case, Casing};
use heck::ToPascalCase;
use quote::{ToTokens, format_ident, quote};
use syn;

/// Generates the field declarations for a create struct
pub(crate) fn generate_create_struct_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| {
            // Exclude fields from create model if create_model = false
            let include_in_create = get_crudcrate_bool(field, "create_model").unwrap_or(true);

            // Exclude join fields entirely from Create models - they're populated by recursive loading
            let is_join_field = get_join_config(field).is_some();

            // Debug output to understand what's happening
            #[cfg(feature = "debug")]
            if let Some(field_name) = &field.ident {
                let should_include = include_in_create && !is_join_field;
                eprintln!("DEBUG CREATE: field '{}' include_in_create={} is_join_field={} should_include={}",
                    field_name, include_in_create, is_join_field, should_include);
            }

            include_in_create && !is_join_field
        })
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            if get_crudcrate_bool(field, "non_db_attr").unwrap_or(false) {
                // Check if this field uses target models
                let has_use_target_models = field_has_crudcrate_flag(field, "use_target_models");
                let final_ty = if has_use_target_models {
                    if let Some((create_model, _)) = resolve_target_models(ty) {
                        // Replace the type with the target's Create model
                        if let syn::Type::Path(type_path) = ty {
                            if let Some(last_seg) = type_path.path.segments.last() {
                                if last_seg.ident == "Vec" {
                                    // Vec<Treatment> -> Vec<TreatmentCreate>
                                    quote! { Vec<#create_model> }
                                } else {
                                    // Treatment -> TreatmentCreate
                                    quote! { #create_model }
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
                if get_crudcrate_expr(field, "default").is_some() {
                    quote! {
                        #[serde(default)]
                        pub #ident: #final_ty
                    }
                } else {
                    quote! {
                        pub #ident: #final_ty
                    }
                }
            } else if get_crudcrate_expr(field, "on_create").is_some() {
                quote! {
                    #[serde(default)]
                    pub #ident: Option<#ty>
                }
            } else {
                quote! {
                    pub #ident: #ty
                }
            }
        })
        .collect()
}

/// Generates the conversion lines for a create model to active model conversion
pub(crate) fn generate_create_conversion_lines(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    let mut conv_lines = Vec::new();
    for field in fields {
        if get_crudcrate_bool(field, "non_db_attr").unwrap_or(false) {
            continue;
        }
        let ident = field.ident.as_ref().unwrap();
        let include = get_crudcrate_bool(field, "create_model").unwrap_or(true);
        let is_optional = field_is_optional(field);

        if include {
            if let Some(expr) = get_crudcrate_expr(field, "on_create") {
                if is_optional {
                    conv_lines.push(quote! {
                        #ident: sea_orm::ActiveValue::Set(match create.#ident {
                            Some(Some(inner)) => Some(inner.into()),
                            Some(None)         => None,
                            None               => Some((#expr).into()),
                        })
                    });
                } else {
                    conv_lines.push(quote! {
                        #ident: sea_orm::ActiveValue::Set(match create.#ident {
                            Some(val) => val.into(),
                            None      => (#expr).into(),
                        })
                    });
                }
            } else if is_optional {
                conv_lines.push(quote! {
                    #ident: sea_orm::ActiveValue::Set(create.#ident.map(|v| v.into()))
                });
            } else {
                conv_lines.push(quote! {
                    #ident: sea_orm::ActiveValue::Set(create.#ident.into())
                });
            }
        } else if let Some(expr) = get_crudcrate_expr(field, "on_create") {
            if is_optional {
                conv_lines.push(quote! {
                    #ident: sea_orm::ActiveValue::Set(Some((#expr).into()))
                });
            } else {
                conv_lines.push(quote! {
                    #ident: sea_orm::ActiveValue::Set((#expr).into())
                });
            }
        } else {
            // Field is excluded from Create model and has no on_create - set to NotSet
            // This allows the field to be set manually later in custom create functions
            conv_lines.push(quote! {
                #ident: sea_orm::ActiveValue::NotSet
            });
        }
    }
    conv_lines
}

/// Filters fields that should be included in update model
pub(crate) fn filter_update_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<&syn::Field> {
    fields
        .iter()
        .filter(|field| {
            // Exclude fields from update model if update_model = false
            let include_in_update = get_crudcrate_bool(field, "update_model").unwrap_or(true);

            // Exclude join fields entirely from Update models - they're populated by recursive loading
            let is_join_field = get_join_config(field).is_some();

            // Debug output to understand what's happening
            #[cfg(feature = "debug")]
            if let Some(field_name) = &field.ident {
                eprintln!("DEBUG UPDATE: field '{}' include_in_update={} is_join_field={}",
                    field_name, include_in_update, is_join_field);
            }

            include_in_update && !is_join_field
        })
        .collect()
}

/// Generates the field declarations for an update struct
pub(crate) fn generate_update_struct_fields(
    included_fields: &[&syn::Field],
) -> Vec<proc_macro2::TokenStream> {
    included_fields
        .iter()
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;

            if get_crudcrate_bool(field, "non_db_attr").unwrap_or(false) {
                // Check if this field uses target models
                let final_ty = if field_has_crudcrate_flag(field, "use_target_models") {
                    if let Some((_, update_model)) = resolve_target_models(ty) {
                        // Replace the type with the target's Update model
                        if let syn::Type::Path(type_path) = ty {
                            if let Some(last_seg) = type_path.path.segments.last() {
                                if last_seg.ident == "Vec" {
                                    // Vec<Treatment> -> Vec<TreatmentUpdate>
                                    quote! { Vec<#update_model> }
                                } else {
                                    // Treatment -> TreatmentUpdate
                                    quote! { #update_model }
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

                if get_crudcrate_expr(field, "default").is_some() {
                    quote! {
                        #[serde(default)]
                        pub #ident: #final_ty
                    }
                } else {
                    quote! {
                        pub #ident: #final_ty
                    }
                }
            } else {
                let inner_ty = extract_inner_type_for_update(ty);
                quote! {
                    #[serde(
                        default,
                        skip_serializing_if = "Option::is_none",
                        with = "crudcrate::serde_with::rust::double_option"
                    )]
                    pub #ident: Option<Option<#inner_ty>>
                }
            }
        })
        .collect()
}

pub(crate) fn generate_router_impl(api_struct_name: &syn::Ident) -> proc_macro2::TokenStream {
    let create_model_name = format_ident!("{}Create", api_struct_name);
    let update_model_name = format_ident!("{}Update", api_struct_name);
    let list_model_name = format_ident!("{}List", api_struct_name);
    let response_model_name = format_ident!("{}Response", api_struct_name);

    generate_axum_router(
        api_struct_name,
        &create_model_name,
        &update_model_name,
        &list_model_name,
        &response_model_name,
    )
}

fn generate_axum_router(
    api_struct_name: &syn::Ident,
    create_model_name: &syn::Ident,
    update_model_name: &syn::Ident,
    list_model_name: &syn::Ident,
    response_model_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    quote! {
        // Generate CRUD handlers using the crudcrate macro
        crudcrate::crud_handlers!(#api_struct_name, #update_model_name, #create_model_name, #list_model_name, #response_model_name);

        impl #api_struct_name {
            /// Generate router with all CRUD endpoints
            pub fn router(db: &sea_orm::DatabaseConnection) -> utoipa_axum::router::OpenApiRouter
            where
                Self: crudcrate::traits::CRUDResource,
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
}

pub(crate) fn generate_crud_resource_impl(
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
    let description = crud_meta.description.as_deref().unwrap_or("");
    let fulltext_language = crud_meta.fulltext_language.as_deref().unwrap_or("english");

    let (get_one_impl, get_all_impl, create_impl, update_impl, delete_impl, delete_many_impl) =
        generate_method_impls(crud_meta, analysis);

    // Generate registration lazy static and auto-registration call only for models without join fields
    // Models with join fields may have circular dependencies that prevent CRUDResource compilation
    let has_join_fields = !analysis.join_on_one_fields.is_empty() || !analysis.join_on_all_fields.is_empty();

    let (registration_static, auto_register_call) = if has_join_fields {
        // Skip registration for models with join fields to avoid circular dependency issues
        (
            quote! {},
            quote! {},
        )
    } else {
        (
            quote! {
                // Lazy static that ensures registration happens on first trait usage
                static __REGISTER_LAZY: std::sync::LazyLock<()> = std::sync::LazyLock::new(|| {
                    crudcrate::register_analyser::<#api_struct_name>();
                });
            },
            quote! {
                std::sync::LazyLock::force(&__REGISTER_LAZY);
            },
        )
    };

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

fn generate_crud_type_aliases(
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

fn generate_id_column(primary_key_field: Option<&syn::Field>) -> proc_macro2::TokenStream {
    if let Some(pk_field) = primary_key_field {
        let field_name = &pk_field.ident.as_ref().unwrap();
        let column_name = format_ident!("{}", ident_to_string(field_name).to_pascal_case());
        quote! { Self::ColumnType::#column_name }
    } else {
        quote! { Self::ColumnType::Id }
    }
}

fn generate_field_entries(fields: &[&syn::Field]) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let field_str = ident_to_string(field_name);
            let column_name = format_ident!("{}", field_str.to_pascal_case());
            quote! { (#field_str, Self::ColumnType::#column_name) }
        })
        .collect()
}

fn generate_like_filterable_entries(fields: &[&syn::Field]) -> Vec<proc_macro2::TokenStream> {
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

fn generate_fulltext_field_entries(fields: &[&syn::Field]) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let field_str = ident_to_string(field_name);
            let column_name = format_ident!("{}", field_str.to_pascal_case());
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
                let is_enum = get_crudcrate_bool(field, "enum_field").unwrap_or(false);

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

/// Generates join loading statements for direct queries (all join fields regardless of one/all flags)
fn generate_join_loading_for_direct_query(
    analysis: &EntityFieldAnalysis,
) -> Vec<proc_macro2::TokenStream> {
    let mut statements = Vec::new();

    // Process ALL join fields for direct queries (both one and all)
    let mut all_join_fields = analysis.join_on_one_fields.clone();
    all_join_fields.extend(analysis.join_on_all_fields.iter());

    // Remove duplicates (in case a field has both join(one) and join(all))
    all_join_fields.sort_by_key(|f| {
        f.ident
            .as_ref()
            .map(std::string::ToString::to_string)
            .unwrap_or_default()
    });
    all_join_fields.dedup_by_key(|f| {
        f.ident
            .as_ref()
            .map(std::string::ToString::to_string)
            .unwrap_or_default()
    });

    // Generate loading statements for all join fields
    for field in &all_join_fields {
        if let Some(field_name) = &field.ident {
            let join_config = get_join_config(field).unwrap_or_default();
            let _depth = join_config.depth.unwrap_or(3);

            // Generate code to load related entities for this field
            let relation_name = if let Some(custom_relation) = &join_config.relation {
                format_ident!("{}", custom_relation)
            } else {
                format_ident!("{}", field_name.to_string().to_pascal_case())
            };

            // Check if this is a Vec<T> field or a single T field by analyzing the type
            let is_vec_field = is_vec_type(&field.ty);

            if is_vec_field {
                // Generate code for Vec<T> fields (has_many relationships)
                let loading_stmt = quote! {
                    // Load related entities for #field_name field
                    if let Ok(related_models) = model.find_related(super::#relation_name::Entity).all(db).await {
                        // Convert related models to API structs (recursive loading happens via their own joins)
                        let mut related_with_joins = Vec::new();
                        for related_model in related_models {
                            let related_api_struct = related_model.into();
                            related_with_joins.push(related_api_struct);
                        }
                        result.#field_name = related_with_joins;
                    }
                };
                statements.push(loading_stmt);
            } else {
                // Generate code for single T or Option<T> fields (belongs_to/has_one relationships)
                let loading_stmt = quote! {
                    // Load related entity for #field_name field
                    if let Ok(Some(related_model)) = model.find_related(super::#relation_name::Entity).one(db).await {
                        result.#field_name = Some(related_model.into());
                    }
                };
                statements.push(loading_stmt);
            }
        }
    }

    statements
}

/// Helper function to determine if a type is Vec<T> or JoinField<Vec<T>>
pub fn is_vec_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            // Check if it's Vec<T>
            if segment.ident == "Vec" {
                return true;
            }
            // Check if it's JoinField<Vec<T>>
            if segment.ident == "JoinField" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        // Recursively check the inner type
                        return is_vec_type(inner_ty);
                    }
                }
            }
        }
    }
    false
}

/// Generate recursive join loading implementation for `get_one` method
fn generate_recursive_loading_implementation(
    analysis: &EntityFieldAnalysis,
) -> proc_macro2::TokenStream {
    // Check if there are any join fields for get_one
    // Fields with join(one) OR join(all) should appear in get_one() responses
    if analysis.join_on_one_fields.is_empty() && analysis.join_on_all_fields.is_empty() {
        return quote! {
            Ok(model.into())
        };
    }

    // Generate single-level loading for join(one) fields only
    let mut loading_statements = Vec::new();
    let mut field_assignments = Vec::new();

    // Process both join(one) and join(all) fields for get_one method
    // Fields with either join(one) or join(all) should appear in get_one() responses
    // IMPORTANT: Deduplicate fields - if a field has join(one, all), it appears in both lists
    let mut seen_fields = std::collections::HashSet::new();
    let all_join_fields_for_get_one: Vec<_> = analysis.join_on_one_fields
        .iter()
        .chain(analysis.join_on_all_fields.iter())
        .filter(|field| {
            if let Some(field_name) = &field.ident {
                seen_fields.insert(field_name.to_string())
            } else {
                true // Include fields without names (shouldn't happen)
            }
        })
        .collect();

    for field in all_join_fields_for_get_one {
        if let Some(field_name) = &field.ident {
            let join_config = get_join_config(field).unwrap_or_default();
            let is_vec_field = is_vec_type(&field.ty);

            // Extract entity and model paths from the field type
            let entity_path = get_entity_path_from_field_type(&field.ty);
            let model_path = get_model_path_from_field_type(&field.ty);

            // Check if this join should stop recursion at this level
            // depth=1 means "load this level but don't recurse into nested joins"
            // depth=2+ means "load this level AND recurse into nested joins"
            // None means "unlimited recursion"
            let stop_recursion = join_config.depth == Some(1);
            let should_recurse = join_config.depth.is_none() || join_config.depth.unwrap_or(1) > 1;

            if is_vec_field {
                // Extract the inner type from Vec<T> (or JoinField<Vec<T>>) and resolve it to the API struct type
                                // Check if field.ty is JoinField - if so, don't use global registry (preserve user's paths)
                                let is_join_field_type = if let syn::Type::Path(type_path) = &field.ty {
                                    type_path.path.segments.last().map(|seg| seg.ident == "JoinField").unwrap_or(false)
                                } else {
                                    false
                                };

                                // For depth-limited loading, we want to keep the Model type, not resolve to API struct
                                // Extract the raw inner type without resolving through the global registry
                                let inner_type = if is_join_field_type {
                                    // For JoinField<Vec<super::vehicle_part::Model>>, extract super::vehicle_part::Model
                                    syn::parse2::<syn::Type>(extract_vec_inner_type(&field.ty)).unwrap_or_else(|_| field.ty.clone())
                                } else if false && let Some(resolved_tokens) = super::two_pass_generator::resolve_join_type_globally(&field.ty) {
                                    // SKIP global registry resolution for depth-limited loading - we want Model types, not API structs
                                    // Parse the resolved tokens back into a Type
                                    if let Ok(mut resolved_type) = syn::parse2::<syn::Type>(resolved_tokens) {
                                        // First, strip JoinField<T> if present
                                        if let syn::Type::Path(type_path) = &resolved_type {
                                            if let Some(segment) = type_path.path.segments.last() {
                                                if segment.ident == "JoinField" {
                                                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                                                        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                                                            resolved_type = inner_ty.clone();
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // Then extract from Vec<T>
                                        if let syn::Type::Path(type_path) = &resolved_type {
                                            if let Some(segment) = type_path.path.segments.last()
                                                && segment.ident == "Vec"
                                                && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
                                                && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                                                inner_ty.clone()
                                            } else {
                                                // Not Vec<T>, use the resolved type directly
                                                resolved_type
                                            }
                                        } else {
                                            syn::parse2::<syn::Type>(extract_vec_inner_type(&field.ty)).unwrap_or_else(|_| field.ty.clone()) // Fallback
                                        }
                                    } else {
                                        syn::parse2::<syn::Type>(extract_vec_inner_type(&field.ty)).unwrap_or_else(|_| field.ty.clone()) // Fallback
                                    }
                                } else {
                                    syn::parse2::<syn::Type>(extract_vec_inner_type(&field.ty)).unwrap_or_else(|_| field.ty.clone())
                                };

                                // For Vec<T> fields (has_many relationships) - depth-aware loading
                                if stop_recursion {
                                    // Depth-limited loading (depth=1) - Load data but don't recurse
                                    // This prevents infinite recursion by converting Models directly to API structs
                                    // without calling their get_one() methods (which would load nested joins)
                                    let api_struct_type = extract_api_struct_type_for_recursive_call(&field.ty);
                                    let loaded_var_name = quote::format_ident!("loaded_{}", field_name);
                                    loading_statements.push(quote! {
                                        let related_models = model.find_related(#entity_path).all(db).await.unwrap_or_default();
                                        let #loaded_var_name: Vec<#api_struct_type> = related_models.into_iter()
                                            .map(|related_model| Into::<#api_struct_type>::into(related_model))
                                            .collect();
                                    });
                                    // Assign the loaded Vec directly (no JoinField wrapper)
                                    field_assignments.push(quote! { result.#field_name = #loaded_var_name; });
                                } else {
                                    // Unlimited recursion - use the original recursive approach
                                    let api_struct_type = extract_api_struct_type_for_recursive_call(&field.ty);
                                    // Generate loading for unlimited recursion
                                    loading_statements.push(quote! {
                                        let related_models = model.find_related(#entity_path).all(db).await.unwrap_or_default();
                                        let mut #field_name = Vec::new();
                                        for related_model in related_models {
                                            // Use recursive get_one to respect join loading for nested entities
                                            match #api_struct_type::get_one(db, related_model.id).await {
                                                Ok(loaded_entity) => #field_name.push(loaded_entity),
                                                Err(_) => {
                                                    // Fallback: convert Model to API struct using explicit Into::into
                                                    // The From<Model> impl is in the target module and should be found via imports
                                                    #field_name.push(Into::<#api_struct_type>::into(related_model))
                                                },
                                            }
                                        }
                                    });
                                    // Assign the loaded Vec directly (no JoinField wrapper)
                                    field_assignments.push(quote! { result.#field_name = #field_name; });
                                }
            } else {
                // Extract the inner type from Option<T> or T
                let inner_type = extract_option_or_direct_inner_type(&field.ty);

                // For single T or Option<T> fields (belongs_to/has_one relationships) - depth-aware loading
                if stop_recursion {
                    // Depth-limited loading (depth=1) - Load data but don't recurse
                    // This prevents infinite recursion by converting Models directly to API structs
                    // without calling their get_one() methods (which would load nested joins)
                    let loaded_var_name = quote::format_ident!("loaded_{}", field_name);
                    loading_statements.push(quote! {
                        let #loaded_var_name = if let Ok(Some(related_model)) = model.find_related(#entity_path).one(db).await {
                            Some(Into::<#inner_type>::into(related_model))
                        } else {
                            None
                        };
                    });
                    // Assign the loaded Option directly (no JoinField wrapper)
                    field_assignments.push(quote! {
                        result.#field_name = #loaded_var_name;
                    });
                } else {
                    // Unlimited recursion - use the original recursive approach
                    loading_statements.push(quote! {
                        let #field_name = if let Ok(Some(related_model)) = model.find_related(#entity_path).one(db).await {
                            // Use recursive get_one to respect join loading for nested entities
                            match #inner_type::get_one(db, related_model.id).await {
                                Ok(loaded_entity) => Some(loaded_entity),
                                Err(_) => {
                                    // Fallback: convert Model to API struct using explicit Into::into
                                    Some(Into::<#inner_type>::into(related_model))
                                },
                            }
                        } else {
                            None
                        };
                    });
                    // Assign the loaded Option directly (no JoinField wrapper)
                    field_assignments.push(quote! {
                        result.#field_name = #field_name;
                    });
                }
            }
        }
    }

    quote! {
        // Load all join fields with recursive loading
        #(#loading_statements)*

        // Create result struct with loaded join data
        let mut result: Self = model.into();
        #(#field_assignments)*

        Ok(result)
    }
}

/// Generate `get_one` method implementation
fn generate_get_one_impl(
    crud_meta: &CRUDResourceMeta,
    analysis: &EntityFieldAnalysis,
) -> proc_macro2::TokenStream {
    if let Some(fn_path) = &crud_meta.fn_get_one {
        quote! {
            async fn get_one(db: &sea_orm::DatabaseConnection, id: uuid::Uuid) -> Result<Self, sea_orm::DbErr> {
                #fn_path(db, id).await
            }
        }
    } else {
        // Generate default implementation for get_one with recursive join support
        let has_joins =
            !analysis.join_on_one_fields.is_empty() || !analysis.join_on_all_fields.is_empty();

        if has_joins {
            // Generate the recursive loading statements for ALL join fields (direct query loads all joins)
            let _join_loading_statements = generate_join_loading_for_direct_query(analysis);

            // Generate the actual recursive loading implementation
            let recursive_loading_code = generate_recursive_loading_implementation(analysis);

            quote! {
                async fn get_one(db: &sea_orm::DatabaseConnection, id: uuid::Uuid) -> Result<Self, sea_orm::DbErr> {
                    use sea_orm::{EntityTrait, ModelTrait, Related};

                    // Load the main entity first
                    let main_model = Self::EntityType::find_by_id(id)
                        .one(db)
                        .await?;

                    match main_model {
                        Some(model) => {
                            #recursive_loading_code
                        }
                        None => Err(sea_orm::DbErr::RecordNotFound("Record not found".to_string())),
                    }
                }
            }
        } else {
            quote! {
                async fn get_one(db: &sea_orm::DatabaseConnection, id: uuid::Uuid) -> Result<Self, sea_orm::DbErr> {
                    let model = Self::EntityType::find_by_id(id)
                        .one(db)
                        .await?;
                    match model {
                        Some(model) => Ok(model.into()),
                        None => Err(sea_orm::DbErr::RecordNotFound("Record not found".to_string())),
                    }
                }
            }
        }
    }
}

/// Generate `get_all` method implementation
fn generate_get_all_impl(
    crud_meta: &CRUDResourceMeta,
    analysis: &EntityFieldAnalysis,
) -> proc_macro2::TokenStream {
    if let Some(fn_path) = &crud_meta.fn_get_all {
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
        // Check if there are join(all) fields that need loading
        let has_join_all_fields = !analysis.join_on_all_fields.is_empty();

        if has_join_all_fields {
            // Generate get_all with join loading
            let join_loading = generate_get_all_join_loading(analysis);

            quote! {
                async fn get_all(
                    db: &sea_orm::DatabaseConnection,
                    condition: &sea_orm::Condition,
                    order_column: Self::ColumnType,
                    order_direction: sea_orm::Order,
                    offset: u64,
                    limit: u64,
                ) -> Result<Vec<Self::ListModel>, sea_orm::DbErr> {
                    use sea_orm::{QueryOrder, QuerySelect, EntityTrait, ModelTrait};

                    let models = Self::EntityType::find()
                        .filter(condition.clone())
                        .order_by(order_column, order_direction)
                        .offset(offset)
                        .limit(limit)
                        .all(db)
                        .await?;

                    let mut results = Vec::new();
                    for model in models {
                        #join_loading
                        results.push(Self::ListModel::from(result));
                    }
                    Ok(results)
                }
            }
        } else {
            // Standard get_all without joins
            quote! {
                async fn get_all(
                    db: &sea_orm::DatabaseConnection,
                    condition: &sea_orm::Condition,
                    order_column: Self::ColumnType,
                    order_direction: sea_orm::Order,
                    offset: u64,
                    limit: u64,
                ) -> Result<Vec<Self::ListModel>, sea_orm::DbErr> {
                    use sea_orm::{QueryOrder, QuerySelect, EntityTrait};

                    let models = Self::EntityType::find()
                        .filter(condition.clone())
                        .order_by(order_column, order_direction)
                        .offset(offset)
                        .limit(limit)
                        .all(db)
                        .await?;
                    Ok(models.into_iter().map(|model| Self::ListModel::from(Self::from(model))).collect())
                }
            }
        }
    }
}

/// Generate join loading logic for `get_all` method
fn generate_get_all_join_loading(analysis: &EntityFieldAnalysis) -> proc_macro2::TokenStream {
    let mut loading_statements = Vec::new();
    let mut field_assignments = Vec::new();

    for field in &analysis.join_on_all_fields {
        if let Some(field_name) = &field.ident {
            let join_config = get_join_config(field).unwrap_or_default();
            let is_vec_field = is_vec_type(&field.ty);
            // Check if this join should stop recursion at this level
            let stop_recursion = join_config.depth == Some(1);

            // Extract entity and model paths from the field type
            let entity_path = get_entity_path_from_field_type(&field.ty);
            let model_path = get_model_path_from_field_type(&field.ty);

            if is_vec_field {
                // Extract the target type from Vec<TargetType> and resolve it to API struct
                // Check if field.ty is JoinField - if so, don't use global registry (preserve user's paths)
                let is_join_field_type = if let syn::Type::Path(type_path) = &field.ty {
                    type_path.path.segments.last().map(|seg| seg.ident == "JoinField").unwrap_or(false)
                } else {
                    false
                };

                let target_type = if is_join_field_type {
                    // For JoinField, preserve the original type and just extract inner
                    syn::parse2::<syn::Type>(extract_vec_inner_type(&field.ty)).unwrap_or_else(|_| field.ty.clone())
                } else if let Some(resolved_tokens) = super::two_pass_generator::resolve_join_type_globally(&field.ty) {
                    // Parse the resolved tokens back into a Type
                    if let Ok(resolved_type) = syn::parse2::<syn::Type>(resolved_tokens) {
                        // If it's Vec<T>, we need to extract the inner type from the resolved type
                        if let syn::Type::Path(type_path) = &resolved_type {
                            if let Some(segment) = type_path.path.segments.last()
                                && segment.ident == "Vec"
                                && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
                                && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                                inner_ty.clone()
                            } else {
                                // Not Vec<T>, use the resolved type directly
                                resolved_type
                            }
                        } else {
                            syn::parse2::<syn::Type>(extract_vec_inner_type(&field.ty)).unwrap_or_else(|_| field.ty.clone()) // Fallback
                        }
                    } else {
                        syn::parse2::<syn::Type>(extract_vec_inner_type(&field.ty)).unwrap_or_else(|_| field.ty.clone()) // Fallback
                    }
                } else {
                    syn::parse2::<syn::Type>(extract_vec_inner_type(&field.ty)).unwrap_or_else(|_| field.ty.clone())
                };

                // For Vec<T> relationships, load all related models - depth-aware
                let api_struct_type = extract_api_struct_type_for_recursive_call(&field.ty);

                if stop_recursion {
                    // Depth-limited loading (depth=1) - Load data but don't recurse
                    let loaded_var_name = quote::format_ident!("loaded_{}", field_name);
                    loading_statements.push(quote! {
                        let related_models = model.find_related(#entity_path).all(db).await.unwrap_or_default();
                        let #loaded_var_name: Vec<#api_struct_type> = related_models.into_iter()
                            .map(|related_model| Into::<#api_struct_type>::into(related_model))
                            .collect();
                    });
                    field_assignments.push(quote! {
                        result.#field_name = #loaded_var_name;
                    });
                } else {
                    // Unlimited recursion - use recursive get_one calls
                    loading_statements.push(quote! {
                        let related_models = model.find_related(#entity_path).all(db).await.unwrap_or_default();
                        let mut #field_name = Vec::new();
                        for related_model in related_models {
                            // Try recursive loading by calling the target type's get_one method
                            match #api_struct_type::get_one(db, related_model.id).await {
                                Ok(loaded_entity) => #field_name.push(loaded_entity),
                                Err(_) => {
                                    // Fallback: convert Model to API struct using explicit Into::into with full path
                                    #field_name.push(Into::<#api_struct_type>::into(related_model))
                                },
                            }
                        }
                    });
                    // Assign the loaded Vec directly for get_all (no JoinField wrapper)
                    field_assignments.push(quote! {
                        result.#field_name = #field_name;
                    });
                }
            } else {
                // For single relationships (Option<T> or T), load one related model - depth-aware
                let target_type = extract_option_or_direct_inner_type(&field.ty);

                if stop_recursion {
                    // Depth-limited loading (depth=1) - Load data but don't recurse
                    let loaded_var_name = quote::format_ident!("loaded_{}", field_name);
                    loading_statements.push(quote! {
                        let #loaded_var_name = match model.find_related(#entity_path).one(db).await.unwrap_or_default() {
                            Some(related_model) => Some(Into::<#target_type>::into(related_model)),
                            None => None,
                        };
                    });
                    field_assignments.push(quote! {
                        result.#field_name = #loaded_var_name.unwrap_or_default();
                    });
                } else {
                    // Unlimited recursion - use recursive get_one calls
                    loading_statements.push(quote! {
                        let #field_name = match model.find_related(#entity_path).one(db).await.unwrap_or_default() {
                            Some(related_model) => {
                                // Try recursive loading by calling the target type's get_one method
                                match #target_type::get_one(db, related_model.id).await {
                                    Ok(loaded_entity) => Some(loaded_entity),
                                    Err(_) => {
                                        // Fallback: convert Model to API struct using explicit Into::into
                                        Some(Into::<#target_type>::into(related_model))
                                    },
                                }
                            }
                            None => None,
                        };
                    });
                    // Assign the loaded Option/value directly for get_all (no JoinField wrapper)
                    field_assignments.push(quote! {
                        result.#field_name = #field_name.unwrap_or_default();
                    });
                }
            }
        }
    }

    // Convert base model to API struct and assign join fields
    quote! {
        #(#loading_statements)*
        let mut result: Self = model.into();
        #(#field_assignments)*
    }
}

/// Generate create method implementation
fn generate_create_impl(
    crud_meta: &CRUDResourceMeta,
    _analysis: &EntityFieldAnalysis,
) -> proc_macro2::TokenStream {
    if let Some(fn_path) = &crud_meta.fn_create {
        quote! {
            async fn create(db: &sea_orm::DatabaseConnection, data: Self::CreateModel) -> Result<Self, sea_orm::DbErr> {
                #fn_path(db, data).await
            }
        }
    } else {
        quote! {
            // Default create implementation
            async fn create(db: &sea_orm::DatabaseConnection, data: Self::CreateModel) -> Result<Self, sea_orm::DbErr> {
                let active_model: Self::ActiveModelType = data.into();
                let result = Self::EntityType::insert(active_model).exec(db).await?;
                Self::get_one(db, result.last_insert_id.into()).await
            }
        }
    }
}

/// Generate update method implementation
fn generate_update_impl(
    crud_meta: &CRUDResourceMeta,
    _analysis: &EntityFieldAnalysis,
) -> proc_macro2::TokenStream {
    if let Some(fn_path) = &crud_meta.fn_update {
        quote! {
            async fn update(db: &sea_orm::DatabaseConnection, id: uuid::Uuid, data: Self::UpdateModel) -> Result<Self, sea_orm::DbErr> {
                #fn_path(db, id, data).await
            }
        }
    } else {
        quote! {
            // Default update implementation
            async fn update(db: &sea_orm::DatabaseConnection, id: uuid::Uuid, data: Self::UpdateModel) -> Result<Self, sea_orm::DbErr> {
                use sea_orm::{EntityTrait, IntoActiveModel, ActiveModelTrait};
                use crudcrate::traits::MergeIntoActiveModel;

                let model = Self::EntityType::find_by_id(id)
                    .one(db)
                    .await?
                    .ok_or(sea_orm::DbErr::RecordNotFound(format!(
                        "{} not found",
                        Self::RESOURCE_NAME_SINGULAR
                    )))?;
                let existing: Self::ActiveModelType = model.into_active_model();
                let updated_model = data.merge_into_activemodel(existing)?;
                let updated = updated_model.update(db).await?;
                Ok(Self::from(updated))
            }
        }
    }
}

/// Generate delete method implementation
fn generate_delete_impl(crud_meta: &CRUDResourceMeta) -> proc_macro2::TokenStream {
    if let Some(fn_path) = &crud_meta.fn_delete {
        quote! {
            async fn delete(db: &sea_orm::DatabaseConnection, id: uuid::Uuid) -> Result<uuid::Uuid, sea_orm::DbErr> {
                #fn_path(db, id).await
            }
        }
    } else {
        quote! {
            // Default delete implementation
            async fn delete(db: &sea_orm::DatabaseConnection, id: uuid::Uuid) -> Result<uuid::Uuid, sea_orm::DbErr> {
                use sea_orm::EntityTrait;

                let res = Self::EntityType::delete_by_id(id).exec(db).await?;
                match res.rows_affected {
                    0 => Err(sea_orm::DbErr::RecordNotFound(format!(
                        "{} not found",
                        Self::RESOURCE_NAME_SINGULAR
                    ))),
                    _ => Ok(id),
                }
            }
        }
    }
}

/// Generate `delete_many` method implementation
fn generate_delete_many_impl(crud_meta: &CRUDResourceMeta) -> proc_macro2::TokenStream {
    if let Some(fn_path) = &crud_meta.fn_delete_many {
        quote! {
            async fn delete_many(db: &sea_orm::DatabaseConnection, ids: Vec<uuid::Uuid>) -> Result<Vec<uuid::Uuid>, sea_orm::DbErr> {
                #fn_path(db, ids).await
            }
        }
    } else {
        quote! {
            // Default delete_many implementation
            async fn delete_many(db: &sea_orm::DatabaseConnection, ids: Vec<uuid::Uuid>) -> Result<Vec<uuid::Uuid>, sea_orm::DbErr> {
                use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                Self::EntityType::delete_many()
                    .filter(Self::ID_COLUMN.is_in(ids.clone()))
                    .exec(db)
                    .await?;
                Ok(ids)
            }
        }
    }
}

fn generate_method_impls(
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
    let get_one_impl = generate_get_one_impl(crud_meta, analysis);
    let get_all_impl = generate_get_all_impl(crud_meta, analysis);
    let create_impl = generate_create_impl(crud_meta, analysis);
    let update_impl = generate_update_impl(crud_meta, analysis);
    let delete_impl = generate_delete_impl(crud_meta);
    let delete_many_impl = generate_delete_many_impl(crud_meta);

    (
        get_one_impl,
        get_all_impl,
        create_impl,
        update_impl,
        delete_impl,
        delete_many_impl,
    )
}

fn extract_vec_inner_type(ty: &syn::Type) -> proc_macro2::TokenStream {
    // First, strip JoinField<T> wrapper if present
    let unwrapped_type = if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "JoinField" {
                // Extract T from JoinField<T>
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        inner_ty
                    } else {
                        ty
                    }
                } else {
                    ty
                }
            } else {
                ty
            }
        } else {
            ty
        }
    } else {
        ty
    };

    // Then extract from Vec<T>
    if let syn::Type::Path(type_path) = unwrapped_type {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Vec" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return quote! { #inner_ty };
                    }
                }
            }
        }
    }
    quote! { () } // Fallback
}

/// Extract the inner type from Option<T> or return T directly
fn extract_option_or_direct_inner_type(ty: &syn::Type) -> proc_macro2::TokenStream {
    // First, strip JoinField<T> wrapper if present
    let unwrapped_type = if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "JoinField" {
                // Extract T from JoinField<T>
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        inner_ty
                    } else {
                        ty
                    }
                } else {
                    ty
                }
            } else {
                ty
            }
        } else {
            ty
        }
    } else {
        ty
    };

    // Then check if it's Option<T>
    if let syn::Type::Path(type_path) = unwrapped_type {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return quote! { #inner_ty };
                    }
                }
            }
        }
    }
    // If it's not Option<T>, return the unwrapped type
    quote! { #unwrapped_type }
}

/// Extract the API struct type for recursive get_one() calls from field types
/// This ALWAYS extracts the inner type and resolves Model -> API struct
/// JoinField<Vec<super::vehicle::Model>> -> Vehicle
/// Vec<super::customer::Model> -> Customer
fn extract_api_struct_type_for_recursive_call(field_type: &syn::Type) -> proc_macro2::TokenStream {
    // Helper function to extract inner type from any Type (JoinField<T>, Vec<T>, Option<T>, or direct T)
    // and resolve Model -> API struct name
    fn extract_inner_type_from_type(ty: &syn::Type) -> proc_macro2::TokenStream {
        if let syn::Type::Path(type_path) = ty {
            if let Some(segment) = type_path.path.segments.last() {
                if segment.ident == "JoinField" || segment.ident == "Vec" || segment.ident == "Option" {
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                            // Recursively extract from the inner type (handles JoinField<Vec<T>>, Vec<Option<T>> etc.)
                            return extract_inner_type_from_type(inner_ty);
                        }
                    }
                }
            }
        }
        // Base case: resolve Model path to API struct with FULL PATH
        // super::vehicle_part::Model -> super::vehicle_part::VehiclePart (not just VehiclePart)
        // This is critical for macro expansion order - we need the full path so Rust can resolve it later
        if let syn::Type::Path(type_path) = ty {
            // Extract the module path (everything except the last segment "Model")
            let path_segments: Vec<_> = type_path.path.segments.iter()
                .take(type_path.path.segments.len().saturating_sub(1)) // All except last
                .collect();

            if !path_segments.is_empty() && type_path.path.segments.last().map(|s| s.ident == "Model").unwrap_or(false) {
                // We have a path like super::vehicle_part::Model
                // Extract the base type and get the API struct name
                if let Some(base_type_str) = super::two_pass_generator::extract_base_type_string(ty) {
                    if let Some(api_name) = super::two_pass_generator::find_api_struct_name(&base_type_str) {
                        let api_ident = quote::format_ident!("{}", api_name);
                        // Reconstruct the full path: super::vehicle_part::VehiclePart
                        #[cfg(feature = "debug")]
                        eprintln!("DEBUG extract_inner_type: Resolved Model to API struct with path: {:?} -> {}::{}", quote!{#ty}, quote!{#(#path_segments)::*}, api_name);
                        return quote! { #(#path_segments)::*::#api_ident };
                    }
                }
            }
        }

        // Fallback: return the type as-is
        quote! { #ty }
    }

    // Check if field_type is already JoinField<T> - if so, DON'T use global registry
    // The user has already provided the correct type with proper paths
    let is_join_field = if let syn::Type::Path(type_path) = field_type {
        type_path.path.segments.last().map(|seg| seg.ident == "JoinField").unwrap_or(false)
    } else {
        false
    };

    // Only use global registry for non-JoinField types (handles type aliases like VehicleJoin)
    let resolved_type = if is_join_field {
        // For JoinField<T>, preserve the original type (user already provided correct paths)
        #[cfg(feature = "debug")]
        eprintln!("DEBUG extract_api_struct_type: Field is JoinField, preserving original type");
        field_type.clone()
    } else if let Some(resolved_tokens) = super::two_pass_generator::resolve_join_type_globally(field_type) {
        if let Ok(parsed_type) = syn::parse2::<syn::Type>(resolved_tokens) {
            #[cfg(feature = "debug")]
            eprintln!("DEBUG extract_api_struct_type (RESOLVED): {:?} -> {:?}", quote!{#field_type}, quote!{#parsed_type});

            // Check if the resolved type is just a bare ident without generics (e.g., just "JoinField")
            // This indicates a bug in the global resolution - ignore it and use the original type
            if let syn::Type::Path(ref type_path) = parsed_type {
                if let Some(segment) = type_path.path.segments.last() {
                    if (segment.ident == "JoinField" || segment.ident == "Vec" || segment.ident == "Option")
                        && matches!(segment.arguments, syn::PathArguments::None) {
                        #[cfg(feature = "debug")]
                        eprintln!("DEBUG extract_api_struct_type: Ignoring bare {} from resolution, using original type", segment.ident);
                        field_type.clone()
                    } else {
                        parsed_type
                    }
                } else {
                    parsed_type
                }
            } else {
                parsed_type
            }
        } else {
            field_type.clone()
        }
    } else {
        field_type.clone()
    };

    // Now extract the inner type from the resolved type
    if let syn::Type::Path(type_path) = &resolved_type {
        if let Some(segment) = type_path.path.segments.last() {
            let type_name = segment.ident.to_string();

            // For JoinField<T>, Vec<T>, or Option<T> fields, ALWAYS extract the inner type for recursive calls
            if segment.ident == "JoinField" || segment.ident == "Vec" || segment.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        let inner_type = extract_inner_type_from_type(inner_ty);
                        #[cfg(feature = "debug")]
                        eprintln!("DEBUG extract_api_struct_type (INNER from {}): {:?} -> {:?}", segment.ident, quote!{#resolved_type}, quote!{#inner_type});
                        return inner_type;
                    }
                }
            }

            // Handle type aliases that end with "Join" (VehicleJoin -> Vehicle)
            // This handles cases where the type alias wasn't resolved to Vec<T> properly
            if type_name.ends_with("Join") {
                let base_name = type_name.strip_suffix("Join").unwrap_or(&type_name);
                let api_struct_name = base_name; // Most API structs have the same name as the entity
                #[cfg(feature = "debug")]
                eprintln!("DEBUG extract_api_struct_type (JOIN): {:?} -> {:?}", quote!{#resolved_type}, quote!{#api_struct_name});
                return quote! { #api_struct_name };
            }

            // For direct types, use them as-is
            #[cfg(feature = "debug")]
            eprintln!("DEBUG extract_api_struct_type (DIRECT): {:?} -> {:?}", quote!{#resolved_type}, quote!{#resolved_type});
            return quote! { #resolved_type };
        }
    }

    // Fallback: extract inner type from the original field type directly
    let inner_type = extract_inner_type_from_type(field_type);
    #[cfg(feature = "debug")]
    eprintln!("DEBUG extract_api_struct_type (FINAL): {:?} -> {:?}", quote!{#field_type}, quote!{#inner_type});
    inner_type
}

/// Generates optimized `get_all` implementation with selective column fetching when needed
#[allow(dead_code)]
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
                format_ident!("{}", ident_to_string(field_name).to_pascal_case());
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

pub(crate) fn generate_response_struct_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| {
            let include_in_response = get_crudcrate_bool(field, "one_model").unwrap_or(true);
            include_in_response
        })
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;

            // Similar logic to List model for join fields
            let final_ty = if get_join_config(field).is_some() {
                super::resolve_join_field_type_preserving_container(ty)
            } else {
                quote! { #ty }
            };

            quote! {
                pub #ident: #final_ty
            }
        })
        .collect()
}

pub(crate) fn generate_list_struct_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| {
            let include_in_list = get_crudcrate_bool(field, "list_model").unwrap_or(true);
            // Only exclude join(one) fields from List models - keep join(all) fields since they're meant for list responses
            let is_join_one_only = if let Some(join_config) = get_join_config(field) {
                !join_config.on_all  // Exclude if NOT loading in get_all (on_all = false)
            } else {
                false
            };
            include_in_list && !is_join_one_only
        })
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
            } else if get_join_config(field).is_some() {
                // For join fields, resolve the type from JoinField<Vec<Model>> to Vec<APIStruct>
                // This ensures List models have proper API struct types, not DB Model types
                super::resolve_join_field_type_preserving_container(ty)
            } else {
                quote! { #ty }
            };

            quote! {
                pub #ident: #final_ty
            }
        })
        .collect()
}

pub(crate) fn generate_response_from_assignments(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| {
            let include_in_response = get_crudcrate_bool(field, "one_model").unwrap_or(true);
            include_in_response
        })
        .map(|field| {
            let ident = &field.ident;
            quote! {
                #ident: model.#ident
            }
        })
        .collect()
}

pub(crate) fn generate_list_from_assignments(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| {
            let include_in_list = get_crudcrate_bool(field, "list_model").unwrap_or(true);
            // Only exclude join(one) fields from List models - keep join(all) fields since they're meant for list responses
            let is_join_one_only = if let Some(join_config) = get_join_config(field) {
                !join_config.on_all  // Exclude if NOT loading in get_all (on_all = false)
            } else {
                false
            };
            include_in_list && !is_join_one_only
        })
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;

            // Check if this field uses target models
            if field_has_crudcrate_flag(field, "use_target_models") {
                if let Some((_, _, _)) = resolve_target_models_with_list(ty) {
                    // For Vec<T>, convert each item using From trait
                    if let syn::Type::Path(type_path) = ty
                        && let Some(last_seg) = type_path.path.segments.last()
                        && last_seg.ident == "Vec"
                    {
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

pub(crate) fn generate_list_from_model_assignments(
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
                        && last_seg.ident == "Vec"
                    {
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

        let include_in_list = get_crudcrate_bool(field, "list_model").unwrap_or(true);
        // Only exclude join(one) fields from List models - keep join(all) fields since they're meant for list responses
        let is_join_one_only = if let Some(join_config) = get_join_config(field) {
            !join_config.on_all  // Exclude if NOT loading in get_all (on_all = false)
        } else {
            false
        };

        if include_in_list && !is_join_one_only {
            // Check if this is a join(all) field
            let is_join_all = get_join_config(field).map(|c| c.on_all).unwrap_or(false);

            if is_join_all {
                // Join(all) fields: Initialize with empty vec in From<Model> - they'll be populated by get_all() loading logic
                // The ListModel struct includes them with Vec<APIStruct> type, so we initialize with vec![]
                // This avoids type mismatch: Model has JoinField<Vec<T>>, ListModel has Vec<T>
                assignments.push(quote! {
                    #field_name: vec![]
                });
            } else {
                // Regular non-DB fields: use default or specified default
                let default_expr = get_crudcrate_expr(field, "default")
                    .unwrap_or_else(|| syn::parse_quote!(Default::default()));
                assignments.push(quote! {
                    #field_name: #default_expr
                });
            }
        }
        // Fields with list_model = false or join(one)-only fields are not included in ListModel struct, so skip them
    }

    assignments
}

/// Generate helper methods for join loading
#[allow(dead_code)]
fn generate_join_helper_methods(analysis: &EntityFieldAnalysis) -> proc_macro2::TokenStream {
    // Only generate if there are join(all) fields
    if analysis.join_on_all_fields.is_empty() {
        return quote! {};
    }

    // Generate join loading logic for join(all) fields
    let mut loading_statements = Vec::new();

    for field in &analysis.join_on_all_fields {
        if let Some(field_name) = &field.ident {
            let is_vec_field = is_vec_type(&field.ty);

            if is_vec_field {
                // Extract the target entity from Vec<TargetType>
                let target_entity_path = get_entity_path_from_field_type(&field.ty);

                loading_statements.push(quote! {
                    let #field_name: Vec<_> = sea_orm::ModelTrait::find_related(&original_model, #target_entity_path).all(db).await
                        .unwrap_or_default()
                        .into_iter().map(|related_model| related_model.into()).collect();
                    loaded_model.#field_name = #field_name;
                });
            } else {
                // For single T or Option<T> fields (belongs_to/has_one relationships)
                let target_entity_path = get_entity_path_from_field_type(&field.ty);

                loading_statements.push(quote! {
                    let #field_name = sea_orm::ModelTrait::find_related(&original_model, #target_entity_path).one(db).await.ok()
                        .flatten().map(|related_model| related_model.into());
                    loaded_model.#field_name = #field_name;
                });
            }
        }
    }

    quote! {
        /// Helper method to load join data for get_all endpoint
        async fn load_all_joins(
            db: &sea_orm::DatabaseConnection,
            mut loaded_model: Self,
            original_model: <Self as crudcrate::traits::CRUDResource>::EntityType::Model
        ) -> Result<Self, sea_orm::DbErr> {
            // Load all join fields for join(all)
            #(#loading_statements)*

            Ok(loaded_model)
        }
    }
}

/// Map field types to their corresponding entity paths
fn get_entity_path_from_field_type(field_type: &syn::Type) -> proc_macro2::TokenStream {
    // First, strip JoinField<T> wrapper if present
    let unwrapped_type = if let syn::Type::Path(type_path) = field_type {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "JoinField" {
                // Extract T from JoinField<T>
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        inner_ty
                    } else {
                        field_type
                    }
                } else {
                    field_type
                }
            } else {
                field_type
            }
        } else {
            field_type
        }
    } else {
        field_type
    };

    // Then, resolve the field type using the global registry to handle type aliases
    let resolved_type = if let Some(resolved_tokens) = super::two_pass_generator::resolve_join_type_globally(unwrapped_type) {
        if let Ok(parsed_type) = syn::parse2::<syn::Type>(resolved_tokens) {
            parsed_type
        } else {
            unwrapped_type.clone()
        }
    } else {
        unwrapped_type.clone()
    };

    // Extract the target type from Vec<T> or Option<T>
    let target_type = if let syn::Type::Path(type_path) = &resolved_type {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Vec" {
                // Vec<T> - extract T
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        inner_ty
                    } else {
                        field_type
                    }
                } else {
                    field_type
                }
            } else if segment.ident == "Option" {
                // Option<T> - extract T
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        inner_ty
                    } else {
                        field_type
                    }
                } else {
                    field_type
                }
            } else {
                // T (direct type)
                field_type
            }
        } else {
            field_type
        }
    } else {
        field_type
    };

    // Handle fully qualified paths like crate::sites::replicates::db::SiteReplicate
    if let syn::Type::Path(type_path) = target_type {
        if type_path.path.segments.len() > 1 {
            // For paths like crate::sites::replicates::db::SiteReplicate
            // Convert to crate::sites::replicates::db::Entity
            let mut path_segments = type_path.path.segments.clone();
            if let Some(last_segment) = path_segments.last_mut() {
                // Replace the last segment (e.g., SiteReplicate) with Entity
                last_segment.ident = syn::Ident::new("Entity", last_segment.ident.span());
                let modified_path = syn::Path {
                    leading_colon: type_path.path.leading_colon,
                    segments: path_segments,
                };
                return quote! { #modified_path };
            }
        } else if let Some(segment) = type_path.path.segments.last() {
            // Fallback: Convert TypeName to snake_case for simple paths
            // Handle API struct aliases by stripping common suffixes
            let type_name = segment.ident.to_string();
            let base_name = if type_name.ends_with("API") {
                // Convert VehicleAPI → Vehicle
                type_name.strip_suffix("API").unwrap_or(&type_name)
            } else {
                &type_name
            };
            let entity_name = base_name.to_case(Case::Snake);
            let entity_path = format_ident!("{}", entity_name);
            return quote! { super::#entity_path::Entity };
        }
    }

    quote! { Entity } // Fallback
}

/// Map field types to their corresponding model paths
fn get_model_path_from_field_type(field_type: &syn::Type) -> proc_macro2::TokenStream {
    // First, strip JoinField<T> wrapper if present
    let unwrapped_type = if let syn::Type::Path(type_path) = field_type {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "JoinField" {
                // Extract T from JoinField<T>
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        inner_ty
                    } else {
                        field_type
                    }
                } else {
                    field_type
                }
            } else {
                field_type
            }
        } else {
            field_type
        }
    } else {
        field_type
    };

    // Then, resolve the field type using the global registry to handle type aliases
    let resolved_type = if let Some(resolved_tokens) = super::two_pass_generator::resolve_join_type_globally(unwrapped_type) {
        if let Ok(parsed_type) = syn::parse2::<syn::Type>(resolved_tokens) {
            parsed_type
        } else {
            unwrapped_type.clone()
        }
    } else {
        unwrapped_type.clone()
    };

    // Extract the target type from Vec<T> or Option<T>
    let target_type = if let syn::Type::Path(type_path) = &resolved_type {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Vec" {
                // Vec<T> - extract T
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        inner_ty
                    } else {
                        field_type
                    }
                } else {
                    field_type
                }
            } else if segment.ident == "Option" {
                // Option<T> - extract T
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        inner_ty
                    } else {
                        field_type
                    }
                } else {
                    field_type
                }
            } else {
                // T (direct type)
                field_type
            }
        } else {
            field_type
        }
    } else {
        field_type
    };

    // Handle fully qualified paths like crate::sites::replicates::db::SiteReplicate
    if let syn::Type::Path(type_path) = target_type {
        if type_path.path.segments.len() > 1 {
            // For paths like crate::sites::replicates::db::SiteReplicate
            // Convert to crate::sites::replicates::db::Model
            let mut path_segments = type_path.path.segments.clone();
            if let Some(last_segment) = path_segments.last_mut() {
                // Replace the last segment (e.g., SiteReplicate) with Model
                last_segment.ident = syn::Ident::new("Model", last_segment.ident.span());
                let modified_path = syn::Path {
                    leading_colon: type_path.path.leading_colon,
                    segments: path_segments,
                };
                return quote! { #modified_path };
            }
        } else if let Some(segment) = type_path.path.segments.last() {
            // Fallback: Convert TypeName to snake_case::Model for simple paths
            // Handle API struct aliases by stripping common suffixes
            let type_name = segment.ident.to_string();
            let base_name = if type_name.ends_with("API") {
                // Convert VehicleAPI → Vehicle
                type_name.strip_suffix("API").unwrap_or(&type_name)
            } else {
                &type_name
            };
            let entity_name = base_name.to_case(Case::Snake);
            let model_path = format_ident!("{}", entity_name);
            return quote! { super::#model_path::Model };
        }
    }

    quote! { Model } // Fallback
}

/// Manual generation functions for Create and Update models when exclusions are present
/// Generate field declarations for manual Create struct based on `EntityFieldAnalysis`
pub(crate) fn generate_create_struct_fields_manual(
    analysis: &EntityFieldAnalysis,
) -> Vec<proc_macro2::TokenStream> {
    let mut fields = Vec::new();

    // Process database fields
    for field in &analysis.db_fields {
        if get_crudcrate_bool(field, "create_model") == Some(false) {
            continue; // Skip excluded fields
        }

        let field_name = &field.ident;
        let field_type = &field.ty;

        // Handle on_create expressions - make field optional if on_create is present
        let final_type = if get_crudcrate_expr(field, "on_create").is_some() {
            if field_is_optional(field) {
                // Already optional, keep as is but add default
                quote! { Option<#field_type> }
            } else {
                // Make optional since we have a default
                quote! { Option<#field_type> }
            }
        } else {
            quote! { #field_type }
        };

        let serde_attrs = if get_crudcrate_expr(field, "on_create").is_some() {
            quote! { #[serde(default)] }
        } else {
            quote! {}
        };

        fields.push(quote! {
            #serde_attrs
            pub #field_name: #final_type
        });
    }

    // Process non-database fields
    for field in &analysis.non_db_fields {
        if get_crudcrate_bool(field, "create_model") == Some(false) {
            continue; // Skip excluded fields
        }

        let field_name = &field.ident;
        let field_type = &field.ty;
        let default_expr = get_crudcrate_expr(field, "default")
            .unwrap_or_else(|| syn::parse_quote!(Default::default()));

        fields.push(quote! {
            #[serde(default = #default_expr)]
            pub #field_name: #field_type
        });
    }

    fields
}

/// Generate conversion logic for manual Create struct
pub(crate) fn generate_create_conversion_manual(
    analysis: &EntityFieldAnalysis,
    _active_model_path: &str,
) -> Vec<proc_macro2::TokenStream> {
    let mut conversions = Vec::new();
    let _active_model_ident = syn::Ident::new(_active_model_path, proc_macro2::Span::call_site());

    // Process database fields
    for field in &analysis.db_fields {
        let field_name = &field.ident;

        if get_crudcrate_bool(field, "create_model") == Some(false) {
            // Field is excluded from create model - apply on_create if present
            if let Some(on_create_expr) = get_crudcrate_expr(field, "on_create") {
                conversions.push(quote! {
                    #field_name: sea_orm::ActiveValue::Set(#on_create_expr.into())
                });
            } else {
                conversions.push(quote! {
                    #field_name: sea_orm::ActiveValue::NotSet
                });
            }
            continue;
        }

        // Field is included in create model
        if let Some(on_create_expr) = get_crudcrate_expr(field, "on_create") {
            if field_is_optional(field) {
                conversions.push(quote! {
                    #field_name: match create.#field_name {
                        Some(Some(v)) => sea_orm::ActiveValue::Set(Some(v.into())),
                        Some(None) => sea_orm::ActiveValue::Set(None),
                        None => sea_orm::ActiveValue::Set(#on_create_expr.into()),
                    }
                });
            } else {
                conversions.push(quote! {
                    #field_name: match create.#field_name {
                        Some(v) => sea_orm::ActiveValue::Set(v.into()),
                        None => sea_orm::ActiveValue::Set(#on_create_expr.into()),
                    }
                });
            }
        } else if field_is_optional(field) {
            conversions.push(quote! {
                #field_name: sea_orm::ActiveValue::Set(create.#field_name.map(|v| v.into()))
            });
        } else {
            conversions.push(quote! {
                #field_name: sea_orm::ActiveValue::Set(create.#field_name.into())
            });
        }
    }

    // Process non-database fields (typically set to NotSet)
    for field in &analysis.non_db_fields {
        let field_name = &field.ident;
        conversions.push(quote! {
            #field_name: sea_orm::ActiveValue::NotSet
        });
    }

    conversions
}

/// Generate field declarations for manual Update struct based on `EntityFieldAnalysis`
pub(crate) fn generate_update_struct_fields_manual(
    analysis: &EntityFieldAnalysis,
) -> Vec<proc_macro2::TokenStream> {
    let mut fields = Vec::new();

    // Process database fields
    for field in &analysis.db_fields {
        if get_crudcrate_bool(field, "update_model") == Some(false) {
            continue; // Skip excluded fields
        }

        let field_name = &field.ident;
        let field_type = &field.ty;

        // Update fields should be optional (allow partial updates)
        let final_type = if field_is_optional(field) {
            quote! { Option<#field_type> }
        } else {
            quote! { Option<#field_type> }
        };

        fields.push(quote! {
            pub #field_name: #final_type
        });
    }

    // Process non-database fields
    for field in &analysis.non_db_fields {
        if get_crudcrate_bool(field, "update_model") == Some(false) {
            continue; // Skip excluded fields
        }

        let field_name = &field.ident;
        let field_type = &field.ty;
        let final_type = if field_is_optional(field) {
            quote! { Option<#field_type> }
        } else {
            quote! { Option<#field_type> }
        };

        fields.push(quote! {
            pub #field_name: #final_type
        });
    }

    fields
}

/// Generate conversion logic for manual Update struct
pub(crate) fn generate_update_conversion_manual(
    analysis: &EntityFieldAnalysis,
    _active_model_path: &str,
) -> Vec<proc_macro2::TokenStream> {
    let mut conversions = Vec::new();

    // Process database fields
    for field in &analysis.db_fields {
        let field_name = &field.ident;

        if get_crudcrate_bool(field, "update_model") == Some(false) {
            // Field is excluded from update model - apply on_update if present
            if let Some(on_update_expr) = get_crudcrate_expr(field, "on_update") {
                conversions.push(quote! {
                    model.#field_name = sea_orm::ActiveValue::Set(#on_update_expr.into());
                });
            }
            continue;
        }

        // Field is included in update model
        if field_is_optional(field) {
            conversions.push(quote! {
                if let Some(update_field) = update.#field_name {
                    model.#field_name = sea_orm::ActiveValue::Set(update_field.map(|v| v.into()));
                }
            });
        } else {
            conversions.push(quote! {
                if let Some(update_field) = update.#field_name {
                    model.#field_name = sea_orm::ActiveValue::Set(update_field.into());
                }
            });
        }
    }

    // Non-database fields don't need conversion in updates

    conversions
}
