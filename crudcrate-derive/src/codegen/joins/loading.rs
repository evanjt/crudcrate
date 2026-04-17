//! Consolidated join loading code generation
//!
//! This module provides shared logic for generating join loading code for both
//! `get_one()` and `get_all()` methods, eliminating the duplication between
//! handlers/get.rs and joins/recursion.rs
//!
//! ## Security Limits
//!
//! **Regular Joins - `MAX_JOIN_DEPTH` = 5**: Cross-model join recursion is capped at depth 5 to prevent:
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
    generate_get_one_join_loading_inner(analysis, api_struct_name, false)
}

/// Generate join loading code for `get_one_scoped()` — applies child entity
/// scope conditions to Vec join queries at the SQL level.
pub fn generate_get_one_scoped_join_loading(
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    generate_get_one_join_loading_inner(analysis, api_struct_name, true)
}

fn generate_get_one_join_loading_inner(
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
    scoped: bool,
) -> proc_macro2::TokenStream {
    // Check if there are any join fields
    if analysis.join_on_one_fields.is_empty() && analysis.join_on_all_fields.is_empty() {
        return quote! { Self::from(model) };
    }

    // Deduplicate fields (some may have both join(one) and join(all))
    let mut seen_fields = std::collections::HashSet::new();
    let mut join_fields: Vec<&syn::Field> = Vec::new();

    for field in analysis
        .join_on_one_fields
        .iter()
        .chain(analysis.join_on_all_fields.iter())
    {
        if field
            .ident
            .as_ref()
            .is_none_or(|name| seen_fields.insert(name.to_string()))
        {
            join_fields.push(field);
        }
    }

    generate_join_loading_impl(&join_fields, "get_one", api_struct_name, scoped)
}

/// Generate batch loading code for `get_all()` method
///
/// This generates optimized batch loading that reduces N+1 queries to 2 queries:
/// 1. One query to fetch N parent entities (already done before this code runs)
/// 2. One query per join field to fetch ALL related entities for ALL parents
///
/// Returns a tuple of (`pre_loop_code`, `in_loop_code)`:
/// - `pre_loop_code`: Batch loads all related entities and groups them by parent ID
/// - `in_loop_code`: Looks up pre-loaded data from `HashMaps` (no queries)
pub fn generate_get_all_batch_loading(
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    if analysis.join_on_all_fields.is_empty() {
        return (quote! {}, quote! { Self::from(model) });
    }

    // Extract PK field ident (fallback to `id` for backward compat)
    let pk_ident = analysis
        .primary_key_field
        .and_then(|f| f.ident.as_ref())
        .cloned()
        .unwrap_or_else(|| quote::format_ident!("id"));

    let join_fields: Vec<&syn::Field> = analysis.join_on_all_fields.clone();
    generate_batch_loading_impl(&join_fields, api_struct_name, &pk_ident, false)
}

/// Generate batch loading code for `get_all_scoped()` — applies child entity
/// scope conditions to Vec child batch queries at the SQL level, and recurses
/// via `get_one_scoped` for depth > 1 so grandchildren are also filtered.
pub fn generate_get_all_scoped_batch_loading(
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    if analysis.join_on_all_fields.is_empty() {
        return (quote! {}, quote! { Self::from(model) });
    }

    let pk_ident = analysis
        .primary_key_field
        .and_then(|f| f.ident.as_ref())
        .cloned()
        .unwrap_or_else(|| quote::format_ident!("id"));

    let join_fields: Vec<&syn::Field> = analysis.join_on_all_fields.clone();
    generate_batch_loading_impl(&join_fields, api_struct_name, &pk_ident, true)
}

