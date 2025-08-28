use quote::{format_ident, quote, ToTokens};
use syn::{Ident, Type};
use super::structs::{CRUDResourceMeta, EntityFieldAnalysis};
use super::attribute_parser::{get_crudcrate_bool, get_crudcrate_expr, field_has_crudcrate_flag, ident_to_string};
use super::field_analyzer::{field_is_optional, resolve_target_models, resolve_target_models_with_list, extract_inner_type_for_update};

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

/// Generates the merge code for update models
pub(crate) fn generate_update_merge_code(
    all_fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    included_fields: &[&syn::Field],
) -> (Vec<proc_macro2::TokenStream>, Vec<proc_macro2::TokenStream>) {
    let mut included_merge = Vec::new();
    let mut excluded_merge = Vec::new();

    for field in included_fields {
        let field_name = field.ident.as_ref().unwrap();
        let is_optional = field_is_optional(field);

        if is_optional {
            included_merge.push(quote! {
                if let Some(value) = self.#field_name {
                    model.#field_name = match value {
                        Some(v) => sea_orm::ActiveValue::Set(Some(v.into())),
                        None => sea_orm::ActiveValue::Set(None),
                    };
                }
            });
        } else {
            included_merge.push(quote! {
                if let Some(value) = self.#field_name {
                    model.#field_name = sea_orm::ActiveValue::Set(value.into());
                }
            });
        }
    }

    for field in all_fields {
        let field_name = field.ident.as_ref().unwrap();
        let is_optional = field_is_optional(field);

        if !included_fields.iter().any(|f| f.ident == field.ident) {
            if let Some(expr) = get_crudcrate_expr(field, "on_update") {
                if is_optional {
                    excluded_merge.push(quote! {
                        model.#field_name = sea_orm::ActiveValue::Set(Some((#expr).into()));
                    });
                } else {
                    excluded_merge.push(quote! {
                        model.#field_name = sea_orm::ActiveValue::Set((#expr).into());
                    });
                }
            }
        }
    }

    (included_merge, excluded_merge)
}

/// Generates field declarations for list struct
pub(crate) fn generate_list_struct_fields(fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>) -> Vec<proc_macro2::TokenStream> {
    let mut result = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;

        if get_crudcrate_bool(field, "list_model").unwrap_or(true) {
            if field_has_crudcrate_flag(field, "use_target_models") {
                if let Some((_create_model, _update_model, list_model)) = resolve_target_models_with_list(field_type) {
                    if field_type.to_token_stream().to_string().starts_with("Vec<") {
                        result.push(quote! {
                            pub #field_name: Vec<#list_model>
                        });
                    } else {
                        result.push(quote! {
                            pub #field_name: #list_model
                        });
                    }
                } else {
                    result.push(quote! {
                        pub #field_name: #field_type
                    });
                }
            } else {
                result.push(quote! {
                    pub #field_name: #field_type
                });
            }
        }
    }

    result
}

/// Generates assignment expressions for From<Model> for List conversion
pub(crate) fn generate_list_from_assignments(fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>) -> Vec<proc_macro2::TokenStream> {
    let mut result = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();

        if get_crudcrate_bool(field, "list_model").unwrap_or(true) {
            if field_has_crudcrate_flag(field, "use_target_models") {
                if field.ty.to_token_stream().to_string().starts_with("Vec<") {
                    result.push(quote! {
                        #field_name: model.#field_name.into_iter().map(|item| item.into()).collect()
                    });
                } else {
                    result.push(quote! {
                        #field_name: model.#field_name.into()
                    });
                }
            } else {
                result.push(quote! {
                    #field_name: model.#field_name
                });
            }
        }
    }

    result
}

