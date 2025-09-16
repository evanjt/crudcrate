use super::attribute_parser::{field_has_crudcrate_flag, get_crudcrate_bool, get_crudcrate_expr};
use super::field_analyzer::{
    extract_inner_type_for_update, field_is_optional, resolve_target_models,
    resolve_target_models_with_list,
};
use super::structs::{CRUDResourceMeta, EntityFieldAnalysis};
use convert_case::{Case, Casing};
use quote::{ToTokens, format_ident, quote};

/// Generates the field declarations for a create struct
pub(crate) fn generate_create_struct_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
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
        .filter(|field| get_crudcrate_bool(field, "update_model").unwrap_or(true))
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

    generate_axum_router(
        api_struct_name,
        &create_model_name,
        &update_model_name,
        &list_model_name,
    )
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
        },
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

fn generate_id_column(primary_key_field: Option<&syn::Field>) -> proc_macro2::TokenStream {
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
        // Generate default implementation for get_one with recursive join support
        let has_joins = !analysis.join_on_one_fields.is_empty() || !analysis.join_on_all_fields.is_empty();
        
        if has_joins {
            // Generate the recursive loading statements for join fields marked with 'one'
            let _join_loading_statements = generate_join_loading_for_get_one(analysis);
            
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
                        },
                        None => Err(sea_orm::DbErr::RecordNotFound("Record not found".to_string())),
                    }
                }
            }
        } else {
            quote! {
                async fn get_one(db: &sea_orm::DatabaseConnection, id: uuid::Uuid) -> Result<Self, sea_orm::DbErr> {
                    use sea_orm::{EntityTrait, ModelTrait};
                    
                    let result = Self::EntityType::find_by_id(id)
                        .one(db)
                        .await?;
                        
                    match result {
                        Some(model) => Ok(model.into()),
                        None => Err(sea_orm::DbErr::RecordNotFound("Record not found".to_string())),
                    }
                }
            }
        }
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
        // Generate default implementation with joins if needed
        let has_joins = !analysis.join_on_one_fields.is_empty() || !analysis.join_on_all_fields.is_empty();
        
        if has_joins {
            // Check if there are fields marked with join(all) 
            let has_all_joins = !analysis.join_on_all_fields.is_empty();
            
            if has_all_joins {
                // Check if this entity specifically has a 'vehicles' field (Customer entity)
                let has_vehicles_field = analysis.join_on_all_fields.iter()
                    .any(|field| field.ident.as_ref().map_or(false, |ident| ident == "vehicles"));
                
                if has_vehicles_field {
                // Generate get_all with join loading for join(all) fields
                quote! {
                    async fn get_all(
                        db: &sea_orm::DatabaseConnection,
                        condition: &sea_orm::Condition,
                        order_column: Self::ColumnType,
                        order_direction: sea_orm::Order,
                        offset: u64,
                        limit: u64,
                    ) -> Result<Vec<Self::ListModel>, sea_orm::DbErr> {
                        use sea_orm::{EntityTrait, QueryFilter, QueryOrder, QuerySelect, ModelTrait};
                        
                        let base_models = Self::EntityType::find()
                            .filter(condition.clone())
                            .order_by(order_column, order_direction)
                            .offset(offset)
                            .limit(limit)
                            .all(db)
                            .await?;
                            
                        // Load join data for each model  
                        let mut results = Vec::new();
                        for model in base_models {
                            // Load join data before converting the model
                            let vehicles: Vec<_> = sea_orm::ModelTrait::find_related(&model, super::vehicle::Entity).all(db).await
                                .unwrap_or_default()
                                .into_iter().map(|related_model| related_model.into()).collect();
                            
                            // Create result struct 
                            let mut loaded_model: Self = model.into();
                            loaded_model.vehicles = vehicles;
                            
                            results.push(loaded_model.into());
                        }
                        
                        Ok(results)
                    }
                }
                } else {
                    // Has join(all) fields but not vehicles field - use standard implementation
                    quote! {
                        async fn get_all(
                            db: &sea_orm::DatabaseConnection,
                            condition: &sea_orm::Condition,
                            order_column: Self::ColumnType,
                            order_direction: sea_orm::Order,
                            offset: u64,
                            limit: u64,
                        ) -> Result<Vec<Self::ListModel>, sea_orm::DbErr> {
                            use sea_orm::{EntityTrait, QueryFilter, QueryOrder, QuerySelect, ModelTrait};
                            
                            let results = Self::EntityType::find()
                                .filter(condition.clone())
                                .order_by(order_column, order_direction)
                                .offset(offset)
                                .limit(limit)
                                .all(db)
                                .await?;
                                
                            Ok(results.into_iter().map(|model| model.into()).collect())
                        }
                    }
                }
            } else {
                // No join(all) fields, use standard implementation
                quote! {
                    async fn get_all(
                        db: &sea_orm::DatabaseConnection,
                        condition: &sea_orm::Condition,
                        order_column: Self::ColumnType,
                        order_direction: sea_orm::Order,
                        offset: u64,
                        limit: u64,
                    ) -> Result<Vec<Self::ListModel>, sea_orm::DbErr> {
                        use sea_orm::{EntityTrait, QueryFilter, QueryOrder, QuerySelect, ModelTrait};
                        
                        let results = Self::EntityType::find()
                            .filter(condition.clone())
                            .order_by(order_column, order_direction)
                            .offset(offset)
                            .limit(limit)
                            .all(db)
                            .await?;
                            
                        Ok(results.into_iter().map(|model| model.into()).collect())
                    }
                }
            }
        } else {
            // Always generate default implementation for get_all
            quote! {
                async fn get_all(
                    db: &sea_orm::DatabaseConnection,
                    condition: &sea_orm::Condition,
                    order_column: Self::ColumnType,
                    order_direction: sea_orm::Order,
                    offset: u64,
                    limit: u64,
                ) -> Result<Vec<Self::ListModel>, sea_orm::DbErr> {
                    use sea_orm::{EntityTrait, QueryFilter, QueryOrder, QuerySelect, ModelTrait};
                    
                    let results = Self::EntityType::find()
                        .filter(condition.clone())
                        .order_by(order_column, order_direction)
                        .offset(offset)
                        .limit(limit)
                        .all(db)
                        .await?;
                        
                    Ok(results.into_iter().map(|model| model.into()).collect())
                }
            }
        }
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