/// Generate optimized batch loading code for `get_all()`
///
/// Returns (`pre_loop_code`, `in_loop_code`) where:
/// - `pre_loop_code` runs ONCE before the loop to batch load all related entities
/// - `in_loop_code` runs for each model to assign pre-loaded data
///
/// When `scoped` is true, Vec<T> child batch queries include the child entity's
/// `ScopeFilterable::scope_condition()` at the SQL level, and depth > 1 recursion
/// uses `get_one_scoped` so that grandchildren are filtered too.
fn generate_batch_loading_impl(
    join_fields: &[&syn::Field],
    api_struct_name: &syn::Ident,
    pk_ident: &syn::Ident,
    scoped: bool,
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
                    "Self-referencing field '{field_name}' in struct '{api_struct_name}' has depth={original_depth}, but self-references only support depth=1"
                );
                return (quote! { compile_error!(#error_msg); }, quote! {});
            }
            1
        } else {
            join_config
                .depth
                .unwrap_or(MAX_JOIN_DEPTH)
                .min(MAX_JOIN_DEPTH)
        };

        let depth_limited = effective_depth == 1;

        // Get entity and model paths
        let (entity_path, model_path) = if let Some(custom_path) = &join_config.path {
            if let Ok(path_tokens) = custom_path.parse::<proc_macro2::TokenStream>() {
                (
                    quote! { #path_tokens::Entity },
                    quote! { #path_tokens::Model },
                )
            } else {
                let error_msg =
                    format!("Invalid join path '{custom_path}' for field '{field_name}'");
                return (quote! { compile_error!(#error_msg); }, quote! {});
            }
        } else {
            (
                get_path_from_field_type(&field.ty, "Entity"),
                get_path_from_field_type(&field.ty, "Model"),
            )
        };

        // Get the Column path for the FK column
        let column_path = get_path_from_field_type(&field.ty, "Column");

        // Derive FK column identifiers (respects fk_column override, self-ref, or convention)
        let (fk_column_pascal, fk_field_snake) =
            derive_fk_idents(&join_config, api_struct_name, is_self_referencing);

        // HashMap variable for storing batch-loaded data
        let map_var = quote::format_ident!("{}_by_parent", field_name);

        // When scoped, compute the child's ScopeFilterable::scope_condition()
        // once per batch query and apply it to both the SQL-level filter and
        // the depth > 1 recursive fetch.
        let scope_filter_for_vec = if scoped && is_vec_field {
            let inner_type = extract_api_struct_type_for_recursive_call(&field.ty);
            let inner_type_str = inner_type.to_string();
            let struct_name = inner_type_str
                .split("::")
                .last()
                .unwrap_or(&inner_type_str)
                .trim();
            let list_suffix = format!("{struct_name}List");
            let child_list_type = get_path_from_field_type(&field.ty, &list_suffix);
            quote! {
                let __child_scope: Option<sea_orm::Condition> =
                    <#child_list_type as crudcrate::ScopeFilterable>::scope_condition();
                let query = if let Some(ref cs) = __child_scope {
                    query.filter(cs.clone())
                } else {
                    query
                };
            }
        } else {
            quote! {}
        };

        if is_vec_field {
            let api_struct_type = extract_api_struct_type_for_recursive_call(&field.ty);

            if depth_limited {
                // Depth=1: Simple batch load without recursion
                // Each batch load is Box::pin'd to move its future to the heap,
                // preventing async state machine bloat when multiple joins accumulate.
                if is_self_referencing {
                    // Self-referencing: FK column derived via derive_fk_idents
                    batch_loading_statements.push(quote! {
                        let mut #map_var: std::collections::HashMap<uuid::Uuid, Vec<#api_struct_type>> = Box::pin(async {
                            use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                            let query = #entity_path::find()
                                .filter(#column_path::#fk_column_pascal.is_in(parent_ids.clone()));
                            #scope_filter_for_vec
                            let all_related = query.all(db).await?;

                            let mut map: std::collections::HashMap<uuid::Uuid, Vec<#api_struct_type>> =
                                std::collections::HashMap::new();
                            for related_model in all_related {
                                if let Some(parent_id) = related_model.#fk_field_snake {
                                    map.entry(parent_id)
                                        .or_insert_with(Vec::new)
                                        .push(#api_struct_type::from(related_model));
                                }
                            }
                            Ok::<_, crudcrate::ApiError>(map)
                        }).await?;
                    });
                } else {
                    // Regular join: use derived FK column name
                    // Column enum uses PascalCase (CustomerId), field uses snake_case (customer_id)
                    batch_loading_statements.push(quote! {
                        let mut #map_var: std::collections::HashMap<uuid::Uuid, Vec<#api_struct_type>> = Box::pin(async {
                            use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                            let query = #entity_path::find()
                                .filter(#column_path::#fk_column_pascal.is_in(parent_ids.clone()));
                            #scope_filter_for_vec
                            let all_related = query.all(db).await?;

                            let mut map: std::collections::HashMap<uuid::Uuid, Vec<#api_struct_type>> =
                                std::collections::HashMap::new();
                            for related_model in all_related {
                                let fk_value = related_model.#fk_field_snake;
                                map.entry(fk_value)
                                    .or_insert_with(Vec::new)
                                    .push(#api_struct_type::from(related_model));
                            }
                            Ok::<_, crudcrate::ApiError>(map)
                        }).await?;
                    });
                }

                field_assignments.push(quote! {
                    item.#field_name = #map_var.remove(&parent_id).unwrap_or_default();
                });
            } else {
                // Depth > 1: Need recursive loading via get_one() / get_one_scoped()
                // When scoped, recurse via get_one_scoped so grandchildren are also
                // filtered by the child's scope_condition. Note: Self-referencing
                // fields are always depth=1, so this branch is only reached for
                // cross-model joins.
                let recursive_fetch = if scoped {
                    quote! {
                        let entity = match __child_scope.as_ref() {
                            Some(cs) => match #api_struct_type::get_one_scoped(db, related_model.id, cs).await {
                                Ok(e) => e,
                                Err(e) => {
                                    tracing::warn!(error = %e, "Failed to load nested scoped relations, using flat model");
                                    #api_struct_type::from(related_model)
                                }
                            },
                            None => match #api_struct_type::get_one(db, related_model.id).await {
                                Ok(e) => e,
                                Err(e) => {
                                    tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                    #api_struct_type::from(related_model)
                                }
                            },
                        };
                    }
                } else {
                    quote! {
                        let entity = match #api_struct_type::get_one(db, related_model.id).await {
                            Ok(e) => e,
                            Err(e) => {
                                tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                #api_struct_type::from(related_model)
                            }
                        };
                    }
                };

                batch_loading_statements.push(quote! {
                        let mut #map_var: std::collections::HashMap<uuid::Uuid, Vec<#api_struct_type>> = Box::pin(async {
                            use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                            let query = #entity_path::find()
                                .filter(#column_path::#fk_column_pascal.is_in(parent_ids.clone()));
                            #scope_filter_for_vec
                            let all_related_models: Vec<#model_path> = query.all(db).await?;

                            let mut map: std::collections::HashMap<uuid::Uuid, Vec<#api_struct_type>> =
                                std::collections::HashMap::new();

                            // For each related model, call get_one() / get_one_scoped() to load nested relations
                            for related_model in all_related_models {
                                let fk_value = related_model.#fk_field_snake;
                                #recursive_fetch
                                map.entry(fk_value)
                                    .or_insert_with(Vec::new)
                                    .push(entity);
                            }
                            Ok::<_, crudcrate::ApiError>(map)
                        }).await?;
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
                    let mut #map_var: std::collections::HashMap<uuid::Uuid, #target_type> = Box::pin(async {
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
                        Ok::<_, crudcrate::ApiError>(map)
                    }).await?;
                });
            } else {
                batch_loading_statements.push(quote! {
                    let mut #map_var: std::collections::HashMap<uuid::Uuid, #target_type> = Box::pin(async {
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
                        Ok::<_, crudcrate::ApiError>(map)
                    }).await?;
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

/// Convert `PascalCase` to `snake_case`
fn to_snake_case(s: &str) -> String {
    use convert_case::{Case, Casing};
    s.to_case(Case::Snake)
}

/// Derive FK column identifiers for a join field.
///
/// Returns `(pascal_ident, snake_ident)` — e.g., `(CustomerId, customer_id)`.
///
/// Resolution order:
/// 1. Explicit `fk_column = "..."` from join config (highest priority)
/// 2. Self-referencing: `ParentId` / `parent_id`
/// 3. Convention: `{ParentStructName}Id` / `{parent_struct_name}_id`
fn derive_fk_idents(
    join_config: &crate::codegen::joins::config::JoinConfig,
    api_struct_name: &syn::Ident,
    is_self_referencing: bool,
) -> (proc_macro2::Ident, proc_macro2::Ident) {
    if let Some(ref fk) = join_config.fk_column {
        // Explicit override: use as-is for PascalCase, derive snake_case
        (
            quote::format_ident!("{}", fk),
            quote::format_ident!("{}", to_snake_case(fk)),
        )
    } else if is_self_referencing {
        (
            quote::format_ident!("ParentId"),
            quote::format_ident!("parent_id"),
        )
    } else {
        (
            quote::format_ident!("{}Id", api_struct_name),
            quote::format_ident!("{}_id", to_snake_case(&api_struct_name.to_string())),
        )
    }
}

/// Shared implementation for generating join loading code
///
/// When `scoped` is true, Vec<T> join queries include the child entity's
/// `ScopeFilterable::scope_condition()` as an additional WHERE clause,
/// filtering private children at the SQL level.
fn generate_join_loading_impl(
    join_fields: &[&syn::Field],
    _context: &str,
    api_struct_name: &syn::Ident,
    scoped: bool,
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
                    "Self-referencing field '{field_name}' in struct '{api_struct_name}' has depth={original_depth}, but self-references only support depth=1 to prevent exponential query growth. Please change to: join(one, depth = 1)"
                );
                return quote! { compile_error!(#error_msg); };
            }
            1 // Always use depth=1 for self-referencing (no recursive loading)
        } else {
            join_config
                .depth
                .unwrap_or(MAX_JOIN_DEPTH)
                .min(MAX_JOIN_DEPTH)
        };

        // For self-referencing fields, we use Entity::find().filter() instead of find_related()
        // Self-referencing fields are ALWAYS depth-limited (depth=1) to prevent exponential growth
        // Regular fields use recursive loading when depth > 1
        let depth_limited = effective_depth == 1;

        // Get entity path and model path (custom or derived from type)
        let (entity_path, model_path) = if let Some(custom_path) = &join_config.path {
            if let Ok(path_tokens) = custom_path.parse::<proc_macro2::TokenStream>() {
                (
                    quote! { #path_tokens::Entity },
                    quote! { #path_tokens::Model },
                )
            } else {
                // Generate a compile error if the path is invalid
                let error_msg = format!(
                    "Invalid join path '{custom_path}' for field '{field_name}'. Expected a valid Rust module path."
                );
                return quote! { compile_error!(#error_msg); };
            }
        } else {
            (
                get_path_from_field_type(&field.ty, "Entity"),
                get_path_from_field_type(&field.ty, "Model"),
            )
        };

        // Derive FK column identifiers (respects fk_column override, self-ref, or convention)
        let (fk_column_pascal, _fk_field_snake) =
            derive_fk_idents(&join_config, api_struct_name, is_self_referencing);

        // When scoped, derive the child's {StructName}List type path so we can
        // reuse its ScopeFilterable::scope_condition() for both SQL-level
        // filtering (Vec fields) and for recursing via get_one_scoped at
        // depth > 1.
        let child_list_type_path = if scoped {
            let inner_type = extract_api_struct_type_for_recursive_call(&field.ty);
            let inner_type_str = inner_type.to_string();
            let struct_name = inner_type_str
                .split("::")
                .last()
                .unwrap_or(&inner_type_str)
                .trim();
            let list_suffix = format!("{struct_name}List");
            Some(get_path_from_field_type(&field.ty, &list_suffix))
        } else {
            None
        };

        // SQL-level scope filter for Vec children (applied before fetching).
        let scope_filter = if let (true, Some(child_list_type)) =
            (is_vec_field, &child_list_type_path)
        {
            quote! {
                // Apply child entity's scope condition (if any) at the SQL level
                let query = if let Some(child_scope) = <#child_list_type as crudcrate::ScopeFilterable>::scope_condition() {
                    query.filter(child_scope)
                } else {
                    query
                };
            }
        } else {
            quote! {}
        };

        if is_vec_field {
            // Vec<T> relationships (has_many)
            let api_struct_type = extract_api_struct_type_for_recursive_call(&field.ty);

            if depth_limited {
                // Depth=1: Load data, no recursion
                let loaded_var = quote::format_ident!("loaded_{}", field_name);
                let column_path = get_path_from_field_type(&field.ty, "Column");

                loading_statements.push(quote! {
                    let #loaded_var: Vec<#api_struct_type> = {
                        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
                        let query = #entity_path::find()
                            .filter(#column_path::#fk_column_pascal.eq(model.id));
                        #scope_filter
                        let related_models = Box::pin(query.all(db)).await?;
                        related_models
                            .into_iter()
                            .map(|m: #model_path| #api_struct_type::from(m))
                            .collect::<Vec<_>>()
                    };
                });
                field_assignments.push(quote! { result.#field_name = #loaded_var; });
            } else {
                // Depth > 1: Recursive loading.
                // - Scoped: fetch via get_one_scoped with the child's own scope condition
                //   so grandchildren remain filtered.
                // - Unscoped: use get_one (existing behaviour).
                let column_path = get_path_from_field_type(&field.ty, "Column");
                let recursive_fetch = if let Some(child_list_type) = &child_list_type_path {
                    quote! {
                        let __child_scope = <#child_list_type as crudcrate::ScopeFilterable>::scope_condition();
                        match __child_scope {
                            Some(cs) => match #api_struct_type::get_one_scoped(db, related_model.id, &cs).await {
                                Ok(entity) => result.push(entity),
                                Err(e) => {
                                    tracing::warn!(error = %e, "Failed to load nested scoped relations, using flat model");
                                    result.push(related_model.into());
                                }
                            },
                            None => match #api_struct_type::get_one(db, related_model.id).await {
                                Ok(entity) => result.push(entity),
                                Err(e) => {
                                    tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                    result.push(related_model.into());
                                }
                            },
                        }
                    }
                } else {
                    quote! {
                        match #api_struct_type::get_one(db, related_model.id).await {
                            Ok(entity) => result.push(entity),
                            Err(e) => {
                                tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                result.push(related_model.into());
                            }
                        }
                    }
                };

                loading_statements.push(quote! {
                    let #field_name: Vec<#api_struct_type> = {
                        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
                        let query = #entity_path::find()
                            .filter(#column_path::#fk_column_pascal.eq(model.id));
                        #scope_filter
                        let related_models = Box::pin(query.all(db)
                        ).await?;
                        let mut result = Vec::new();
                        for related_model in related_models {
                            #recursive_fetch
                        }
                        result
                    };
                });
                field_assignments.push(quote! { result.#field_name = #field_name; });
            }
        } else {
            // Option<T> or T relationships (belongs_to/has_one)
            // Use find_related() here (wrapped in Box::pin for stack safety) because
            // the FK direction varies: belongs_to has FK on self, has_one has FK on related.
            // find_related() resolves this correctly via the Related<E> trait definition.
            let target_type = extract_option_or_direct_inner_type(&field.ty);

            if depth_limited {
                // Depth=1: Load data, no recursion
                let loaded_var = quote::format_ident!("loaded_{}", field_name);
                loading_statements.push(quote! {
                    let #loaded_var = Box::pin(
                        model.find_related(#entity_path).one(db)
                    ).await?
                    .map(|m: #model_path| #target_type::from(m));
                });
                field_assignments.push(quote! {
                    result.#field_name = #loaded_var;
                });
            } else {
                // Unlimited depth: Recursive loading.
                // Scoped paths use get_one_scoped with the child's own scope condition.
                let recursive_option_fetch = if let Some(child_list_type) = &child_list_type_path {
                    quote! {
                        let __child_scope = <#child_list_type as crudcrate::ScopeFilterable>::scope_condition();
                        match __child_scope {
                            Some(cs) => match #target_type::get_one_scoped(db, related_model.id, &cs).await {
                                Ok(entity) => Some(entity),
                                Err(e) => {
                                    tracing::warn!(error = %e, "Failed to load nested scoped relations, using flat model");
                                    Some(related_model.into())
                                }
                            },
                            None => match #target_type::get_one(db, related_model.id).await {
                                Ok(entity) => Some(entity),
                                Err(e) => {
                                    tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                    Some(related_model.into())
                                }
                            },
                        }
                    }
                } else {
                    quote! {
                        match #target_type::get_one(db, related_model.id).await {
                            Ok(entity) => Some(entity),
                            Err(e) => {
                                tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                Some(related_model.into())
                            }
                        }
                    }
                };

                loading_statements.push(quote! {
                    let #field_name = match Box::pin(
                        model.find_related(#entity_path).one(db)
                    ).await? {
                        Some(related_model) => { #recursive_option_fetch }
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
