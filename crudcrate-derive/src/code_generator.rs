use convert_case::{Case, Casing};
use quote::{format_ident, quote, ToTokens};
use super::attribute_parser::{get_crudcrate_bool, get_crudcrate_expr, field_has_crudcrate_flag};
use super::field_analyzer::{field_is_optional, resolve_target_models, extract_inner_type_for_update, resolve_target_models_with_list};
use super::structs::{CRUDResourceMeta, EntityFieldAnalysis, Framework};

/// Generates the field declarations for a create struct
pub(crate) fn generate_create_struct_fields(fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| get_crudcrate_bool(field, "create_model").unwrap_or(true))
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
pub(crate) fn generate_create_conversion_lines(fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>) -> Vec<proc_macro2::TokenStream> {
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
pub(crate) fn filter_update_fields(fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>) -> Vec<&syn::Field> {
    fields
        .iter()
        .filter(|field| get_crudcrate_bool(field, "update_model").unwrap_or(true))
        .collect()
}

/// Generates the field declarations for an update struct
pub(crate) fn generate_update_struct_fields(included_fields: &[&syn::Field]) -> Vec<proc_macro2::TokenStream> {
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

pub(crate) fn generate_router_impl(api_struct_name: &syn::Ident, framework: &Framework) -> proc_macro2::TokenStream {
    let create_model_name = format_ident!("{}Create", api_struct_name);
    let update_model_name = format_ident!("{}Update", api_struct_name);
    let list_model_name = format_ident!("{}List", api_struct_name);

    match framework {
        Framework::Axum => generate_axum_router(api_struct_name, &create_model_name, &update_model_name, &list_model_name),
        Framework::SpringRs => generate_spring_router(api_struct_name, &create_model_name, &update_model_name, &list_model_name),
    }
}

fn generate_axum_router(
    api_struct_name: &syn::Ident,
    create_model_name: &syn::Ident,
    update_model_name: &syn::Ident,
    list_model_name: &syn::Ident,
) -> proc_macro2::TokenStream {
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

#[allow(clippy::too_many_lines)]
fn generate_spring_router(
    api_struct_name: &syn::Ident,
    create_model_name: &syn::Ident,
    update_model_name: &syn::Ident,
    list_model_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    quote! {
        // Spring-RS CRUD handlers
        use spring_web::{get, post, put, delete};
        use spring_web::{
            axum::response::{IntoResponse, Json},
            error::Result,
            extractor::{Component, Path, Query},
        };
        use spring_sea_orm::DbConn;
        use hyper::{StatusCode, HeaderMap};
        use uuid::Uuid;
        use crudcrate::models::FilterOptions;

        #[get("/{id}")]
        #[utoipa::path(
            get,
            path = "/{id}",
            responses(
                (status = StatusCode::OK, description = "The requested resource", body = #api_struct_name),
                (status = StatusCode::NOT_FOUND, description = "Resource not found"),
                (status = StatusCode::INTERNAL_SERVER_ERROR, description = "Internal Server Error")
            ),
            operation_id = format!("get_one_{}", <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR),
            summary = format!("Get one {}", <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR),
            description = format!("Retrieves one {} by its ID.\n\n{}", <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR, <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_DESCRIPTION)
        )]
        pub async fn get_one_handler(
            Component(db): Component<DbConn>,
            Path(id): Path<Uuid>,
        ) -> Result<Json<#api_struct_name>, (StatusCode, Json<String>)> {
            match <#api_struct_name as crudcrate::traits::CRUDResource>::get_one(&db, id).await {
                Ok(item) => Ok(Json(item)),
                Err(sea_orm::DbErr::RecordNotFound(_)) => {
                    Err((StatusCode::NOT_FOUND, Json("Not Found".to_string())))
                }
                Err(_) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json("Internal Server Error".to_string()))),
            }
        }

        #[get("/")]
        #[utoipa::path(
            get,
            path = "/",
            responses(
                (status = StatusCode::OK, description = "List of resources", body = [#list_model_name]),
                (status = StatusCode::INTERNAL_SERVER_ERROR, description = "Internal Server Error")
            ),
            params(crudcrate::models::FilterOptions),
            operation_id = format!("get_all_{}", <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_PLURAL),
            summary = format!("Get all {}", <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_PLURAL),
            description = format!(
                "Retrieves all {}.\n\n{}\n\nAdditional sortable columns: {}.\n\nAdditional filterable columns: {}.",
                <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_PLURAL,
                <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_DESCRIPTION,
                <#api_struct_name as crudcrate::traits::CRUDResource>::sortable_columns()
                    .iter()
                    .map(|(name, _)| format!("\n- {}", name))
                    .collect::<Vec<String>>()
                    .join(""),
                <#api_struct_name as crudcrate::traits::CRUDResource>::filterable_columns()
                    .iter()
                    .map(|(name, _)| format!("\n- {}", name))
                    .collect::<Vec<String>>()
                    .join("")
            )
        )]
        pub async fn get_all_handler(
            Query(params): Query<FilterOptions>,
            Component(db): Component<DbConn>,
        ) -> Result<(HeaderMap, Json<Vec<#list_model_name>>), (StatusCode, Json<String>)> {
            let (offset, limit) = crudcrate::filter::parse_pagination(&params);
            let condition = crudcrate::filter::apply_filters::<#api_struct_name>(
                params.filter.clone(), 
                &<#api_struct_name as crudcrate::traits::CRUDResource>::filterable_columns(), 
                db.get_database_backend()
            );
            let (order_column, order_direction) = crudcrate::sort::parse_sorting(
                &params,
                &<#api_struct_name as crudcrate::traits::CRUDResource>::sortable_columns(),
                <#api_struct_name as crudcrate::traits::CRUDResource>::ID_COLUMN,
            );
            
            let items = match <#api_struct_name as crudcrate::traits::CRUDResource>::get_all(&db, &condition, order_column, order_direction, offset, limit).await {
                Ok(items) => items,
                Err(err) => return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(err.to_string()))),
            };
            
            let total_count = <#api_struct_name as crudcrate::traits::CRUDResource>::total_count(&db, &condition).await;
            let headers = crudcrate::pagination::calculate_content_range(
                offset, 
                limit, 
                total_count, 
                <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_PLURAL
            );
            
            Ok((headers, Json(items)))
        }

        #[post("/")]
        #[utoipa::path(
            post,
            path = "/",
            request_body = #create_model_name,
            responses(
                (status = StatusCode::CREATED, description = "Resource created successfully", body = #api_struct_name),
                (status = StatusCode::CONFLICT, description = "Duplicate record", body = String)
            ),
            operation_id = format!("create_one_{}", <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR),
            summary = format!("Create one {}", <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR),
            description = format!("Creates a new {}.\n\n{}", <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR, <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_DESCRIPTION)
        )]
        pub async fn create_one_handler(
            Component(db): Component<DbConn>,
            Json(json): Json<#create_model_name>,
        ) -> Result<(StatusCode, Json<#api_struct_name>), (StatusCode, Json<String>)> {
            <#api_struct_name as crudcrate::traits::CRUDResource>::create(&db, json)
                .await
                .map(|res| (StatusCode::CREATED, Json(res)))
                .map_err(|err| {
                    if let Some(sea_orm::SqlErr::UniqueConstraintViolation(detail)) = err.sql_err() {
                        (StatusCode::CONFLICT, Json(format!("Conflict: {}", detail)))
                    } else {
                        (StatusCode::INTERNAL_SERVER_ERROR, Json("Internal Server Error".to_string()))
                    }
                })
        }

        #[put("/{id}")]
        #[utoipa::path(
            put,
            path = "/{id}",
            request_body = #update_model_name,
            responses(
                (status = StatusCode::OK, description = "Resource updated successfully", body = #api_struct_name),
                (status = StatusCode::NOT_FOUND, description = "Resource not found"),
                (status = StatusCode::CONFLICT, description = "Duplicate record", body = String)
            ),
            operation_id = format!("update_one_{}", <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR),
            summary = format!("Update one {}", <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR),
            description = format!("Updates one {} by its ID.\n\n{}", <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR, <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_DESCRIPTION)
        )]
        pub async fn update_one_handler(
            Component(db): Component<DbConn>,
            Path(id): Path<Uuid>,
            Json(json): Json<#update_model_name>,
        ) -> Result<Json<#api_struct_name>, (StatusCode, Json<String>)> {
            <#api_struct_name as crudcrate::traits::CRUDResource>::update(&db, id, json)
                .await
                .map(Json)
                .map_err(|err| {
                    match err {
                        sea_orm::DbErr::Custom(msg) => (StatusCode::UNPROCESSABLE_ENTITY, Json(msg)),
                        sea_orm::DbErr::RecordNotFound(_) => (StatusCode::NOT_FOUND, Json("Not Found".to_string())),
                        _ => {
                            if let Some(sea_orm::SqlErr::UniqueConstraintViolation(detail)) = err.sql_err() {
                                (StatusCode::CONFLICT, Json(format!("Conflict: {}", detail)))
                            } else {
                                (StatusCode::INTERNAL_SERVER_ERROR, Json("Internal Server Error".to_string()))
                            }
                        }
                    }
                })
        }

        #[delete("/{id}")]
        #[utoipa::path(
            delete,
            path = "/{id}",
            responses(
                (status = StatusCode::NO_CONTENT, description = "Resource deleted successfully"),
                (status = StatusCode::NOT_FOUND, description = "Resource not found"),
                (status = StatusCode::INTERNAL_SERVER_ERROR, description = "Internal Server Error")
            ),
            operation_id = format!("delete_one_{}", <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR),
            summary = format!("Delete one {}", <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR),
            description = format!("Deletes one {} by its ID.\n\n{}", <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_SINGULAR, <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_DESCRIPTION)
        )]
        pub async fn delete_one_handler(
            Component(db): Component<DbConn>,
            Path(id): Path<Uuid>,
        ) -> Result<StatusCode, (StatusCode, Json<String>)> {
            <#api_struct_name as crudcrate::traits::CRUDResource>::delete(&db, id)
                .await
                .map(|_| StatusCode::NO_CONTENT)
                .map_err(|err| {
                    match err {
                        sea_orm::DbErr::RecordNotFound(_) => (StatusCode::NOT_FOUND, Json("Not Found".to_string())),
                        _ => (StatusCode::INTERNAL_SERVER_ERROR, Json("Internal Server Error".to_string())),
                    }
                })
        }

        #[delete("/batch")]
        #[utoipa::path(
            delete,
            path = "/batch",
            responses(
                (status = StatusCode::OK, description = "Resources deleted successfully", body = [String]),
                (status = StatusCode::INTERNAL_SERVER_ERROR, description = "Internal Server Error", body = String)
            ),
            operation_id = format!("delete_many_{}", <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_PLURAL),
            summary = format!("Delete many {}", <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_PLURAL),
            description = format!("Deletes many {} by their IDs.\n\n{}", <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_NAME_PLURAL, <#api_struct_name as crudcrate::traits::CRUDResource>::RESOURCE_DESCRIPTION)
        )]
        pub async fn delete_many_handler(
            Component(db): Component<DbConn>,
            Json(ids): Json<Vec<Uuid>>,
        ) -> Result<(StatusCode, Json<Vec<Uuid>>), (StatusCode, Json<String>)> {
            <#api_struct_name as crudcrate::traits::CRUDResource>::delete_many(&db, ids)
                .await
                .map(|deleted_ids| (StatusCode::OK, Json(deleted_ids)))
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json("Internal Server Error".to_string())))
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

fn generate_id_column(
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

fn generate_field_entries(fields: &[&syn::Field]) -> Vec<proc_macro2::TokenStream> {
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

fn generate_like_filterable_entries(
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

fn generate_fulltext_field_entries(
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

pub(crate) fn generate_list_struct_fields(
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

pub(crate) fn generate_list_from_assignments(
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