/// Generate field assignments for List model From<Model> implementation (direct from DB Model)
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
                // This field uses target models, so we need to convert the Model field to the target's List model
                if field.ty.to_token_stream().to_string().starts_with("Vec<") {
                    // For Vec<Item>, collect into Vec<ItemList>
                    assignments.push(quote! {
                        #field_name: model.#field_name.into_iter().map(|item| item.into()).collect()
                    });
                } else {
                    // For single item, use direct conversion
                    assignments.push(quote! {
                        #field_name: model.#field_name.into()
                    });
                }
            } else {
                assignments.push(quote! {
                    #field_name: model.#field_name
                });
            }
        }
    }

    // Handle non-DB fields that are included in ListModel
    for field in &analysis.non_db_fields {
        let field_name = &field.ident;

        if get_crudcrate_bool(field, "list_model").unwrap_or(true) {
            // For non-DB fields, use the default value from the original field
            let default_expr = get_crudcrate_expr(field, "default")
                .unwrap_or_else(|| syn::parse_quote!(Default::default()));

            assignments.push(quote! {
                #field_name: #default_expr
            });
        }
    }

    assignments
}

/// Generates the API struct fields and From implementation assignments
pub(crate) fn generate_api_struct_content(analysis: &EntityFieldAnalysis) -> (Vec<proc_macro2::TokenStream>, Vec<proc_macro2::TokenStream>) {
    let mut api_struct_fields = Vec::new();
    let mut from_model_assignments = Vec::new();

    for field in analysis.db_fields.iter().chain(analysis.non_db_fields.iter()) {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;

        if field_has_crudcrate_flag(field, "use_target_models") {
            if let Some((_create_model, _update_model, list_model)) = resolve_target_models_with_list(field_type) {
                if field_type.to_token_stream().to_string().starts_with("Vec<") {
                    api_struct_fields.push(quote! {
                        pub #field_name: Vec<#list_model>
                    });
                    from_model_assignments.push(quote! {
                        #field_name: model.#field_name.into_iter().map(|item| item.into()).collect()
                    });
                } else {
                    api_struct_fields.push(quote! {
                        pub #field_name: #list_model
                    });
                    from_model_assignments.push(quote! {
                        #field_name: model.#field_name.into()
                    });
                }
            } else {
                api_struct_fields.push(quote! {
                    pub #field_name: #field_type
                });
                from_model_assignments.push(quote! {
                    #field_name: model.#field_name
                });
            }
        } else if get_crudcrate_bool(field, "non_db_attr").unwrap_or(false) {
            if let Some(default_expr) = get_crudcrate_expr(field, "default") {
                api_struct_fields.push(quote! {
                    pub #field_name: #field_type
                });
                from_model_assignments.push(quote! {
                    #field_name: #default_expr
                });
            } else {
                api_struct_fields.push(quote! {
                    pub #field_name: #field_type
                });
                from_model_assignments.push(quote! {
                    #field_name: Default::default()
                });
            }
        } else {
            api_struct_fields.push(quote! {
                pub #field_name: #field_type
            });
            from_model_assignments.push(quote! {
                #field_name: model.#field_name
            });
        }
    }

    (api_struct_fields, from_model_assignments)
}

/// Generates the API struct definition
pub(crate) fn generate_api_struct(
    api_struct_name: &Ident,
    api_struct_fields: &[proc_macro2::TokenStream],
    active_model_path: &str,
) -> proc_macro2::TokenStream {
    let active_model_type: Type = syn::parse_str(active_model_path).unwrap();

    quote! {
        #[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
        pub struct #api_struct_name {
            #(#api_struct_fields),*
        }

        impl #api_struct_name {
            pub fn to_activemodel(&self) -> #active_model_type {
                use sea_orm::ActiveValue;
                #active_model_type {
                    id: ActiveValue::NotSet,
                    ..Default::default()
                }
            }
        }
    }
}

/// Generates the From<Model> for ApiStruct implementation
pub(crate) fn generate_from_impl(
    model_name: &Ident,
    api_struct_name: &Ident,
    from_model_assignments: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    quote! {
        impl From<#model_name> for #api_struct_name {
            fn from(model: #model_name) -> Self {
                Self {
                    #(#from_model_assignments),*
                }
            }
        }
    }
}

