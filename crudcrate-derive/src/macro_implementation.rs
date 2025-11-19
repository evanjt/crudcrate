use crate::codegen::{
    handlers::{create, delete, get, update},
    type_resolution::{
        generate_crud_type_aliases, generate_enum_field_checker, generate_field_entries,
        generate_id_column, generate_like_filterable_entries,
    },
};
use crate::traits::crudresource::structs::{CRUDResourceMeta, EntityFieldAnalysis};
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
    let fulltext_entries = generate_field_entries(&analysis.fulltext_fields);
    let enum_field_checker = generate_enum_field_checker(&analysis.db_fields);
    let name_singular = crud_meta.name_singular.as_deref().unwrap_or("resource");
    let description = crud_meta.description.as_deref().unwrap_or("");
    let fulltext_language = crud_meta.fulltext_language.as_deref().unwrap_or("english");

    let (get_one_impl, get_all_impl, create_impl, update_impl, delete_impl, delete_many_impl) =
        generate_method_impls(crud_meta, analysis);

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

            #get_one_impl
            #get_all_impl
            #create_impl
            #update_impl
            #delete_impl
            #delete_many_impl
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
    let get_one_impl = get::generate_get_one_impl(crud_meta, analysis);
    let get_all_impl = get::generate_get_all_impl(crud_meta, analysis);
    let create_impl = create::generate_create_impl(crud_meta);
    let update_impl = update::generate_update_impl(crud_meta);
    let delete_impl = delete::generate_delete_impl(crud_meta);
    let delete_many_impl = delete::generate_delete_many_impl(crud_meta);

    (
        get_one_impl,
        get_all_impl,
        create_impl,
        update_impl,
        delete_impl,
        delete_many_impl,
    )
}
