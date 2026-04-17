use crate::codegen::{
    handlers::{create, delete, get, update},
    joins::get_join_config,
    type_resolution::{
        extract_api_struct_type_for_recursive_call, generate_crud_type_aliases,
        generate_enum_field_checker, generate_field_entries, generate_id_column,
        generate_like_filterable_entries, generate_scoped_excluded_entries,
        get_path_from_field_type, is_vec_type,
    },
};
use crate::traits::crudresource::structs::{
    CRUDResourceMeta, EntityFieldAnalysis, JoinFilterSortConfig,
};
use quote::quote;

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
    let scoped_excluded_entries = generate_scoped_excluded_entries(&analysis.db_fields);
    let fulltext_entries = generate_field_entries(&analysis.fulltext_fields);
    let enum_field_checker = generate_enum_field_checker(&analysis.db_fields);
    let name_singular = crud_meta.name_singular.as_deref().unwrap_or("resource");
    let description = crud_meta.description.as_deref().unwrap_or("");
    let fulltext_language = crud_meta.fulltext_language.as_deref().unwrap_or("english");

    // Generate joined filterable/sortable column definitions
    let joined_filterable_entries =
        generate_joined_column_entries(&analysis.join_filter_sort_configs, true);
    let joined_sortable_entries =
        generate_joined_column_entries(&analysis.join_filter_sort_configs, false);

    let (
        get_one_impl,
        get_all_impl,
        create_impl,
        create_many_impl,
        update_impl,
        update_many_impl,
        delete_impl,
        delete_many_impl,
    ) = generate_method_impls(crud_meta, analysis, api_struct_name);

    // Generate registration lazy static and auto-registration call only for models without join fields
    // Models with join fields may have circular dependencies that prevent CRUDResource compilation
    let _has_join_fields =
        !analysis.join_on_one_fields.is_empty() || !analysis.join_on_all_fields.is_empty();

    // Generate resource name plural constant
    let resource_name_plural_impl = {
        let name_plural = crud_meta.name_plural.clone().unwrap_or_default();
        quote! {
            const RESOURCE_NAME_PLURAL: &'static str = #name_plural;
        }
    };

    // Generate configurable limits (only if specified, otherwise use trait defaults)
    let batch_limit_impl = crud_meta.batch_limit.map(|limit| {
        quote! {
            fn batch_limit() -> usize { #limit }
        }
    });

    let max_page_size_impl = crud_meta.max_page_size.map(|size| {
        quote! {
            fn max_page_size() -> u64 { #size }
        }
    });

    // Generate require_scope constant (only when attribute is set, otherwise use trait default)
    let require_scope_impl = if crud_meta.require_scope {
        Some(quote! {
            const REQUIRE_SCOPE: bool = true;
        })
    } else {
        None
    };

    // Generate #[cfg(test)] FK validation tests for Vec joins
    let fk_validation_tests = generate_fk_validation_tests(analysis, api_struct_name);

    quote! {
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
            #batch_limit_impl
            #require_scope_impl
            #max_page_size_impl

            fn sortable_columns() -> Vec<(&'static str, Self::ColumnType)> {
                vec![#(#sortable_entries),*]
            }

            fn filterable_columns() -> Vec<(&'static str, Self::ColumnType)> {
                vec![#(#filterable_entries),*]
            }

            fn is_enum_field(field_name: &str) -> bool {
                #enum_field_checker
            }

            fn like_filterable_columns() -> Vec<&'static str> {
                vec![#(#like_filterable_entries),*]
            }

            fn fulltext_searchable_columns() -> Vec<(&'static str, Self::ColumnType)> {
                vec![#(#fulltext_entries),*]
            }

            fn scoped_excluded_columns() -> &'static [&'static str] {
                &[#(#scoped_excluded_entries),*]
            }

            fn joined_filterable_columns() -> Vec<crudcrate::JoinedColumnDef> {
                vec![#(#joined_filterable_entries),*]
            }

            fn joined_sortable_columns() -> Vec<crudcrate::JoinedColumnDef> {
                vec![#(#joined_sortable_entries),*]
            }

            #get_one_impl
            #get_all_impl
            #create_impl
            #create_many_impl
            #update_impl
            #update_many_impl
            #delete_impl
            #delete_many_impl
        }

        #fk_validation_tests
    }
}

/// Generate `JoinedColumnDef` entries for filterable or sortable columns on joined entities.
///
/// # Arguments
/// * `configs` - The join filter/sort configurations from field analysis
/// * `filterable` - If true, generate filterable entries; if false, generate sortable entries
fn generate_joined_column_entries(
    configs: &[JoinFilterSortConfig],
    filterable: bool,
) -> Vec<proc_macro2::TokenStream> {
    let mut entries = Vec::new();

    for config in configs {
        let join_field = &config.field_name;
        let columns = if filterable {
            &config.filterable_columns
        } else {
            &config.sortable_columns
        };

        for column in columns {
            let full_path = format!("{join_field}.{column}");
            entries.push(quote! {
                crudcrate::JoinedColumnDef {
                    join_field: #join_field,
                    column_name: #column,
                    full_path: #full_path,
                }
            });
        }
    }

    entries
}

/// Generate `#[cfg(test)]` functions that validate FK column naming conventions
/// against the actual `SeaORM` `RelationDef` at test time.
///
/// For each Vec<T> join field, generates a test that:
/// 1. Fetches the `RelationDef` via `<ChildEntity as Related<ParentEntity>>::to()`
/// 2. Extracts the `from_col` column name via `Iden::unquoted()`
/// 3. Asserts it matches our convention-derived FK column name
///
/// This catches FK naming convention mismatches in CI before they reach production.
fn generate_fk_validation_tests(
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    let mut tests = Vec::new();

    // Collect all join fields (both on_one and on_all, deduplicated)
    let mut seen = std::collections::HashSet::new();
    let all_join_fields: Vec<&syn::Field> = analysis
        .join_on_one_fields
        .iter()
        .chain(analysis.join_on_all_fields.iter())
        .copied()
        .filter(|f| {
            f.ident
                .as_ref()
                .is_none_or(|name| seen.insert(name.to_string()))
        })
        .collect();

    for field in &all_join_fields {
        // Only generate for Vec<T> fields (has_many) — these use convention-derived FK columns.
        // Option<T> fields use find_related() which handles FK resolution internally.
        if !is_vec_type(&field.ty) {
            continue;
        }

        let Some(field_name) = &field.ident else {
            continue;
        };

        let join_config = get_join_config(field).unwrap_or_default();

        // Skip self-referencing joins — they use ParentId which is a crudcrate convention,
        // not derived from SeaORM relations
        let inner_type = extract_api_struct_type_for_recursive_call(&field.ty);
        if inner_type.to_string().trim() == api_struct_name.to_string().trim() {
            continue;
        }

        // If fk_column is explicitly set, the user owns the mapping — skip validation
        if join_config.fk_column.is_some() {
            continue;
        }

        // Derive the convention FK column name
        let fk_snake = {
            use convert_case::{Case, Casing};
            format!("{}_id", api_struct_name.to_string().to_case(Case::Snake))
        };

        // Get child entity path
        let child_entity = get_path_from_field_type(&field.ty, "Entity");

        let test_fn_name = quote::format_ident!(
            "_crudcrate_validate_fk_{}_{}",
            api_struct_name.to_string().to_lowercase(),
            field_name
        );

        // Adjust entity paths for the test module. From inside the nested test
        // submodule, `super::` reaches the parent db module. Absolute paths
        // (`crate::...`) resolve the same from any position in the crate, so
        // don't prepend `super::` to them — that would produce an invalid
        // `super::crate::...` path. Relative paths from `get_path_from_field_type`
        // (e.g., `super::module::Entity`) do need the extra hop.
        let parent_entity = quote! { super::Entity };
        let child_entity_str = child_entity.to_string();
        let child_entity_adjusted = if child_entity_str.trim_start().starts_with("crate ::")
            || child_entity_str.trim_start().starts_with(":: crate")
            || child_entity_str.trim_start().starts_with("crate::")
        {
            quote! { #child_entity }
        } else {
            quote! { super::#child_entity }
        };

        let assert_msg = format!(
            "crudcrate FK mismatch: convention derived '{fk_snake}' for join '{api_struct_name}.{field_name}', \
             but SeaORM RelationDef says the FK column is '{{}}'. \
             Fix: add fk_column = \"ActualColumnName\" to the join attribute."
        );

        tests.push(quote! {
            #[test]
            fn #test_fn_name() {
                use sea_orm::Iden;
                // Get the RelationDef: ChildEntity -> ParentEntity (the FK is on the child)
                let def = <#child_entity_adjusted as sea_orm::Related<#parent_entity>>::to();
                let mut from_col_name = String::new();
                def.from_col.unquoted(&mut from_col_name);
                assert_eq!(
                    from_col_name, #fk_snake,
                    #assert_msg, from_col_name
                );
            }
        });
    }

    if tests.is_empty() {
        return quote! {};
    }

    let mod_name = quote::format_ident!(
        "_crudcrate_fk_validation_{}",
        api_struct_name.to_string().to_lowercase()
    );

    // Place tests in a submodule with `use super::*;` — the child entity paths
    // reference `super::child_module::Entity` which resolves from the test module
    // as `super::super::child_module::Entity`. To fix this, we also `use super::*`
    // and reference the Entity types through the re-exports in the parent module.
    quote! {
        #[cfg(test)]
        #[allow(non_snake_case)]
        mod #mod_name {
            use super::*;
            #(#tests)*
        }
    }
}

fn generate_method_impls(
    crud_meta: &CRUDResourceMeta,
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
) -> (
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
) {
    let get_one_impl = get::generate_get_one_impl(crud_meta, analysis, api_struct_name);
    let get_all_impl = get::generate_get_all_impl(crud_meta, analysis, api_struct_name);
    let create_impl = create::generate_create_impl(crud_meta);
    let create_many_impl = create::generate_create_many_impl(crud_meta);
    let update_impl = update::generate_update_impl(crud_meta);
    let update_many_impl = update::generate_update_many_impl(crud_meta);
    let delete_impl = delete::generate_delete_impl(crud_meta);
    let delete_many_impl = delete::generate_delete_many_impl(crud_meta);

    (
        get_one_impl,
        get_all_impl,
        create_impl,
        create_many_impl,
        update_impl,
        update_many_impl,
        delete_impl,
        delete_many_impl,
    )
}