/// Generates conditional CRUD implementation based on metadata
pub(crate) fn generate_conditional_crud_impl(
    api_struct_name: &Ident,
    crud_meta: &CRUDResourceMeta,
    active_model_path: &str,
    analysis: &EntityFieldAnalysis,
    table_name: &str,
) -> proc_macro2::TokenStream {
    let active_model_type: Type = syn::parse_str(active_model_path).unwrap();
    let create_name = format_ident!("{}Create", api_struct_name);
    let update_name = format_ident!("{}Update", api_struct_name);
    let list_name = format_ident!("{}List", api_struct_name);

    let primary_key_field = analysis.primary_key_field.unwrap();
    let pk_field_name = primary_key_field.ident.as_ref().unwrap();
    let pk_field_type = &primary_key_field.ty;

    let description = crud_meta.description.as_ref().unwrap();
    let name_singular = crud_meta.name_singular.as_ref().unwrap();
    let name_plural = crud_meta.name_plural.as_ref().unwrap();
    let entity_type = crud_meta.entity_type.as_ref().unwrap();
    let column_type = crud_meta.column_type.as_ref().unwrap();

    let entity_type_ident = format_ident!("{}", entity_type);
    let column_type_ident = format_ident!("{}", column_type);

    let sortable_columns = generate_sortable_columns_list(analysis);
    let filterable_columns = generate_filterable_columns_list(analysis);
    let fulltext_columns = generate_fulltext_columns_list(analysis);

    let get_one_impl = generate_get_one_impl(crud_meta, &entity_type_ident);
    let get_all_impl = generate_get_all_impl(crud_meta, &entity_type_ident, &column_type_ident);
    let create_impl = generate_create_impl(crud_meta, &entity_type_ident);
    let update_impl = generate_update_impl(crud_meta, &entity_type_ident, pk_field_name);
    let delete_impl = generate_delete_impl(crud_meta, &entity_type_ident, pk_field_name);
    let delete_many_impl = generate_delete_many_impl(crud_meta, &entity_type_ident, &column_type_ident, pk_field_name);

    let router_impl = if crud_meta.generate_router {
        generate_router_impl(api_struct_name, table_name)
    } else {
        quote! {}
    };

    quote! {
        impl crudcrate::traits::CRUDResource for #api_struct_name {
            type Model = #api_struct_name;
            type CreateModel = #create_name;
            type UpdateModel = #update_name;
            type ListModel = #list_name;
            type Entity = #entity_type_ident;
            type ActiveModel = #active_model_type;
            type PrimaryKeyType = #pk_field_type;
            type ColumnType = #column_type_ident;

            fn description() -> &'static str {
                #description
            }

            fn name_singular() -> &'static str {
                #name_singular
            }

            fn name_plural() -> &'static str {
                #name_plural
            }

            fn primary_key_column() -> Self::ColumnType {
                #column_type_ident::#pk_field_name
            }

            fn sortable_columns() -> Vec<String> {
                #sortable_columns
            }

            fn filterable_columns() -> Vec<String> {
                #filterable_columns
            }

            fn fulltext_columns() -> Vec<String> {
                #fulltext_columns
            }

            #get_one_impl
            #get_all_impl
            #create_impl
            #update_impl
            #delete_impl
            #delete_many_impl
        }

        #router_impl
    }
}

