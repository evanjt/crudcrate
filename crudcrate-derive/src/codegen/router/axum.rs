use quote::{format_ident, quote};

pub(crate) fn generate_router_impl(
    api_struct_name: &syn::Ident,
    has_scoped_fields: bool,
) -> proc_macro2::TokenStream {
    let create_model_name = format_ident!("{}Create", api_struct_name);
    let update_model_name = format_ident!("{}Update", api_struct_name);
    let list_model_name = format_ident!("{}List", api_struct_name);
    let response_model_name = format_ident!("{}Response", api_struct_name);
    let scoped_list_name = if has_scoped_fields {
        format_ident!("{}ScopedList", api_struct_name)
    } else {
        list_model_name.clone()
    };
    let scoped_response_name = if has_scoped_fields {
        format_ident!("{}ScopedResponse", api_struct_name)
    } else {
        response_model_name.clone()
    };

    generate_axum_router(
        api_struct_name,
        &create_model_name,
        &update_model_name,
        &list_model_name,
        &response_model_name,
        &scoped_list_name,
        &scoped_response_name,
    )
}

fn generate_axum_router(
    api_struct_name: &syn::Ident,
    create_model_name: &syn::Ident,
    update_model_name: &syn::Ident,
    list_model_name: &syn::Ident,
    response_model_name: &syn::Ident,
    scoped_list_name: &syn::Ident,
    scoped_response_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    quote! {
        // Generate CRUD handlers using the crudcrate macro
        crudcrate::crud_handlers!(#api_struct_name, #update_model_name, #create_model_name, #list_model_name, #response_model_name, #scoped_list_name, #scoped_response_name);

        impl #api_struct_name {
            /// Generate router with all CRUD endpoints
            pub fn router(db: &sea_orm::DatabaseConnection) -> utoipa_axum::router::OpenApiRouter
            where
                Self: crudcrate::traits::CRUDResource,
            {
                use utoipa_axum::{router::OpenApiRouter, routes};

                tracing::info!(
                    resource = Self::RESOURCE_NAME_PLURAL,
                    table = Self::TABLE_NAME,
                    batch_limit = Self::batch_limit(),
                    max_page_size = Self::max_page_size(),
                    "Mounting CRUD routes with security defaults: input_sanitization=enabled, sql_parameterization=enabled. See https://crudcrate.evanjt.com/latest/advanced/security.html"
                );

                OpenApiRouter::new()
                    .routes(routes!(get_one_handler))
                    .routes(routes!(get_all_handler))
                    .routes(routes!(create_one_handler))
                    .routes(routes!(create_many_handler))
                    .routes(routes!(update_one_handler))
                    .routes(routes!(update_many_handler))
                    .routes(routes!(delete_one_handler))
                    .routes(routes!(delete_many_handler))
                    .with_state(db.clone())
            }

            /// Generate read-only router with only GET endpoints.
            ///
            /// Use with [`ScopeCondition`](crudcrate::ScopeCondition) to create
            /// public/filtered API endpoints:
            ///
            /// ```rust,ignore
            /// use crudcrate::ScopeCondition;
            ///
            /// let public = Article::read_only_router(&db)
            ///     .layer(Extension(ScopeCondition(
            ///         Condition::all().add(article::Column::IsPrivate.eq(false))
            ///     )));
            /// ```
            pub fn read_only_router(db: &sea_orm::DatabaseConnection) -> utoipa_axum::router::OpenApiRouter
            where
                Self: crudcrate::traits::CRUDResource,
            {
                use utoipa_axum::{router::OpenApiRouter, routes};

                tracing::info!(
                    resource = Self::RESOURCE_NAME_PLURAL,
                    table = Self::TABLE_NAME,
                    max_page_size = Self::max_page_size(),
                    "Mounting read-only routes"
                );

                OpenApiRouter::new()
                    .routes(routes!(get_one_handler))
                    .routes(routes!(get_all_handler))
                    .with_state(db.clone())
            }
        }
    }
}
