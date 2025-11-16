use crate::codegen::join_strategies::get_join_config;
use crate::codegen::join_strategies::recursion::generate_recursive_loading_implementation;
use crate::codegen::type_resolution::{
    extract_api_struct_type_for_recursive_call, extract_option_or_direct_inner_type,
    extract_vec_inner_type, get_path_from_field_type, is_vec_type,
};
use crate::traits::crudresource::structs::{CRUDResourceMeta, EntityFieldAnalysis};
use quote::quote;

/// Generate join loading logic for `get_all` method
#[allow(clippy::too_many_lines)]
pub fn generate_get_all_join_loading(analysis: &EntityFieldAnalysis) -> proc_macro2::TokenStream {
    let mut loading_statements = Vec::new();
    let mut field_assignments = Vec::new();

    for field in &analysis.join_on_all_fields {
        if let Some(field_name) = &field.ident {
            let join_config = get_join_config(field).unwrap_or_default();
            let is_vec_field = is_vec_type(&field.ty);
            // Check if this join should stop recursion at this level
            let stop_recursion = join_config.depth == Some(1);

            // Extract entity and model paths from the field type or use custom path
            let entity_path = if let Some(custom_path) = &join_config.path {
                // Parse custom path string into a token stream
                let path_tokens: proc_macro2::TokenStream = custom_path.parse().unwrap();
                quote! { #path_tokens::Entity }
            } else {
                get_path_from_field_type(&field.ty, "Entity")
            };
            let _model_path = get_path_from_field_type(&field.ty, "Model");

            if is_vec_field {
                // No complex type resolution needed - extract directly from Vec<T>
                let _target_type = syn::parse2::<syn::Type>(extract_vec_inner_type(&field.ty))
                    .unwrap_or_else(|_| field.ty.clone());

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

/// Generate `get_all` method implementation
pub fn generate_get_all_impl(
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

/// Generate `get_one` method implementation
pub fn generate_get_one_impl(
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
