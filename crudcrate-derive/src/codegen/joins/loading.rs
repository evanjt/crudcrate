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
use heck::ToPascalCase;
use quote::quote;

/// Generate `joined_field_has_scope` method for `CRUDResource` impl.
///
/// Emits a match arm per `Vec<Child>` join field with `filterable(...)` columns,
/// resolving to `<ChildList as ScopeFilterable>::scope_condition().is_some()` at
/// runtime. Used by the `scope_propagation_strict` profile field to decide
/// whether a joined filter is safe under an active `ScopeCondition`.
///
/// Falls back to `false` for unknown field names — matching the default trait
/// impl's conservative posture.
pub fn generate_joined_field_has_scope_impl(
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    let candidates: Vec<(&syn::Field, String)> = analysis
        .join_on_all_fields
        .iter()
        .filter_map(|&field| {
            if !is_vec_type(&field.ty) {
                return None;
            }
            let field_name = field.ident.as_ref()?.to_string();
            let config = analysis
                .join_filter_sort_configs
                .iter()
                .find(|c| c.field_name == field_name)?;
            if config.filterable_columns.is_empty() {
                None
            } else {
                Some((field, field_name))
            }
        })
        .collect();

    if candidates.is_empty() {
        return quote! {};
    }

    let arms = candidates.iter().map(|(field, field_name)| {
        let inner_type = extract_api_struct_type_for_recursive_call(&field.ty);
        let inner_type_string = inner_type.to_string();
        let list_suffix = {
            let struct_name = inner_type_string
                .split("::")
                .last()
                .unwrap_or(&inner_type_string)
                .trim();
            format!("{struct_name}List")
        };
        let child_list_type = get_path_from_field_type(&field.ty, &list_suffix);
        let _ = api_struct_name;
        quote! {
            #field_name => {
                <#child_list_type as crudcrate::ScopeFilterable>::scope_condition().is_some()
            }
        }
    });

    quote! {
        fn joined_field_has_scope(field: &str) -> bool {
            match field {
                #( #arms )*
                _ => false,
            }
        }
    }
}