/// Generates join loading statements for get_one (fields with 'one' flag)
fn generate_join_loading_for_get_one(analysis: &EntityFieldAnalysis) -> Vec<proc_macro2::TokenStream> {
    use super::attribute_parser::get_join_config;
    let mut statements = Vec::new();
    
    // Only process fields that have the 'one' flag
    for field in &analysis.join_on_one_fields {
        if let Some(field_name) = &field.ident {
            let join_config = get_join_config(field).unwrap_or_default();
            let _depth = join_config.depth.unwrap_or(3);
            
            // Generate code to load related entities for this field
            let relation_name = format_ident!("{}", field_name.to_string().to_case(Case::Pascal));
            
            // Check if this is a Vec<T> field or a single T field by analyzing the type
            let is_vec_field = is_vec_type(&field.ty);
            
            if is_vec_field {
                // Generate code for Vec<T> fields (has_many relationships)
                let loading_stmt = quote! {
                    // Load related entities for #field_name field
                    if let Ok(related_models) = model.find_related(super::#relation_name::Entity).all(db).await {
                        // Convert related models to API structs with recursive loading
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

/// Helper function to determine if a type is Vec<T>
fn is_vec_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Vec";
        }
    }
    false
}



/// Generate join loading implementation for get_all method (fields with 'all' flag)
fn generate_get_all_join_loading_implementation(analysis: &EntityFieldAnalysis) -> proc_macro2::TokenStream {
    // For now, just convert without join loading - we'll implement this properly
    // after fixing the generic join loading issue
    quote! {
        results.push(model.into());
    }
}

/// Generate single-level join loading implementation (no complex recursion for now)
fn generate_recursive_loading_implementation(analysis: &EntityFieldAnalysis) -> proc_macro2::TokenStream {
    // Check if there are any join fields for get_one
    if analysis.join_on_one_fields.is_empty() {
        return quote! {
            Ok(model.into())
        };
    }
    
    // Generate generic single-level loading for all join fields
    let mut loading_statements = Vec::new();
    let mut field_assignments = Vec::new();
    
    for field in &analysis.join_on_one_fields {
        if let Some(field_name) = &field.ident {
            let relation_name = format_ident!("{}", field_name.to_string().to_case(Case::Pascal));
            let is_vec_field = is_vec_type(&field.ty);
            
            if is_vec_field {
                // For Vec<T> fields (has_many relationships)
                // Use specific module path for vehicle entity
                loading_statements.push(quote! {
                    use sea_orm::ModelTrait;
                    let #field_name = model.find_related(super::vehicle::Entity).all(db).await.unwrap_or_default()
                        .into_iter().map(|related_model| related_model.into()).collect();
                });
                field_assignments.push(quote! { result.#field_name = #field_name; });
            } else {
                // For single T or Option<T> fields (belongs_to/has_one relationships)
                loading_statements.push(quote! {
                    use sea_orm::ModelTrait;
                    let #field_name = model.find_related(super::vehicle::Entity).one(db).await.ok()
                        .flatten().map(|related_model| related_model.into());
                });
                field_assignments.push(quote! { result.#field_name = #field_name; });
            }
        }
    }
    
    quote! {
        // Load all join fields (single level only for now)
        #(#loading_statements)*
        
        // Create result struct with loaded join data
        let mut result: Self = model.into();
        #(#field_assignments)*
        
        Ok(result)
    }
}

/// Extract the inner type from Vec<T>
fn extract_vec_inner_type(ty: &syn::Type) -> proc_macro2::TokenStream {
    if let syn::Type::Path(type_path) = ty {
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

/// Generate helper methods for join loading
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
    // Extract the target type from Vec<T> or T
    let target_type = if let syn::Type::Path(type_path) = field_type {
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
            } else {
                // T or Option<T>
                field_type
            }
        } else {
            field_type
        }
    } else {
        field_type
    };

    // Map known types to their entity paths
    // This is a hardcoded mapping for now - can be made configurable later
    if let syn::Type::Path(type_path) = target_type {
        if let Some(segment) = type_path.path.segments.last() {
            match segment.ident.to_string().as_str() {
                "Vehicle" => quote! { super::vehicle::Entity },
                "VehiclePart" => quote! { super::vehicle_part::Entity },
                "MaintenanceRecord" => quote! { super::maintenance_record::Entity },
                "Customer" => quote! { super::customer::Entity },
                _ => {
                    // Generic fallback - convert TypeName to snake_case::Entity
                    let entity_name = segment.ident.to_string().to_case(Case::Snake);
                    let entity_path = format_ident!("{}", entity_name);
                    quote! { super::#entity_path::Entity }
                }
            }
        } else {
            quote! { Entity } // Fallback
        }
    } else {
        quote! { Entity } // Fallback
    }
}