/// Generates the sortable columns list
fn generate_sortable_columns_list(analysis: &EntityFieldAnalysis) -> proc_macro2::TokenStream {
    let columns: Vec<proc_macro2::TokenStream> = analysis
        .sortable_fields
        .iter()
        .map(|field| {
            let field_name = ident_to_string(field.ident.as_ref().unwrap());
            quote! { #field_name.to_string() }
        })
        .collect();

    quote! { vec![#(#columns),*] }
}

/// Generates the filterable columns list
fn generate_filterable_columns_list(analysis: &EntityFieldAnalysis) -> proc_macro2::TokenStream {
    let columns: Vec<proc_macro2::TokenStream> = analysis
        .filterable_fields
        .iter()
        .map(|field| {
            let field_name = ident_to_string(field.ident.as_ref().unwrap());
            quote! { #field_name.to_string() }
        })
        .collect();

    quote! { vec![#(#columns),*] }
}

/// Generates the fulltext columns list
fn generate_fulltext_columns_list(analysis: &EntityFieldAnalysis) -> proc_macro2::TokenStream {
    let columns: Vec<proc_macro2::TokenStream> = analysis
        .fulltext_fields
        .iter()
        .map(|field| {
            let field_name = ident_to_string(field.ident.as_ref().unwrap());
            quote! { #field_name.to_string() }
        })
        .collect();

    quote! { vec![#(#columns),*] }
}

/// Generates get_one implementation
fn generate_get_one_impl(crud_meta: &CRUDResourceMeta, entity_type: &Ident) -> proc_macro2::TokenStream {
    if let Some(custom_fn) = &crud_meta.fn_get_one {
        quote! {
            async fn get_one(
                db: &sea_orm::DatabaseConnection,
                id: Self::PrimaryKeyType,
            ) -> Result<Option<Self::Model>, sea_orm::DbErr> {
                #custom_fn(db, id).await
            }
        }
    } else {
        quote! {
            async fn get_one(
                db: &sea_orm::DatabaseConnection,
                id: Self::PrimaryKeyType,
            ) -> Result<Option<Self::Model>, sea_orm::DbErr> {
                use sea_orm::EntityTrait;
                #entity_type::find_by_id(id)
                    .one(db)
                    .await
                    .map(|opt| opt.map(Into::into))
            }
        }
    }
}

/// Generates get_all implementation
fn generate_get_all_impl(
    crud_meta: &CRUDResourceMeta,
    entity_type: &Ident,
    column_type: &Ident,
) -> proc_macro2::TokenStream {
    if let Some(custom_fn) = &crud_meta.fn_get_all {
        quote! {
            async fn get_all(
                db: &sea_orm::DatabaseConnection,
                filter_params: std::collections::HashMap<String, String>,
            ) -> Result<crudcrate::CrudResponse<Self::ListModel>, sea_orm::DbErr> {
                #custom_fn(db, filter_params).await
            }
        }
    } else {
        quote! {
            async fn get_all(
                db: &sea_orm::DatabaseConnection,
                filter_params: std::collections::HashMap<String, String>,
            ) -> Result<crudcrate::CrudResponse<Self::ListModel>, sea_orm::DbErr> {
                use sea_orm::{EntityTrait, QueryFilter, QueryOrder, PaginatorTrait};
                use crudcrate::filter::apply_filters;

                let query = #entity_type::find();
                let filtered_query = apply_filters::<#entity_type, #column_type>(query, &filter_params)?;
                let (items, total) = filtered_query.paginate(db, 50).fetch_and_count().await?;

                Ok(crudcrate::CrudResponse {
                    items: items.into_iter().map(Into::into).collect(),
                    total: total as usize,
                    page: 1,
                    per_page: 50,
                })
            }
        }
    }
}

/// Generates create implementation
fn generate_create_impl(crud_meta: &CRUDResourceMeta, _entity_type: &Ident) -> proc_macro2::TokenStream {
    if let Some(custom_fn) = &crud_meta.fn_create {
        quote! {
            async fn create(
                db: &sea_orm::DatabaseConnection,
                create_model: Self::CreateModel,
            ) -> Result<Self::Model, sea_orm::DbErr> {
                #custom_fn(db, create_model).await
            }
        }
    } else {
        quote! {
            async fn create(
                db: &sea_orm::DatabaseConnection,
                create_model: Self::CreateModel,
            ) -> Result<Self::Model, sea_orm::DbErr> {
                use sea_orm::{EntityTrait, ActiveModelTrait};
                let active_model: Self::ActiveModel = create_model.into();
                let model = active_model.insert(db).await?;
                Ok(model.into())
            }
        }
    }
}

/// Generates update implementation
fn generate_update_impl(
    crud_meta: &CRUDResourceMeta,
    entity_type: &Ident,
    pk_field_name: &Ident,
) -> proc_macro2::TokenStream {
    if let Some(custom_fn) = &crud_meta.fn_update {
        quote! {
            async fn update(
                db: &sea_orm::DatabaseConnection,
                id: Self::PrimaryKeyType,
                update_model: Self::UpdateModel,
            ) -> Result<Option<Self::Model>, sea_orm::DbErr> {
                #custom_fn(db, id, update_model).await
            }
        }
    } else {
        quote! {
            async fn update(
                db: &sea_orm::DatabaseConnection,
                id: Self::PrimaryKeyType,
                update_model: Self::UpdateModel,
            ) -> Result<Option<Self::Model>, sea_orm::DbErr> {
                use sea_orm::{EntityTrait, ActiveModelTrait, ActiveValue};
                use crudcrate::traits::MergeIntoActiveModel;

                if let Some(existing) = #entity_type::find_by_id(id).one(db).await? {
                    let mut active_model: Self::ActiveModel = existing.into();
                    active_model.#pk_field_name = ActiveValue::Set(id);
                    let merged = update_model.merge_into_activemodel(active_model)?;
                    let updated = merged.update(db).await?;
                    Ok(Some(updated.into()))
                } else {
                    Ok(None)
                }
            }
        }
    }
}

/// Generates delete implementation
fn generate_delete_impl(
    crud_meta: &CRUDResourceMeta,
    entity_type: &Ident,
    pk_field_name: &Ident,
) -> proc_macro2::TokenStream {
    if let Some(custom_fn) = &crud_meta.fn_delete {
        quote! {
            async fn delete(
                db: &sea_orm::DatabaseConnection,
                id: Self::PrimaryKeyType,
            ) -> Result<bool, sea_orm::DbErr> {
                #custom_fn(db, id).await
            }
        }
    } else {
        quote! {
            async fn delete(
                db: &sea_orm::DatabaseConnection,
                id: Self::PrimaryKeyType,
            ) -> Result<bool, sea_orm::DbErr> {
                use sea_orm::{EntityTrait, ActiveModelTrait, ActiveValue};

                if #entity_type::find_by_id(id).one(db).await?.is_some() {
                    let mut active_model = Self::ActiveModel::default();
                    active_model.#pk_field_name = ActiveValue::Set(id);
                    active_model.delete(db).await?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }
}

/// Generates delete_many implementation
fn generate_delete_many_impl(
    crud_meta: &CRUDResourceMeta,
    entity_type: &Ident,
    column_type: &Ident,
    pk_field_name: &Ident,
) -> proc_macro2::TokenStream {
    if let Some(custom_fn) = &crud_meta.fn_delete_many {
        quote! {
            async fn delete_many(
                db: &sea_orm::DatabaseConnection,
                ids: Vec<Self::PrimaryKeyType>,
            ) -> Result<u64, sea_orm::DbErr> {
                #custom_fn(db, ids).await
            }
        }
    } else {
        quote! {
            async fn delete_many(
                db: &sea_orm::DatabaseConnection,
                ids: Vec<Self::PrimaryKeyType>,
            ) -> Result<u64, sea_orm::DbErr> {
                use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                let result = #entity_type::delete_many()
                    .filter(#column_type::#pk_field_name.is_in(ids))
                    .exec(db)
                    .await?;
                Ok(result.rows_affected)
            }
        }
    }
}

/// Generates router implementation if enabled
fn generate_router_impl(api_struct_name: &Ident, table_name: &str) -> proc_macro2::TokenStream {
    let router_fn_name = format_ident!("{}_router", table_name);

    quote! {
        pub fn #router_fn_name() -> axum::Router {
            use axum::routing::{get, post, put, delete};

            axum::Router::new()
                .route(&format!("/{}", #table_name), get(crudcrate::handlers::get_all::<#api_struct_name>))
                .route(&format!("/{}", #table_name), post(crudcrate::handlers::create::<#api_struct_name>))
                .route(&format!("/{}/:id", #table_name), get(crudcrate::handlers::get_one::<#api_struct_name>))
                .route(&format!("/{}/:id", #table_name), put(crudcrate::handlers::update::<#api_struct_name>))
                .route(&format!("/{}/:id", #table_name), delete(crudcrate::handlers::delete::<#api_struct_name>))
                .route(&format!("/{}/bulk", #table_name), delete(crudcrate::handlers::delete_many::<#api_struct_name>))
        }
    }
}