/// Generate `resolve_joined_filters` method for `CRUDResource` impl.
///
/// For each `join(..., filterable(...))` field on a `Vec<Child>`, emits a
/// match arm that, when a [`crudcrate::JoinedFilter`] targets that field,
/// runs a sub-query on the child entity with the child's
/// [`crudcrate::ScopeFilterable::scope_condition()`] applied, collects the
/// matching parent-FK values, and adds `Self::ID_COLUMN.is_in(ids)` to the
/// augmented condition.
///
/// `Option<Child>` fields (`belongs_to`) and fields without declared filterable
/// columns are silently skipped — the default trait impl's debug-log
/// behavior handles the runtime case.
///
/// Returns empty tokens if there are no filterable joined columns (letting
/// the default trait method handle the no-op case).
pub fn generate_resolve_joined_filters_impl(
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    // Collect Vec<Child> fields that have non-empty filterable_columns
    let candidates: Vec<(&syn::Field, String, Vec<String>)> = analysis
        .join_on_all_fields
        .iter()
        .filter_map(|&field| {
            // Only Vec<Child> — Option<Child> (belongs_to) has the FK on the
            // parent, not the child, so the sub-query direction is reversed
            // and requires different codegen. Skip for now.
            if !is_vec_type(&field.ty) {
                return None;
            }
            let field_name = field.ident.as_ref()?.to_string();
            let config = analysis
                .join_filter_sort_configs
                .iter()
                .find(|c| c.field_name == field_name)?;
            if config.filterable_columns.is_empty() {
                None
            } else {
                Some((field, field_name, config.filterable_columns.clone()))
            }
        })
        .collect();

    if candidates.is_empty() {
        // No override needed — default trait impl returns the condition unchanged
        return quote! {};
    }

    let field_arms = candidates.iter().map(|(field, field_name, filterable_columns)| {
        let join_config = get_join_config(field).unwrap_or_default();

        let inner_type = extract_api_struct_type_for_recursive_call(&field.ty);
        let inner_type_string = inner_type.to_string();
        let api_struct_name_string = api_struct_name.to_string();
        let is_self_referencing = inner_type_string.trim() == api_struct_name_string.trim();

        // Entity / Column / Model paths
        let (entity_path, column_path) = if let Some(custom_path) = &join_config.path {
            if let Ok(path_tokens) = custom_path.parse::<proc_macro2::TokenStream>() {
                (
                    quote! { #path_tokens::Entity },
                    quote! { #path_tokens::Column },
                )
            } else {
                let error_msg = format!("Invalid join path '{custom_path}' for field '{field_name}'");
                return quote! { compile_error!(#error_msg); };
            }
        } else {
            (
                get_path_from_field_type(&field.ty, "Entity"),
                get_path_from_field_type(&field.ty, "Column"),
            )
        };

        let (fk_column_pascal, _fk_field_snake, use_runtime_filter) =
            derive_fk_idents(&join_config, api_struct_name, is_self_referencing);

        // Child List type path for ScopeFilterable::scope_condition()
        let list_suffix = {
            let struct_name = inner_type_string
                .split("::")
                .last()
                .unwrap_or(&inner_type_string)
                .trim();
            format!("{struct_name}List")
        };
        let child_list_type = get_path_from_field_type(&field.ty, &list_suffix);

        // Column match arms: "make" => Some(column::Make), ...
        let column_arms = filterable_columns.iter().map(|col| {
            let col_pascal = quote::format_ident!("{}", col.to_pascal_case());
            quote! {
                #col => crudcrate::build_comparison_expr(
                    #column_path::#col_pascal,
                    __jf.operator,
                    &__jf.value,
                ),
            }
        });

        // FK column on the child, referenced in the subquery's SELECT. Static and
        // self-referencing fields use the typed column. The convention-derived path
        // resolves the FK name from the RelationDef at runtime (as the sort path does),
        // avoiding a FromStr round-trip that could panic.
        let fk_col_ref = if is_self_referencing || !use_runtime_filter {
            quote! {
                {
                    let (__t, __c) = sea_orm::ColumnTrait::as_column_ref(
                        &#column_path::#fk_column_pascal
                    );
                    sea_orm::sea_query::IntoColumnRef::into_column_ref((__t, __c))
                }
            }
        } else {
            quote! {
                {
                    let __rel_def = <#entity_path as sea_orm::Related<
                        <Self as crudcrate::traits::CRUDResource>::EntityType
                    >>::to();
                    let __fk_col_name: String = __rel_def.from_col.iter().next()
                        .map(|__i| __i.inner().to_string())
                        .unwrap_or_default();
                    let __child_tbl = sea_orm::EntityName::table_name(&#entity_path).to_string();
                    sea_orm::sea_query::IntoColumnRef::into_column_ref((
                        sea_orm::sea_query::Alias::new(__child_tbl),
                        sea_orm::sea_query::Alias::new(__fk_col_name),
                    ))
                }
            }
        };

        quote! {
            #field_name => {
                let __sub_expr: Option<sea_orm::sea_query::SimpleExpr> = match __jf.column.as_str() {
                    #( #column_arms )*
                    _ => None,
                };

                if let Some(__sub_expr) = __sub_expr {
                    use sea_orm::sea_query::{Expr, ExprTrait, Query};

                    let __child_scope: Option<sea_orm::Condition> =
                        <#child_list_type as crudcrate::ScopeFilterable>::scope_condition();

                    // Match parents with `id IN (SELECT <fk> FROM child WHERE <expr>
                    // [AND <child scope>])`. The database does the work, so no child
                    // rows are materialised and the bound-parameter count is
                    // independent of how many children match.
                    let mut __where = sea_orm::Condition::all().add(__sub_expr);
                    if let Some(__cs) = __child_scope {
                        __where = __where.add(__cs);
                    }

                    let __fk_ref: sea_orm::sea_query::ColumnRef = #fk_col_ref;
                    let __subquery = Query::select()
                        .expr(Expr::col(__fk_ref))
                        .from(#entity_path)
                        .cond_where(__where)
                        .to_owned();

                    let (__pt, __pc) = sea_orm::ColumnTrait::as_column_ref(&Self::ID_COLUMN);
                    let __parent_ref = sea_orm::sea_query::IntoColumnRef::into_column_ref((__pt, __pc));
                    __augmented = __augmented.add(Expr::col(__parent_ref).in_subquery(__subquery));
                }
            }
        }
    });

    quote! {
        async fn resolve_joined_filters(
            _db: &sea_orm::DatabaseConnection,
            condition: sea_orm::Condition,
            joined_filters: &[crudcrate::JoinedFilter],
        ) -> Result<sea_orm::Condition, crudcrate::ApiError> {
            if joined_filters.is_empty() {
                return Ok(condition);
            }

            let mut __augmented = condition;

            for __jf in joined_filters {
                match __jf.join_field.as_str() {
                    #( #field_arms )*
                    _ => {} // Unknown join_field, silently skip (matches parser-level behaviour)
                }
            }

            Ok(__augmented)
        }
    }
}

/// Generate `get_all_joined_sorted` method for `CRUDResource` impl.
///
/// For each `join(..., sortable(...))` field on a `Vec<Child>`, emits a match
/// arm that orders the parent query by a correlated sub-query over the child
/// column: `ORDER BY (SELECT MIN(child.<column>) FROM child WHERE child.<fk> =
/// parent.<pk>) <dir>`. `MIN` is used for both directions so each parent maps
/// to a single ordering key (no JOIN, no `DISTINCT`, parents stay unique) and
/// to-many relations have a deterministic key.
///
/// `Option<Child>` fields (`belongs_to`) and fields without declared sortable
/// columns are skipped. Unknown `join_field`/`column` combinations fall back to
/// ordering by the parent's default index column — matching the trait default's
/// no-op behavior so the request never mis-orders silently.
///
/// Returns empty tokens if there are no sortable joined columns (letting the
/// default trait method handle the fallback case).
pub fn generate_get_all_joined_sorted_impl(
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    // Collect Vec<Child> fields that have non-empty sortable_columns. Mirrors
    // generate_resolve_joined_filters_impl: Option<Child> (belongs_to) has the
    // FK on the parent, so the correlated sub-query direction is reversed and is
    // not handled here.
    let candidates: Vec<(&syn::Field, String, Vec<String>)> = analysis
        .join_on_all_fields
        .iter()
        .filter_map(|&field| {
            if !is_vec_type(&field.ty) {
                return None;
            }
            let field_name = field.ident.as_ref()?.to_string();
            let config = analysis
                .join_filter_sort_configs
                .iter()
                .find(|c| c.field_name == field_name)?;
            if config.sortable_columns.is_empty() {
                None
            } else {
                Some((field, field_name, config.sortable_columns.clone()))
            }
        })
        .collect();

    if candidates.is_empty() {
        // No override needed — default trait impl falls back to default_index_column
        return quote! {};
    }

    let field_arms = candidates.iter().map(|(field, field_name, sortable_columns)| {
        let join_config = get_join_config(field).unwrap_or_default();

        let inner_type = extract_api_struct_type_for_recursive_call(&field.ty);
        let inner_type_string = inner_type.to_string();
        let api_struct_name_string = api_struct_name.to_string();
        let is_self_referencing = inner_type_string.trim() == api_struct_name_string.trim();

        // Entity / Column paths for the child table
        let (entity_path, column_path) = if let Some(custom_path) = &join_config.path {
            if let Ok(path_tokens) = custom_path.parse::<proc_macro2::TokenStream>() {
                (
                    quote! { #path_tokens::Entity },
                    quote! { #path_tokens::Column },
                )
            } else {
                let error_msg = format!("Invalid join path '{custom_path}' for field '{field_name}'");
                return quote! { compile_error!(#error_msg); };
            }
        } else {
            (
                get_path_from_field_type(&field.ty, "Entity"),
                get_path_from_field_type(&field.ty, "Column"),
            )
        };

        let (fk_column_pascal, _fk_field_snake, use_runtime_filter) =
            derive_fk_idents(&join_config, api_struct_name, is_self_referencing);

        // The correlated sub-query references the child FK column and the parent
        // PK column. Static FK columns and self-referencing fields use the typed
        // `ColumnTrait::as_column_ref()`; the convention-derived path resolves the
        // FK name from the SeaORM RelationDef at runtime (matching the batch
        // loader) and builds a table-qualified Alias ref.
        let fk_col_ref = if is_self_referencing || !use_runtime_filter {
            quote! {
                {
                    let (__t, __c) = sea_orm::ColumnTrait::as_column_ref(
                        &#column_path::#fk_column_pascal
                    );
                    sea_orm::sea_query::IntoColumnRef::into_column_ref((__t, __c))
                }
            }
        } else {
            quote! {
                {
                    let __rel_def = <#entity_path as sea_orm::Related<
                        <Self as crudcrate::traits::CRUDResource>::EntityType
                    >>::to();
                    let __fk_col_name: String = __rel_def.from_col.iter().next()
                        .map(|__i| __i.inner().to_string())
                        .unwrap_or_default();
                    let __child_tbl = sea_orm::EntityName::table_name(&#entity_path).to_string();
                    sea_orm::sea_query::IntoColumnRef::into_column_ref((
                        sea_orm::sea_query::Alias::new(__child_tbl),
                        sea_orm::sea_query::Alias::new(__fk_col_name),
                    ))
                }
            }
        };

        // Column match arms: "year" => Some(column::Year), ...
        let column_arms = sortable_columns.iter().map(|col| {
            let col_pascal = quote::format_ident!("{}", col.to_pascal_case());
            quote! {
                #col => {
                    let (__t, __c) = sea_orm::ColumnTrait::as_column_ref(
                        &#column_path::#col_pascal
                    );
                    Some(sea_orm::sea_query::IntoColumnRef::into_column_ref((__t, __c)))
                }
            }
        });

        quote! {
            #field_name => {
                let __child_col_ref: Option<sea_orm::sea_query::ColumnRef> = match column {
                    #( #column_arms )*
                    _ => None,
                };

                if let Some(__child_col_ref) = __child_col_ref {
                    use sea_orm::sea_query::{Expr, ExprTrait, Query, QueryStatementBuilder};

                    let __parent_pk_ref = <Self::ColumnType as sea_orm::ColumnTrait>::as_column_ref(
                        &Self::ID_COLUMN
                    );
                    let __fk_ref: sea_orm::sea_query::ColumnRef = #fk_col_ref;

                    let __subquery = Query::select()
                        .expr(Expr::col(__child_col_ref).min())
                        .from(#entity_path)
                        .and_where(Expr::col(__fk_ref).equals(__parent_pk_ref))
                        .to_owned();

                    let __order_expr = sea_orm::sea_query::SimpleExpr::SubQuery(
                        None,
                        Box::new(__subquery.into_sub_query_statement()),
                    );

                    let __models = Self::EntityType::find()
                        .filter(condition.clone())
                        .order_by(__order_expr, direction)
                        .offset(offset)
                        .limit(limit)
                        .all(db)
                        .await
                        .map_err(crudcrate::ApiError::database)?;

                    return Ok(__models
                        .into_iter()
                        .map(|__m| <Self::ListModel as From<Self>>::from(Self::from(__m)))
                        .collect());
                }
            }
        }
    });

    quote! {
        async fn get_all_joined_sorted(
            db: &sea_orm::DatabaseConnection,
            condition: &sea_orm::Condition,
            join_field: &str,
            column: &str,
            direction: sea_orm::Order,
            offset: u64,
            limit: u64,
        ) -> Result<Vec<Self::ListModel>, crudcrate::ApiError> {
            use sea_orm::{EntityTrait, ColumnTrait, QueryFilter, QueryOrder, QuerySelect};

            match join_field {
                #( #field_arms )*
                _ => {}
            }

            // Unknown join_field/column: fall back to the parent default index
            // column (the same key the trait default uses). Never mis-orders.
            Self::get_all(
                db,
                condition,
                Self::default_index_column(),
                direction,
                offset,
                limit,
            )
            .await
        }
    }
}

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

    // The parent's primary-key value type. Used as the HashMap key and the
    // `parent_ids` element type so batch loading works for UUID, integer, or
    // String primary keys (not just `uuid::Uuid`). Generated inside the
    // `impl CRUDResource for ParentApiStruct`, so `Self` resolves to the parent.
    let parent_pk_ty = quote! { crudcrate::PrimaryKeyType<Self> };

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

        let (fk_column_pascal, fk_field_snake, use_runtime) =
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
                        let mut #map_var: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> = Box::pin(async {
                            use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                            let query = #entity_path::find()
                                .filter(#column_path::#fk_column_pascal.is_in(parent_ids.clone()));
                            #scope_filter_for_vec
                            let all_related = query.all(db).await?;

                            let mut map: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> =
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
                } else if use_runtime {
                    // Runtime FK resolution from SeaORM RelationDef
                    batch_loading_statements.push(quote! {
                        let mut #map_var: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> = Box::pin(async {
                            use sea_orm::{EntityTrait, QueryFilter, ColumnTrait, ModelTrait};
                            use sea_orm::sea_query::ExprTrait;
                            use std::str::FromStr;

                            let __rel_def = <#entity_path as sea_orm::Related<
                                <Self as crudcrate::traits::CRUDResource>::EntityType
                            >>::to();
                            let __fk_col_name: String = __rel_def.from_col.iter().next()
                                .map(|__i| __i.inner().to_string())
                                .unwrap_or_default();

                            let query = #entity_path::find()
                                .filter(sea_orm::sea_query::Expr::col(
                                    sea_orm::sea_query::Alias::new(&__fk_col_name)
                                ).is_in(parent_ids.clone()));
                            #scope_filter_for_vec
                            let all_related = query.all(db).await?;

                            let __fk_col = match <<#entity_path as sea_orm::EntityTrait>::Column
                                as FromStr>::from_str(&__fk_col_name)
                            {
                                Ok(__c) => __c,
                                Err(_) => return Err(crudcrate::ApiError::internal(
                                    "CrudCrate: foreign key column not found on child entity",
                                    None,
                                )),
                            };

                            let mut map: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> =
                                std::collections::HashMap::new();
                            for related_model in all_related {
                                let fk_value: #parent_pk_ty = match <#parent_pk_ty as sea_orm::sea_query::ValueType>::try_from(
                                    ModelTrait::get(&related_model, __fk_col.clone())
                                ) {
                                    Ok(v) => v,
                                    Err(_) => continue,
                                };
                                map.entry(fk_value)
                                    .or_insert_with(Vec::new)
                                    .push(#api_struct_type::from(related_model));
                            }
                            Ok::<_, crudcrate::ApiError>(map)
                        }).await?;
                    });
                } else {
                    // Static FK column (explicit fk_column override)
                    batch_loading_statements.push(quote! {
                        let mut #map_var: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> = Box::pin(async {
                            use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                            let query = #entity_path::find()
                                .filter(#column_path::#fk_column_pascal.is_in(parent_ids.clone()));
                            #scope_filter_for_vec
                            let all_related = query.all(db).await?;

                            let mut map: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> =
                                std::collections::HashMap::new();
                            for related_model in all_related {
                                let fk_value = related_model.#fk_field_snake.clone();
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
                                    crudcrate::tracing::warn!(error = %e, "Failed to load nested scoped relations, using flat model");
                                    #api_struct_type::from(related_model)
                                }
                            },
                            None => match #api_struct_type::get_one(db, related_model.id).await {
                                Ok(e) => e,
                                Err(e) => {
                                    crudcrate::tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
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
                                crudcrate::tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                #api_struct_type::from(related_model)
                            }
                        };
                    }
                };

                if use_runtime {
                    batch_loading_statements.push(quote! {
                        let mut #map_var: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> = Box::pin(async {
                            use sea_orm::{EntityTrait, QueryFilter, ColumnTrait, ModelTrait};
                            use sea_orm::sea_query::ExprTrait;
                            use std::str::FromStr;

                            let __rel_def = <#entity_path as sea_orm::Related<
                                <Self as crudcrate::traits::CRUDResource>::EntityType
                            >>::to();
                            let __fk_col_name: String = __rel_def.from_col.iter().next()
                                .map(|__i| __i.inner().to_string())
                                .unwrap_or_default();

                            let query = #entity_path::find()
                                .filter(sea_orm::sea_query::Expr::col(
                                    sea_orm::sea_query::Alias::new(&__fk_col_name)
                                ).is_in(parent_ids.clone()));
                            #scope_filter_for_vec
                            let all_related_models: Vec<#model_path> = query.all(db).await?;

                            let __fk_col = match <<#entity_path as sea_orm::EntityTrait>::Column
                                as FromStr>::from_str(&__fk_col_name)
                            {
                                Ok(__c) => __c,
                                Err(_) => return Err(crudcrate::ApiError::internal(
                                    "CrudCrate: foreign key column not found on child entity",
                                    None,
                                )),
                            };

                            let mut map: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> =
                                std::collections::HashMap::new();
                            for related_model in all_related_models {
                                let fk_value: #parent_pk_ty = match <#parent_pk_ty as sea_orm::sea_query::ValueType>::try_from(
                                    ModelTrait::get(&related_model, __fk_col.clone())
                                ) {
                                    Ok(v) => v,
                                    Err(_) => continue,
                                };
                                #recursive_fetch
                                map.entry(fk_value)
                                    .or_insert_with(Vec::new)
                                    .push(entity);
                            }
                            Ok::<_, crudcrate::ApiError>(map)
                        }).await?;
                    });
                } else {
                    batch_loading_statements.push(quote! {
                        let mut #map_var: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> = Box::pin(async {
                            use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

                            let query = #entity_path::find()
                                .filter(#column_path::#fk_column_pascal.is_in(parent_ids.clone()));
                            #scope_filter_for_vec
                            let all_related_models: Vec<#model_path> = query.all(db).await?;

                            let mut map: std::collections::HashMap<#parent_pk_ty, Vec<#api_struct_type>> =
                                std::collections::HashMap::new();
                            for related_model in all_related_models {
                                let fk_value = related_model.#fk_field_snake.clone();
                                #recursive_fetch
                                map.entry(fk_value)
                                    .or_insert_with(Vec::new)
                                    .push(entity);
                            }
                            Ok::<_, crudcrate::ApiError>(map)
                        }).await?;
                    });
                }

                field_assignments.push(quote! {
                    item.#field_name = #map_var.remove(&parent_id).unwrap_or_default();
                });
            }
        } else {
            // Option<T> relationships (belongs_to/has_one).
            //
            // The FK direction differs between the two shapes: belongs_to has
            // the FK on the parent row, has_one has it on the child. SeaORM's
            // `find_related()` resolves the direction from the `Related<E>`
            // relation definition, so we load each parent's relation that way
            // instead of hand-rolling a sub-query keyed by parent PK (which
            // only handles the has_one direction and silently returns nothing
            // for belongs_to). To-one relations are 1:1 and gain little from a
            // single batched query, so iterating the already-fetched parent
            // models keeps the loader correct for both directions.
            //
            // `fk_column_pascal`, `fk_field_snake`, `column_path`, `model_path`,
            // and `use_runtime` are not needed on this path; mark them used so
            // the Vec<T> branch's bindings don't trip unused-variable lints.
            let _ = (
                &fk_column_pascal,
                &fk_field_snake,
                &column_path,
                &model_path,
                use_runtime,
            );

            let target_type = extract_option_or_direct_inner_type(&field.ty);

            let load_related = if depth_limited {
                quote! {
                    let related = Box::pin(
                        parent_model.find_related(#entity_path).one(db)
                    ).await?
                    .map(#target_type::from);
                }
            } else if scoped {
                // depth > 1, scoped: recurse via get_one_scoped (when the child carries
                // a scope_condition) so the child's own nested joins stay scope-filtered.
                // The Vec<T> branch already does this. Without it an Option<Child> leaks
                // private grandchildren into a scoped response.
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
                    let related = match Box::pin(
                        parent_model.find_related(#entity_path).one(db)
                    ).await? {
                        Some(related_model) => {
                            let __child_scope = <#child_list_type as crudcrate::ScopeFilterable>::scope_condition();
                            let __loaded = match __child_scope {
                                Some(cs) => #target_type::get_one_scoped(db, related_model.id, &cs).await,
                                None => #target_type::get_one(db, related_model.id).await,
                            };
                            match __loaded {
                                Ok(e) => Some(e),
                                Err(e) => {
                                    crudcrate::tracing::warn!(error = %e, "Failed to load nested scoped relations, using flat model");
                                    Some(#target_type::from(related_model))
                                }
                            }
                        }
                        None => None,
                    };
                }
            } else {
                // depth > 1: recurse via the child's get_one so the child's own
                // nested joins are loaded too. Fall back to the flat model on error.
                quote! {
                    let related = match Box::pin(
                        parent_model.find_related(#entity_path).one(db)
                    ).await? {
                        Some(related_model) => {
                            match #target_type::get_one(db, related_model.id).await {
                                Ok(e) => Some(e),
                                Err(e) => {
                                    crudcrate::tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                    Some(#target_type::from(related_model))
                                }
                            }
                        }
                        None => None,
                    };
                }
            };

            batch_loading_statements.push(quote! {
                let mut #map_var: std::collections::HashMap<#parent_pk_ty, #target_type> = Box::pin(async {
                    use sea_orm::{EntityTrait, ModelTrait};

                    let mut map: std::collections::HashMap<#parent_pk_ty, #target_type> =
                        std::collections::HashMap::new();
                    for parent_model in models.iter() {
                        #load_related
                        if let Some(related) = related {
                            map.insert(parent_model.#pk_ident.clone(), related);
                        }
                    }
                    Ok::<_, crudcrate::ApiError>(map)
                }).await?;
            });

            field_assignments.push(quote! {
                item.#field_name = #map_var.remove(&parent_id);
            });
        }
    }

    let pre_loop_code = quote! {
        // Collect all parent IDs for batch loading. Clone each PK so the parent
        // models stay intact for later conversion (the PK value type is not
        // required to be Copy — e.g. String primary keys).
        let parent_ids: Vec<#parent_pk_ty> = models.iter().map(|m| m.#pk_ident.clone()).collect();

        #( #batch_loading_statements )*
    };

    let in_loop_code = quote! {
        // Clone the PK before moving `model` into `Self::from` so non-Copy PK
        // types (String) don't trigger a partial-move error.
        let parent_id = model.#pk_ident.clone();
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
///
/// Returns `(fk_column_pascal, fk_field_snake, use_runtime)`.
/// When `use_runtime` is true, the FK column should be resolved from
/// `RelationDef` at runtime instead of using the static identifiers.
fn derive_fk_idents(
    join_config: &crate::codegen::joins::config::JoinConfig,
    api_struct_name: &syn::Ident,
    is_self_referencing: bool,
) -> (proc_macro2::Ident, proc_macro2::Ident, bool) {
    if let Some(ref fk) = join_config.fk_column {
        (
            quote::format_ident!("{}", fk),
            quote::format_ident!("{}", to_snake_case(fk)),
            false,
        )
    } else if is_self_referencing {
        (
            quote::format_ident!("ParentId"),
            quote::format_ident!("parent_id"),
            false,
        )
    } else {
        (
            quote::format_ident!("{}Id", api_struct_name),
            quote::format_ident!("{}_id", to_snake_case(&api_struct_name.to_string())),
            true, // Use runtime resolution — convention may not match
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

        let (fk_column_pascal, _fk_field_snake, use_runtime_join) =
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
                let loaded_var = quote::format_ident!("loaded_{}", field_name);
                let column_path = get_path_from_field_type(&field.ty, "Column");

                let filter_expr = if use_runtime_join {
                    quote! {
                        let __rel_def = <#entity_path as sea_orm::Related<
                            <Self as crudcrate::traits::CRUDResource>::EntityType
                        >>::to();
                        let __fk_col_name: String = __rel_def.from_col.iter().next()
                            .map(|__i| __i.inner().to_string())
                            .unwrap_or_default();
                        let query = #entity_path::find()
                            .filter(sea_orm::sea_query::ExprTrait::eq(
                                sea_orm::sea_query::Expr::col(
                                    sea_orm::sea_query::Alias::new(&__fk_col_name)
                                ),
                                model.id,
                            ));
                    }
                } else {
                    quote! {
                        let query = #entity_path::find()
                            .filter(#column_path::#fk_column_pascal.eq(model.id));
                    }
                };

                loading_statements.push(quote! {
                    let #loaded_var: Vec<#api_struct_type> = {
                        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
                        #filter_expr
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
                                    crudcrate::tracing::warn!(error = %e, "Failed to load nested scoped relations, using flat model");
                                    result.push(related_model.into());
                                }
                            },
                            None => match #api_struct_type::get_one(db, related_model.id).await {
                                Ok(entity) => result.push(entity),
                                Err(e) => {
                                    crudcrate::tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
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
                                crudcrate::tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
                                result.push(related_model.into());
                            }
                        }
                    }
                };

                let filter_expr_deep = if use_runtime_join {
                    quote! {
                        let __rel_def = <#entity_path as sea_orm::Related<
                            <Self as crudcrate::traits::CRUDResource>::EntityType
                        >>::to();
                        let __fk_col_name: String = __rel_def.from_col.iter().next()
                            .map(|__i| __i.inner().to_string())
                            .unwrap_or_default();
                        let query = #entity_path::find()
                            .filter(sea_orm::sea_query::ExprTrait::eq(
                                sea_orm::sea_query::Expr::col(
                                    sea_orm::sea_query::Alias::new(&__fk_col_name)
                                ),
                                model.id,
                            ));
                    }
                } else {
                    quote! {
                        let query = #entity_path::find()
                            .filter(#column_path::#fk_column_pascal.eq(model.id));
                    }
                };

                loading_statements.push(quote! {
                    let #field_name: Vec<#api_struct_type> = {
                        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
                        #filter_expr_deep
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
                                    crudcrate::tracing::warn!(error = %e, "Failed to load nested scoped relations, using flat model");
                                    Some(related_model.into())
                                }
                            },
                            None => match #target_type::get_one(db, related_model.id).await {
                                Ok(entity) => Some(entity),
                                Err(e) => {
                                    crudcrate::tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
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
                                crudcrate::tracing::warn!(error = %e, "Failed to load nested relations, using flat model");
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
