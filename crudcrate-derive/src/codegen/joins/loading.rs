//! Consolidated join loading code generation
//!
//! This module provides shared logic for generating join loading code for both
//! `get_one()` and `get_all()` methods, eliminating the duplication between
//! handlers/get.rs and joins/recursion.rs
//!
//! ## Security Limits
//!
//! **Regular Joins - MAX_JOIN_DEPTH = 5**: Cross-model join recursion is capped at depth 5 to prevent:
//! - Infinite recursion with circular references
//! - Exponential query growth (N+1 problem)
//! - Database connection pool exhaustion
//!
//! **Self-Referencing Joins - Depth = 1 Only**: Self-referencing fields (e.g., `Category { children: Vec<Category> }`)
//! are automatically limited to depth=1 to prevent exponential query growth. This means self-referencing fields
//! will load immediate children only, without recursive nesting. Depths > 1 will trigger a compile-time warning.
//!
//! **To use deeper joins**:
//! - Explicitly set `depth` parameter: `#[crudcrate(join(all, depth = 3))]`
//! - Regular joins (cross-model): Maximum 5 (values > 5 are capped to 5)
//! - Self-referencing: Always 1 (values > 1 trigger warning and are set to 1)
//! - Unspecified depth defaults to 5 for regular joins, 1 for self-referencing
//!
//! **Example**:
//! ```ignore
//! // Regular joins (different models)
//! #[crudcrate(join(all, depth = 1))]  // Shallow: load related entities only
//! pub users: Vec<User>
//!
//! #[crudcrate(join(all, depth = 3))]  // Medium: 3 levels deep
//! pub organization: Option<Organization>
//!
//! #[crudcrate(join(all))]  // Defaults to depth = 5 (maximum)
//! pub vehicles: Vec<Vehicle>
//!
//! // Self-referencing joins (same model) - always depth=1 only
//! #[crudcrate(join(all))]  // Loads immediate children only
//! pub children: Vec<Category>
//!
//! #[crudcrate(join(all, depth = 5))]  // WARNING: Ignored, self-references always use depth=1
//! pub subcategories: Vec<Category>
//! ```

// Security: Maximum join depth to prevent infinite recursion and resource exhaustion
// Users cannot exceed this limit - values > 5 are automatically capped
const MAX_JOIN_DEPTH: u8 = 5;

use crate::codegen::joins::get_join_config;
use crate::codegen::type_resolution::{
    extract_api_struct_type_for_recursive_call, extract_option_or_direct_inner_type,
    get_path_from_field_type, is_vec_type,
};
use crate::traits::crudresource::structs::EntityFieldAnalysis;
use quote::quote;

/// Generate join loading code for `get_one()` method
///
/// Returns code that evaluates to `Self` (not wrapped in Result).
/// The caller is responsible for wrapping in `Ok()`.
pub fn generate_get_one_join_loading(
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    // Check if there are any join fields
    if analysis.join_on_one_fields.is_empty() && analysis.join_on_all_fields.is_empty() {
        return quote! { Self::from(model) };
    }

    // Deduplicate fields (some may have both join(one) and join(all))
    let mut seen_fields = std::collections::HashSet::new();
    let mut join_fields: Vec<&syn::Field> = Vec::new();

    for field in analysis.join_on_one_fields.iter().chain(analysis.join_on_all_fields.iter()) {
        if field.ident.as_ref().is_none_or(|name| seen_fields.insert(name.to_string())) {
            join_fields.push(field);
        }
    }

    generate_join_loading_impl(&join_fields, "get_one", api_struct_name)
}

/// Generate batch loading code for `get_all()` method
///
/// This generates optimized batch loading that reduces N+1 queries to 2 queries:
/// 1. One query to fetch N parent entities (already done before this code runs)
/// 2. One query per join field to fetch ALL related entities for ALL parents
///
/// Returns a tuple of (pre_loop_code, in_loop_code):
/// - `pre_loop_code`: Batch loads all related entities and groups them by parent ID
/// - `in_loop_code`: Looks up pre-loaded data from HashMaps (no queries)
pub fn generate_get_all_batch_loading(
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    if analysis.join_on_all_fields.is_empty() {
        return (quote! {}, quote! { Self::from(model) });
    }

    // Extract PK field ident (fallback to `id` for backward compat)
    let pk_ident = analysis.primary_key_field
        .and_then(|f| f.ident.as_ref())
        .cloned()
        .unwrap_or_else(|| quote::format_ident!("id"));

    let join_fields: Vec<&syn::Field> = analysis.join_on_all_fields.clone();
    generate_batch_loading_impl(&join_fields, api_struct_name, &pk_ident)
}


