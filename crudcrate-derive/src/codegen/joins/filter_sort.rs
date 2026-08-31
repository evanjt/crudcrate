//! Joined filter and sort sub-queries: `resolve_joined_filters`, `get_all_joined_sorted`, `joined_field_has_scope`.

use crate::attrs::get_join_config;
use crate::codegen::joins::fk::{
    child_paths, derive_fk_idents, fk_column_ref, list_type_of_child, relation_def_expr,
    self_referencing,
};
use crate::ir::EntityFieldAnalysis;
use crate::syn_type::{column_ident, extract_api_struct_type_for_recursive_call, is_vec_type};
use quote::quote;

/// Generate `joined_field_has_scope` method for `CRUDResource` impl.
///
/// Emits a match arm per `Vec<Child>` join field with `filterable(...)` columns,
/// resolving to `<ChildList as ScopeFilterable>::scope_condition().is_some()` at
/// runtime. Used by the `scope_propagation_strict` profile field to decide
/// whether a joined filter is safe under an active `ScopeCondition`.
///
/// Falls back to `false` for unknown field names, matching the default trait
/// impl's conservative posture.
pub(crate) fn generate_joined_field_has_scope_impl(
    analysis: &EntityFieldAnalysis,
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
        let child_list_type = list_type_of_child(field);
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
/// columns are silently skipped; the default trait impl's debug-log
/// behavior handles the runtime case.
///
/// Returns empty tokens if there are no filterable joined columns (letting
/// the default trait method handle the no-op case).
pub(crate) fn generate_resolve_joined_filters_impl(
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    // Collect Vec<Child> fields that have non-empty filterable_columns
    let candidates: Vec<(&syn::Field, String, Vec<String>)> = analysis
        .join_on_all_fields
        .iter()
        .filter_map(|&field| {
            // Only Vec<Child>; Option<Child> (belongs_to) has the FK on the
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
        // No override needed; default trait impl returns the condition unchanged
        return quote! {};
    }

    let field_arms = candidates.iter().map(|(field, field_name, filterable_columns)| {
        let join_config = get_join_config(field).unwrap_or_default();

        let inner_type = extract_api_struct_type_for_recursive_call(&field.ty);
        let is_self_referencing = self_referencing(&field.ty, api_struct_name);

        // Entity / Column / Model paths
        let (entity_path, column_path) = match child_paths(field, field_name, &join_config) {
            Ok(paths) => paths,
            Err(e) => return e,
        };

        let (fk_column_pascal, _fk_field_snake, use_runtime_filter) =
            derive_fk_idents(&join_config, api_struct_name, is_self_referencing);
        let relation_def = relation_def_expr(&join_config, &field.ty, &entity_path);

        // Child List type path for ScopeFilterable::scope_condition()
        let child_list_type = list_type_of_child(field);

        // Column match arms: "make" => Some(column::Make), ...
        let column_arms = filterable_columns.iter().map(|col| {
            let col_pascal = column_ident(col);
            quote! {
                #col => crudcrate::build_filter_expr::<#inner_type, _>(
                    #column_path::#col_pascal,
                    #col,
                    __jf.operator,
                    &__jf.value,
                    __backend,
                )?,
            }
        });

        // FK column on the child, referenced in the subquery's SELECT. Static and
        // self-referencing fields use the typed column. The convention-derived path
        // resolves the FK name from the RelationDef at runtime (as the sort path does),
        // avoiding a FromStr round-trip that could panic.
        let fk_col_ref = fk_column_ref(
            is_self_referencing,
            use_runtime_filter,
            &relation_def,
            &entity_path,
            &column_path,
            &fk_column_pascal,
        );

        quote! {
            #field_name => {
                let __sub_expr: Option<sea_orm::sea_query::Expr> = match __jf.column.as_str() {
                    #( #column_arms )*
                    _ => None,
                };

                if let Some(__sub_expr) = __sub_expr {
                    use sea_orm::ExprTrait;
                    use sea_orm::sea_query::{Expr, Query, QueryStatementBuilder};

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
                    let __parent_ref = crudcrate::table_column_ref(__pt, __pc);
                    __augmented = __augmented.add(Expr::col(__parent_ref).in_subquery(__subquery));
                }
            }
        }
    });

    quote! {
        async fn resolve_joined_filters(
            db: &sea_orm::DatabaseConnection,
            condition: sea_orm::Condition,
            joined_filters: &[crudcrate::JoinedFilter],
        ) -> Result<sea_orm::Condition, crudcrate::ApiError> {
            if joined_filters.is_empty() {
                return Ok(condition);
            }

            let __backend = sea_orm::ConnectionTrait::get_database_backend(db);
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
/// ordering by the parent's default index column, matching the trait default's
/// no-op behavior so the request never mis-orders silently.
///
/// Returns empty tokens if there are no sortable joined columns (letting the
/// default trait method handle the fallback case).
pub(crate) fn generate_get_all_joined_sorted_impl(
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
        // No override needed; default trait impl falls back to default_index_column
        return quote! {};
    }

    let field_arms = candidates.iter().map(|(field, field_name, sortable_columns)| {
        let join_config = get_join_config(field).unwrap_or_default();

        let is_self_referencing = self_referencing(&field.ty, api_struct_name);

        // Entity / Column paths for the child table
        let (entity_path, column_path) = match child_paths(field, field_name, &join_config) {
            Ok(paths) => paths,
            Err(e) => return e,
        };

        let (fk_column_pascal, _fk_field_snake, use_runtime_filter) =
            derive_fk_idents(&join_config, api_struct_name, is_self_referencing);
        let relation_def = relation_def_expr(&join_config, &field.ty, &entity_path);

        // The correlated sub-query references the child FK column and the parent
        // PK column. Static FK columns and self-referencing fields use the typed
        // `ColumnTrait::as_column_ref()`; the convention-derived path resolves the
        // FK name from the SeaORM RelationDef at runtime (matching the batch
        // loader) and builds a table-qualified Alias ref.
        let fk_col_ref = fk_column_ref(
            is_self_referencing,
            use_runtime_filter,
            &relation_def,
            &entity_path,
            &column_path,
            &fk_column_pascal,
        );

        // Column match arms: "year" => Some(column::Year), ...
        let column_arms = sortable_columns.iter().map(|col| {
            let col_pascal = column_ident(col);
            quote! {
                #col => {
                    let (__t, __c) = sea_orm::ColumnTrait::as_column_ref(
                        &#column_path::#col_pascal
                    );
                    Some(crudcrate::table_column_ref(__t, __c))
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
                    use sea_orm::ExprTrait;
                    use sea_orm::sea_query::{Expr, Query, QueryStatementBuilder};

                    let __parent_pk_ref = <Self::ColumnType as sea_orm::ColumnTrait>::as_column_ref(
                        &Self::ID_COLUMN
                    );
                    let __fk_ref: sea_orm::sea_query::ColumnRef = #fk_col_ref;

                    let __subquery = Query::select()
                        .expr(Expr::col(__child_col_ref).min())
                        .from(#entity_path)
                        .and_where(Expr::col(__fk_ref).equals(__parent_pk_ref))
                        .to_owned();

                    let __order_expr = sea_orm::sea_query::Expr::SubQuery(
                        None,
                        Box::new(__subquery.into_sub_query_statement()),
                    );

                    let __models = Self::EntityType::find()
                        .filter(condition.clone())
                        .order_by(__order_expr, direction)
                        .order_by(Self::ID_COLUMN, sea_orm::Order::Asc)
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