/// Generate optimized batch loading code for get_all()
///
/// Returns (pre_loop_code, in_loop_code) where:
/// - pre_loop_code runs ONCE before the loop to batch load all related entities
/// - in_loop_code runs for each model to assign pre-loaded data
fn generate_batch_loading_impl(
    join_fields: &[&syn::Field],
    api_struct_name: &syn::Ident,
    pk_ident: &syn::Ident,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let mut batch_loading_statements = Vec::new();
    let mut field_assignments = Vec::new();

    for field in join_fields {
        let Some(field_name) = &field.ident else {
            continue;
        };

        let join_config = get_join_config(field).unwrap_or_default();
        let is_vec_field = is_vec_type(&field.ty);

        // Check if this is a self-referencing field
        let inner_type = extract_api_struct_type_for_recursive_call(&field.ty);
        let inner_type_string = inner_type.to_string();
        let api_struct_name_string = api_struct_name.to_string();
        let is_self_referencing = inner_type_string.trim() == api_struct_name_string.trim();

        // Security: Cap depth
        let effective_depth = if is_self_referencing {
            let original_depth = join_config.depth.unwrap_or(1).min(MAX_JOIN_DEPTH);
            if original_depth > 1 {
                let error_msg = format!(
                    "Self-referencing field '{}' in struct '{}' has depth={}, but self-references only support depth=1",
                    field_name, api_struct_name, original_depth
                );
                return (quote! { compile_error!(#error_msg); }, quote! {});
            }
            1
        } else {
            join_config.depth.unwrap_or(MAX_JOIN_DEPTH).min(MAX_JOIN_DEPTH)
        };

        let depth_limited = effective_depth == 1;

        // Get entity and model paths
        let (entity_path, model_path) = if let Some(custom_path) = &join_config.path {
            match custom_path.parse::<proc_macro2::TokenStream>() {
                Ok(path_tokens) => (
                    quote! { #path_tokens::Entity },
                    quote! { #path_tokens::Model },
                ),
                Err(_) => {
                    let error_msg = format!(
                        "Invalid join path '{}' for field '{}'",
                        custom_path, field_name
                    );
                    return (quote! { compile_error!(#error_msg); }, quote! {});
                }
            }
        } else {
            (
                get_path_from_field_type(&field.ty, "Entity"),
                get_path_from_field_type(&field.ty, "Model"),
            )
        };

        // Get the Column path for the FK column
        let column_path = get_path_from_field_type(&field.ty, "Column");

        // Derive FK column name from parent struct name
        // Convention: Customer -> Column::CustomerId (PascalCase) and field.customer_id (snake_case)
        let fk_column_pascal = quote::format_ident!("{}Id", api_struct_name);
        let fk_field_snake = quote::format_ident!("{}_id", to_snake_case(&api_struct_name.to_string()));

        // HashMap variable for storing batch-loaded data
        let map_var = quote::format_ident!("{}_by_parent", field_name);

        if is_vec_field {
            let api_struct_type = extract_api_struct_type_for_recursive_call(&field.ty);

            if depth_limited {
                // Depth=1: Simple batch load without recursion
                if is_self_referencing {
                    // Self-referencing: use ParentId column
                    batch_loading_statements.push(quote! {
                        let mut #map_var: std::collections::HashMap<uuid::Uuid, Vec<#api_struct_type>> = {
                            use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                            let all_related = #entity_path::find()
                                .filter(#column_path::ParentId.is_in(parent_ids.clone()))
                                .all(db)
                                .await
                                .unwrap_or_else(|e| {
                                    tracing::warn!(error = %e, "Failed to batch load self-referencing children");
                                    Vec::new()
                                });

                            let mut map: std::collections::HashMap<uuid::Uuid, Vec<#api_struct_type>> =
                                std::collections::HashMap::new();
                            for related_model in all_related {
                                if let Some(parent_id) = related_model.parent_id {
                                    map.entry(parent_id)
                                        .or_insert_with(Vec::new)
                                        .push(#api_struct_type::from(related_model));
                                }
                            }
                            map
                        };
                    });
                } else {
                    // Regular join: use derived FK column name
                    // Column enum uses PascalCase (CustomerId), field uses snake_case (customer_id)
                    batch_loading_statements.push(quote! {
                        let mut #map_var: std::collections::HashMap<uuid::Uuid, Vec<#api_struct_type>> = {
                            use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                            let all_related = #entity_path::find()
                                .filter(#column_path::#fk_column_pascal.is_in(parent_ids.clone()))
                                .all(db)
                                .await?;

                            let mut map: std::collections::HashMap<uuid::Uuid, Vec<#api_struct_type>> =
                                std::collections::HashMap::new();
                            for related_model in all_related {
                                let fk_value = related_model.#fk_field_snake;
                                map.entry(fk_value)
                                    .or_insert_with(Vec::new)
                                    .push(#api_struct_type::from(related_model));
                            }
                            map
                        };
                    });
                }

                field_assignments.push(quote! {
                    item.#field_name = #map_var.remove(&parent_id).unwrap_or_default();
                });
            } else {
                // Depth > 1: Need recursive loading via get_one()
                // For batch loading with depth > 1, we batch load the immediate children,
                // then for each child we call get_one() to load its nested relations
                // Note: Self-referencing fields are always depth=1, so this branch is
                // only reached for cross-model joins.
                // Batch load immediate children, then recursively load their nested relations
                batch_loading_statements.push(quote! {
                        let mut #map_var: std::collections::HashMap<uuid::Uuid, Vec<#api_struct_type>> = {
                            use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                            let all_related_models: Vec<#model_path> = #entity_path::find()
                                .filter(#column_path::#fk_column_pascal.is_in(parent_ids.clone()))
                                .all(db)
                                .await?;

                            let mut map: std::collections::HashMap<uuid::Uuid, Vec<#api_struct_type>> =
                                std::collections::HashMap::new();

                            // For each related model, call get_one() to load its nested relations
                            for related_model in all_related_models {
                                let fk_value = related_model.#fk_field_snake;
                                let entity = match #api_struct_type::get_one(db, related_model.id).await {
                                    Ok(e) => e,
                                    Err(e) => {
                                        tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                        #api_struct_type::from(related_model)
                                    }
                                };
                                map.entry(fk_value)
                                    .or_insert_with(Vec::new)
                                    .push(entity);
                            }
                            map
                        };
                    });

                field_assignments.push(quote! {
                    item.#field_name = #map_var.remove(&parent_id).unwrap_or_default();
                });
            }
        } else {
            // Option<T> relationships (belongs_to/has_one)
            // These are typically 1:1 and benefit less from batch loading,
            // but we can still optimize if there are many parents
            let target_type = extract_option_or_direct_inner_type(&field.ty);

            if depth_limited {
                batch_loading_statements.push(quote! {
                    let mut #map_var: std::collections::HashMap<uuid::Uuid, #target_type> = {
                        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                        let all_related = #entity_path::find()
                            .filter(#column_path::#fk_column_pascal.is_in(parent_ids.clone()))
                            .all(db)
                            .await?;

                        let mut map: std::collections::HashMap<uuid::Uuid, #target_type> =
                            std::collections::HashMap::new();
                        for related_model in all_related {
                            let fk_value = related_model.#fk_field_snake;
                            map.insert(fk_value, #target_type::from(related_model));
                        }
                        map
                    };
                });
            } else {
                batch_loading_statements.push(quote! {
                    let mut #map_var: std::collections::HashMap<uuid::Uuid, #target_type> = {
                        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                        let all_related_models: Vec<#model_path> = #entity_path::find()
                            .filter(#column_path::#fk_column_pascal.is_in(parent_ids.clone()))
                            .all(db)
                            .await?;

                        let mut map: std::collections::HashMap<uuid::Uuid, #target_type> =
                            std::collections::HashMap::new();
                        for related_model in all_related_models {
                            let fk_value = related_model.#fk_field_snake;
                            let entity = match #target_type::get_one(db, related_model.id).await {
                                Ok(e) => e,
                                Err(e) => {
                                    tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                    #target_type::from(related_model)
                                }
                            };
                            map.insert(fk_value, entity);
                        }
                        map
                    };
                });
            }

            field_assignments.push(quote! {
                item.#field_name = #map_var.remove(&parent_id);
            });
        }
    }

    let pre_loop_code = quote! {
        // Collect all parent IDs for batch loading
        let parent_ids: Vec<uuid::Uuid> = models.iter().map(|m| m.#pk_ident).collect();

        #( #batch_loading_statements )*
    };

    let in_loop_code = quote! {
        let parent_id = model.#pk_ident;
        let mut item = Self::from(model);
        #( #field_assignments )*
        item
    };

    (pre_loop_code, in_loop_code)
}

/// Convert PascalCase to snake_case
fn to_snake_case(s: &str) -> String {
    use convert_case::{Case, Casing};
    s.to_case(Case::Snake)
}

/// Shared implementation for generating join loading code
fn generate_join_loading_impl(
    join_fields: &[&syn::Field],
    _context: &str,
    api_struct_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    let mut loading_statements = Vec::new();
    let mut field_assignments = Vec::new();

    for field in join_fields {
        let Some(field_name) = &field.ident else {
            continue;
        };

        let join_config = get_join_config(field).unwrap_or_default();
        let is_vec_field = is_vec_type(&field.ty);

        // Check if this is a self-referencing field (e.g., Category { children: Vec<Category> })
        // Extract the inner type from Vec<T> or Option<T> and check if it matches the API struct name
        let inner_type = extract_api_struct_type_for_recursive_call(&field.ty);
        let inner_type_string = inner_type.to_string();
        let api_struct_name_string = api_struct_name.to_string();
        // Check for exact match (not substring) to avoid false positives like VehiclePart matching Vehicle
        let is_self_referencing = inner_type_string.trim() == api_struct_name_string.trim();

        // Security: Cap depth to prevent infinite recursion and performance issues
        // - Regular joins: Max depth 5 (MAX_JOIN_DEPTH)
        // - Self-referencing: MUST use depth=1 (load immediate children only, no recursion)
        let effective_depth = if is_self_referencing {
            let original_depth = join_config.depth.unwrap_or(1).min(MAX_JOIN_DEPTH);
            if original_depth > 1 {
                let error_msg = format!(
                    "Self-referencing field '{}' in struct '{}' has depth={}, but self-references only support depth=1 to prevent exponential query growth. Please change to: join(one, depth = 1)",
                    field_name, api_struct_name, original_depth
                );
                return quote! { compile_error!(#error_msg); };
            }
            1  // Always use depth=1 for self-referencing (no recursive loading)
        } else {
            join_config.depth.unwrap_or(MAX_JOIN_DEPTH).min(MAX_JOIN_DEPTH)
        };

        // For self-referencing fields, we use Entity::find().filter() instead of find_related()
        // Self-referencing fields are ALWAYS depth-limited (depth=1) to prevent exponential growth
        // Regular fields use recursive loading when depth > 1
        let depth_limited = effective_depth == 1;

        // Get entity path and model path (custom or derived from type)
        let (entity_path, model_path) = if let Some(custom_path) = &join_config.path {
            match custom_path.parse::<proc_macro2::TokenStream>() {
                Ok(path_tokens) => (
                    quote! { #path_tokens::Entity },
                    quote! { #path_tokens::Model },
                ),
                Err(_) => {
                    // Generate a compile error if the path is invalid
                    let error_msg = format!(
                        "Invalid join path '{}' for field '{}'. Expected a valid Rust module path.",
                        custom_path, field_name
                    );
                    return quote! { compile_error!(#error_msg); };
                }
            }
        } else {
            (
                get_path_from_field_type(&field.ty, "Entity"),
                get_path_from_field_type(&field.ty, "Model"),
            )
        };

        if is_vec_field {
            // Vec<T> relationships (has_many)
            let api_struct_type = extract_api_struct_type_for_recursive_call(&field.ty);

            if depth_limited {
                // Depth=1: Load data, no recursion
                // Use explicit From::<Model>::from() to avoid trait ambiguity with self-referencing types
                let loaded_var = quote::format_ident!("loaded_{}", field_name);

                // WORKAROUND: model.find_related(Entity) for self-referencing relationships
                // causes stack overflow in SeaORM. Instead, use Entity::find().filter() directly.
                //
                // For self-referencing relationships, we need to extract the foreign key column
                // from the SeaORM Relation definition. For now, we use a convention-based approach:
                // - Check common FK column names: ParentId, parent_id, etc.
                // - Fall back to manual filtering if needed
                if is_self_referencing {
                    // Try to find the FK column by checking the SeaORM relation
                    // For Category: has_many from=Id to=ParentId, so FK is ParentId
                    // We'll try common patterns and let it compile-fail if wrong
                    let column_path = get_path_from_field_type(&field.ty, "Column");

                    loading_statements.push(quote! {
                        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                        // Try to load using Entity::find().filter() instead of find_related()
                        // This avoids the stack overflow issue with Related<Entity> for Entity
                        let related_models = {
                            // Try common FK column names for self-referencing relationships
                            // Most common: ParentId (for tree structures)
                            use sea_orm::sea_query::IntoCondition;

                            // Build a condition that checks for matching foreign key
                            // For Category: WHERE parent_id = model.id
                            let condition = #column_path::ParentId.eq(model.id).into_condition();

                            #entity_path::find()
                                .filter(condition)
                                .all(db)
                                .await
                                .unwrap_or_else(|e| {
                                    tracing::warn!(error = %e, "Failed to load self-referencing children");
                                    Vec::new()
                                })
                        };

                        let #loaded_var: Vec<#api_struct_type> = related_models
                            .into_iter()
                            .map(|m: #model_path| #api_struct_type::from(m))
                            .collect();
                    });
                } else {
                    loading_statements.push(quote! {
                        let related_models = model.find_related(#entity_path).all(db).await?;
                        let #loaded_var: Vec<#api_struct_type> = related_models
                            .into_iter()
                            .map(|m: #model_path| #api_struct_type::from(m))
                            .collect();
                    });
                }
                field_assignments.push(quote! { result.#field_name = #loaded_var; });
            } else {
                // Unlimited depth: Recursive loading via get_one()
                if is_self_referencing {
                    // For self-referencing, use Entity::find().filter() to avoid stack overflow
                    let column_path = get_path_from_field_type(&field.ty, "Column");
                    loading_statements.push(quote! {
                        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                        // Load related models using filter instead of find_related
                        let related_models = {
                            use sea_orm::sea_query::IntoCondition;
                            let condition = #column_path::ParentId.eq(model.id).into_condition();
                            #entity_path::find()
                                .filter(condition)
                                .all(db)
                                .await
                                .unwrap_or_else(|e| {
                                    tracing::warn!(error = %e, "Failed to load self-referencing children");
                                    Vec::new()
                                })
                        };

                        let mut #field_name = Vec::new();
                        for related_model in related_models {
                            match #api_struct_type::get_one(db, related_model.id).await {
                                Ok(entity) => #field_name.push(entity),
                                Err(e) => {
                                    tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                    #field_name.push(related_model.into());
                                }
                            }
                        }
                    });
                } else {
                    loading_statements.push(quote! {
                        let related_models = model.find_related(#entity_path).all(db).await?;
                        let mut #field_name = Vec::new();
                        for related_model in related_models {
                            match #api_struct_type::get_one(db, related_model.id).await {
                                Ok(entity) => #field_name.push(entity),
                                Err(e) => {
                                    tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                    #field_name.push(related_model.into());
                                }
                            }
                        }
                    });
                }
                field_assignments.push(quote! { result.#field_name = #field_name; });
            }
        } else {
            // Option<T> or T relationships (belongs_to/has_one)
            let target_type = extract_option_or_direct_inner_type(&field.ty);

            if depth_limited {
                // Depth=1: Load data, no recursion
                // Use explicit From::<Model>::from() to avoid trait ambiguity with self-referencing types
                let loaded_var = quote::format_ident!("loaded_{}", field_name);
                loading_statements.push(quote! {
                    let #loaded_var = model
                        .find_related(#entity_path)
                        .one(db)
                        .await?
                        .map(|m: #model_path| #target_type::from(m));
                });
                field_assignments.push(quote! {
                    result.#field_name = #loaded_var;
                });
            } else {
                // Unlimited depth: Recursive loading via get_one()
                loading_statements.push(quote! {
                    let #field_name = match model.find_related(#entity_path).one(db).await? {
                        Some(related_model) => {
                            match #target_type::get_one(db, related_model.id).await {
                                Ok(entity) => Some(entity),
                                Err(e) => {
                                    tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                    Some(related_model.into())
                                }
                            }
                        }
                        None => None,
                    };
                });
                field_assignments.push(quote! {
                    result.#field_name = #field_name;
                });
            }
        }
    }

    // Both contexts return Self directly (not wrapped in Result)
    // The caller is responsible for wrapping in Ok() when needed
    quote! {
        #( #loading_statements )*
        let mut result: Self = model.into();
        #( #field_assignments )*
        result
    }
}
