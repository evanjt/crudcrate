mod structs;
mod attribute_parser;
mod field_analyzer;
mod macro_implementation;
// mod join_generators; // Removed - functions moved to macro_implementation.rs
mod relation_validator;
mod attributes;
#[cfg(feature = "debug")]
mod debug_output;

use proc_macro::TokenStream;

use quote::{ToTokens, format_ident, quote};
use syn::parse::Parser;
use syn::{Data, DeriveInput, Fields, Lit, Meta, parse_macro_input, punctuated::Punctuated, token::Comma};
use heck::ToPascalCase;

use structs::{CRUDResourceMeta, EntityFieldAnalysis};

// Don't need explicit imports since we have mod declarations above

// Helper functions moved from helpers.rs

fn extract_active_model_type(
    input: &DeriveInput,
    name: &syn::Ident,
) -> proc_macro2::TokenStream {
    let mut active_model_override = None;
    for attr in &input.attrs {
        if attr.path().is_ident("active_model")
            && let Some(s) = attribute_parser::get_string_from_attr(attr)
        {
            active_model_override =
                Some(syn::parse_str::<syn::Type>(&s).expect("Invalid active_model type"));
        }
    }
    if let Some(ty) = active_model_override {
        quote! { #ty }
    } else {
        let ident = format_ident!("{}ActiveModel", name);
        quote! { #ident }
    }
}

fn extract_named_fields(
    input: &DeriveInput,
) -> syn::punctuated::Punctuated<syn::Field, syn::token::Comma> {
    if let Data::Struct(data) = &input.data {
        if let Fields::Named(named) = &data.fields {
            named.named.clone()
        } else {
            panic!("ToCreateModel only supports structs with named fields");
        }
    } else {
        panic!("ToCreateModel can only be derived for structs");
    }
}

fn generate_update_merge_code(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    included_fields: &[&syn::Field],
) -> (Vec<proc_macro2::TokenStream>, Vec<proc_macro2::TokenStream>) {
    let included_merge = generate_included_merge_code(included_fields);
    let excluded_merge = generate_excluded_merge_code(fields);
    (included_merge, excluded_merge)
}

fn generate_included_merge_code(
    included_fields: &[&syn::Field],
) -> Vec<proc_macro2::TokenStream> {
    included_fields
        .iter()
        .filter(|field| !attribute_parser::get_crudcrate_bool(field, "non_db_attr").unwrap_or(false))
        .map(|field| {
            let ident = &field.ident;
            let is_optional = field_analyzer::field_is_optional(field);

            if is_optional {
                quote! {
                    model.#ident = match self.#ident {
                        Some(Some(value)) => sea_orm::ActiveValue::Set(Some(value.into())),
                        Some(None)      => sea_orm::ActiveValue::Set(None),
                        None            => sea_orm::ActiveValue::NotSet,
                    };
                }
            } else {
                quote! {
                    model.#ident = match self.#ident {
                        Some(Some(value)) => sea_orm::ActiveValue::Set(value.into()),
                        Some(None) => {
                            return Err(sea_orm::DbErr::Custom(format!(
                                "Field '{}' is required and cannot be set to null",
                                stringify!(#ident)
                            )));
                        },
                        None => sea_orm::ActiveValue::NotSet,
                    };
                }
            }
        })
        .collect()
}

fn generate_excluded_merge_code(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| {
            attribute_parser::get_crudcrate_bool(field, "update_model") == Some(false)
                && !attribute_parser::get_crudcrate_bool(field, "non_db_attr").unwrap_or(false)
        })
        .filter_map(|field| {
            if let Some(expr) = attribute_parser::get_crudcrate_expr(field, "on_update") {
                let ident = &field.ident;
                if field_analyzer::field_is_optional(field) {
                    Some(quote! {
                        model.#ident = sea_orm::ActiveValue::Set(Some((#expr).into()));
                    })
                } else {
                    Some(quote! {
                        model.#ident = sea_orm::ActiveValue::Set((#expr).into());
                    })
                }
            } else {
                None
            }
        })
        .collect()
}

fn extract_entity_fields(
    input: &DeriveInput,
) -> Result<&syn::punctuated::Punctuated<syn::Field, syn::token::Comma>, TokenStream> {
    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => Ok(&fields.named),
            _ => Err(syn::Error::new_spanned(
                input,
                "EntityToModels only supports structs with named fields",
            )
            .to_compile_error()
            .into()),
        },
        _ => Err(
            syn::Error::new_spanned(input, "EntityToModels only supports structs")
                .to_compile_error()
                .into(),
        ),
    }
}

fn parse_entity_attributes(
    input: &DeriveInput,
    struct_name: &syn::Ident,
) -> (syn::Ident, String) {
    let mut api_struct_name = None;
    let mut active_model_path = None;

    for attr in &input.attrs {
        if attr.path().is_ident("crudcrate")
            && let Meta::List(meta_list) = &attr.meta
                && let Ok(metas) =
                    Punctuated::<Meta, Comma>::parse_terminated.parse2(meta_list.tokens.clone())
                {
                    for meta in &metas {
                        if let Meta::NameValue(nv) = meta {
                            if nv.path.is_ident("api_struct") {
                                if let syn::Expr::Lit(expr_lit) = &nv.value
                                    && let Lit::Str(s) = &expr_lit.lit {
                                        api_struct_name = Some(format_ident!("{}", s.value()));
                                    }
                            } else if nv.path.is_ident("active_model")
                                && let syn::Expr::Lit(expr_lit) = &nv.value
                                    && let Lit::Str(s) = &expr_lit.lit {
                                        active_model_path = Some(s.value());
                                    }
                        }
                    }
                }
    }

    let table_name = attribute_parser::extract_table_name(&input.attrs).unwrap_or_else(|| struct_name.to_string());
    let api_struct_name =
        api_struct_name.unwrap_or_else(|| format_ident!("{}", table_name.to_pascal_case()));
    let active_model_path = active_model_path.unwrap_or_else(|| "ActiveModel".to_string());

    (api_struct_name, active_model_path)
}

fn analyze_entity_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> EntityFieldAnalysis<'_> {
    let mut analysis = EntityFieldAnalysis {
        db_fields: Vec::new(),
        non_db_fields: Vec::new(),
        primary_key_field: None,
        sortable_fields: Vec::new(),
        filterable_fields: Vec::new(),
        fulltext_fields: Vec::new(),
        join_on_one_fields: Vec::new(),
        join_on_all_fields: Vec::new(),
        // join_configs: std::collections::HashMap::new(), // Removed due to HashMap key issues
    };

    for field in fields {
        let is_non_db = attribute_parser::get_crudcrate_bool(field, "non_db_attr").unwrap_or(false);
        
        // Check for join attributes regardless of db/non_db status
        if let Some(join_config) = attribute_parser::get_join_config(field) {
            if join_config.on_one {
                analysis.join_on_one_fields.push(field);
            }
            if join_config.on_all {
                analysis.join_on_all_fields.push(field);
            }
            // Note: join_configs removed to avoid HashMap key issues with syn::Field
        }
        
        if is_non_db {
            analysis.non_db_fields.push(field);
        } else {
            analysis.db_fields.push(field);

            if attribute_parser::field_has_crudcrate_flag(field, "primary_key") {
                analysis.primary_key_field = Some(field);
            }
            if attribute_parser::field_has_crudcrate_flag(field, "sortable") {
                analysis.sortable_fields.push(field);
            }
            if attribute_parser::field_has_crudcrate_flag(field, "filterable") {
                analysis.filterable_fields.push(field);
            }
            if attribute_parser::field_has_crudcrate_flag(field, "fulltext") {
                analysis.fulltext_fields.push(field);
            }
        }
    }

    analysis
}

fn validate_field_analysis(analysis: &EntityFieldAnalysis) -> Result<(), TokenStream> {
    if analysis.primary_key_field.is_some()
        && analysis
            .db_fields
            .iter()
            .filter(|field| attribute_parser::field_has_crudcrate_flag(field, "primary_key"))
            .count()
            > 1
    {
        return Err(syn::Error::new_spanned(
            analysis.primary_key_field.unwrap(),
            "Only one field can be marked with 'primary_key' attribute",
        )
        .to_compile_error()
        .into());
    }
    Ok(())
}

fn generate_api_struct_content(
    analysis: &EntityFieldAnalysis,
) -> (Vec<proc_macro2::TokenStream>, Vec<proc_macro2::TokenStream>) {
    let mut api_struct_fields = Vec::new();
    let mut from_model_assignments = Vec::new();

    for field in &analysis.db_fields {
        let field_name = &field.ident;
        let field_type = &field.ty;

        // Check if field is excluded from the main API response (one model)
        // But don't exclude it if it has on_create or on_update expressions (needed for Create/Update models)
        let has_on_create = attribute_parser::get_crudcrate_expr(field, "on_create").is_some();
        let has_on_update = attribute_parser::get_crudcrate_expr(field, "on_update").is_some();

        if attribute_parser::get_crudcrate_bool(field, "one_model") == Some(false)
            && !has_on_create
            && !has_on_update {
            continue; // Skip this field - it's excluded from the get_one response and not needed for Create/Update models
        }

        let api_field_attrs: Vec<_> = field
            .attrs
            .iter()
            .filter(|attr| !attr.path().is_ident("sea_orm"))
            .collect();

        api_struct_fields.push(quote! {
            #(#api_field_attrs)*
            pub #field_name: #field_type
        });

        // Only include this field in the From<Model> assignment if it's not excluded from one_model
        if attribute_parser::get_crudcrate_bool(field, "one_model") != Some(false) {
            let assignment = if field_type
                .to_token_stream()
                .to_string()
                .contains("DateTimeWithTimeZone")
            {
                if field_analyzer::field_is_optional(field) {
                    quote! {
                        #field_name: model.#field_name.map(|dt| dt.with_timezone(&chrono::Utc))
                    }
                } else {
                    quote! {
                        #field_name: model.#field_name.with_timezone(&chrono::Utc)
                    }
                }
            } else {
                quote! {
                    #field_name: model.#field_name
                }
            };

            from_model_assignments.push(assignment);
        } else {
            // Field is excluded from one_model but included in struct (has on_create/on_update)
            // Provide a default value - for timestamp fields, use a reasonable default
            if field_type.to_token_stream().to_string().contains("DateTime") {
                from_model_assignments.push(quote! {
                    #field_name: model.#field_name
                });
            } else {
                from_model_assignments.push(quote! {
                    #field_name: Default::default()
                });
            }
        }
    }

    for field in &analysis.non_db_fields {
        let field_name = &field.ident;
        let field_type = &field.ty;

        // Check if field is excluded from the main API response (one model)
        if attribute_parser::get_crudcrate_bool(field, "one_model") == Some(false) {
            continue; // Skip this field - it's excluded from the get_one response
        }

        let default_expr = attribute_parser::get_crudcrate_expr(field, "default")
            .unwrap_or_else(|| syn::parse_quote!(Default::default()));

        // Preserve all original crudcrate attributes while ensuring required ones are present
        let crudcrate_attrs: Vec<_> = field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("crudcrate"))
            .collect();

        api_struct_fields.push(quote! {
            #(#crudcrate_attrs)*
            pub #field_name: #field_type
        });

        from_model_assignments.push(quote! {
            #field_name: #default_expr
        });
    }

    (api_struct_fields, from_model_assignments)
}

fn generate_api_struct(
    api_struct_name: &syn::Ident,
    api_struct_fields: &[proc_macro2::TokenStream],
    active_model_path: &str,
    crud_meta: &structs::CRUDResourceMeta,
    analysis: &EntityFieldAnalysis,
) -> proc_macro2::TokenStream {
    // Check if we have fields excluded from create/update models
    let _has_create_exclusions = analysis.db_fields.iter()
        .chain(analysis.non_db_fields.iter())
        .any(|field| attribute_parser::get_crudcrate_bool(field, "create_model") == Some(false));
    let _has_update_exclusions = analysis.db_fields.iter()
        .chain(analysis.non_db_fields.iter())
        .any(|field| attribute_parser::get_crudcrate_bool(field, "update_model") == Some(false));

    // Build derive clause based on user preferences
    let mut derives = vec![
        quote!(Clone),
        quote!(Debug),
        quote!(ToSchema),
        quote!(Serialize),
        quote!(Deserialize),
        quote!(ToCreateModel),
        quote!(ToUpdateModel),
    ];
    
    if crud_meta.derive_partial_eq {
        derives.push(quote!(PartialEq));
    }
    
    if crud_meta.derive_eq {
        derives.push(quote!(Eq));
    }
    
    quote! {
        use sea_orm::ActiveValue;
        use utoipa::ToSchema;
        use serde::{Serialize, Deserialize};
        use crudcrate::{ToUpdateModel, ToCreateModel};

        #[derive(#(#derives),*)]
        #[active_model = #active_model_path]
        pub struct #api_struct_name {
            #(#api_struct_fields),*
        }
    }
}

fn generate_from_impl(
    struct_name: &syn::Ident,
    api_struct_name: &syn::Ident,
    from_model_assignments: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    quote! {
        impl From<#struct_name> for #api_struct_name {
            fn from(model: #struct_name) -> Self {
                Self {
                    #(#from_model_assignments),*
                }
            }
        }
    }
}

fn generate_conditional_crud_impl(
    api_struct_name: &syn::Ident,
    crud_meta: &CRUDResourceMeta,
    active_model_path: &str,
    analysis: &EntityFieldAnalysis,
    table_name: &str,
) -> proc_macro2::TokenStream {
    let has_crud_resource_fields = analysis.primary_key_field.is_some()
        || !analysis.sortable_fields.is_empty()
        || !analysis.filterable_fields.is_empty()
        || !analysis.fulltext_fields.is_empty();

    let crud_impl = if has_crud_resource_fields {
        macro_implementation::generate_crud_resource_impl(api_struct_name, crud_meta, active_model_path, analysis, table_name)
    } else {
        quote! {}
    };

    let router_impl = if crud_meta.generate_router && has_crud_resource_fields {
        macro_implementation::generate_router_impl(api_struct_name)
    } else {
        quote! {}
    };

    quote! {
        #crud_impl
        #router_impl
    }
}


/// ===================
/// `ToCreateModel` Macro
/// ===================
/// This macro:
/// 1. Generates a struct named `<OriginalName>Create` that includes only the fields
///    where `#[crudcrate(create_model = false)]` is NOT specified (default = true).
///    If a field has an `on_create` expression, its type becomes `Option<…>`
///    (with `#[serde(default)]`) so the user can override that default.
/// 2. Generates an `impl From<<OriginalName>Create> for <ActiveModelType>>` where:
///    - For each field with `on_create`:
///       - If the original type was `Option<T>`, then `create.<field>` is `Option<Option<T>>`.
///         We match on that and do:
///           ```rust,ignore
///           match create.field {
///             Some(Some(v)) => Some(v.into()),      // user overrode with T
///             Some(None)    => None,                // user explicitly set null
///             None          => Some((expr).into()), // fallback to expr
///           }
///           ```
///       - If the original type was non‐optional `T`, then `create.<field>` is `Option<T>`.
///         We match on that and do:
///           ```rust,ignore
///           match create.field {
///             Some(v) => v.into(),
///             None    => (expr).into(),
///           }
///           ```
///    - For each field without `on_create`:
///       - If the original type was `Option<T>`, we do `create.<field>.map(|v| v.into())`.
///       - If it was non‐optional `T`, we do `create.<field>.into()`.
///    - For any field excluded (`create_model = false`) but having `on_create`, we do
///      `Some((expr).into())` if it was `Option<T>`, or just `(expr).into()` otherwise.
#[proc_macro_derive(ToCreateModel, attributes(crudcrate, active_model))]
pub fn to_create_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let create_name = format_ident!("{}Create", name);

    let active_model_type = extract_active_model_type(&input, name);
    let fields = extract_named_fields(&input);
    let create_struct_fields = macro_implementation::generate_create_struct_fields(&fields);
    let conv_lines = macro_implementation::generate_create_conversion_lines(&fields);

    let expanded = quote! {
        #[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
        pub struct #create_name {
            #(#create_struct_fields),*
        }

        impl From<#create_name> for #active_model_type {
            fn from(create: #create_name) -> Self {
                #active_model_type {
                    #(#conv_lines),*
                }
            }
        }
    };

    #[cfg(feature = "debug")]
    debug_output::print_create_model_debug(&expanded, &name.to_string());

    TokenStream::from(expanded)
}

/// ===================
/// `ToUpdateModel` Macro
/// ===================
/// This macro:
/// 1. Generates a struct named `<OriginalName>Update` that includes only the fields
///    where `#[crudcrate(update_model = false)]` is NOT specified (default = true).
/// 2. Generates an impl for a method
///    `merge_into_activemodel(self, mut model: ActiveModelType) -> ActiveModelType`
///    that, for each field:
///    - If it's included in the update struct, and the user provided a value:
///       - If the original field type was `Option<T>`, we match on
///         `Option<Option<T>>`:
///           ```rust,ignore
///           Some(Some(v)) => ActiveValue::Set(Some(v.into())),
///           Some(None)    => ActiveValue::Set(None),     // explicit set to None
///           None          => ActiveValue::NotSet,       // no change
///           ```  
///       - If the original field type was non‐optional `T`, we match on `Option<T>`:
///           ```rust,ignore
///           Some(val) => ActiveValue::Set(val.into()),
///           _         => ActiveValue::NotSet,
///           ```  
///    - If it's excluded (`update_model = false`) but has `on_update = expr`, we do
///      `ActiveValue::Set(expr.into())` (wrapped in `Some(...)` if the original field was `Option<T>`).
///    - All other fields remain unchanged.
#[proc_macro_derive(ToUpdateModel, attributes(crudcrate, active_model))]
pub fn to_update_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let update_name = format_ident!("{}Update", name);

    let active_model_type = extract_active_model_type(&input, name);
    let fields = extract_named_fields(&input);
    let included_fields = macro_implementation::filter_update_fields(&fields);
    let update_struct_fields = macro_implementation::generate_update_struct_fields(&included_fields);
    let (included_merge, excluded_merge) =
        generate_update_merge_code(&fields, &included_fields);

    let expanded = quote! {
        #[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
        pub struct #update_name {
            #(#update_struct_fields),*
        }

        impl #update_name {
            pub fn merge_fields(self, mut model: #active_model_type) -> Result<#active_model_type, sea_orm::DbErr> {
                #(#included_merge)*
                #(#excluded_merge)*
                Ok(model)
            }
        }

        impl crudcrate::traits::MergeIntoActiveModel<#active_model_type> for #update_name {
            fn merge_into_activemodel(self, model: #active_model_type) -> Result<#active_model_type, sea_orm::DbErr> {
                Self::merge_fields(self, model)
            }
        }
    };

    #[cfg(feature = "debug")]
    debug_output::print_update_model_debug(&expanded, &name.to_string());

    TokenStream::from(expanded)
}

/// ===================
/// `ToListModel` Macro
/// ===================
/// This macro generates a struct named `<OriginalName>List` that includes only the fields
/// where `#[crudcrate(list_model = false)]` is NOT specified (default = true).
/// This allows creating optimized list views by excluding heavy fields like relationships,
/// large text fields, or computed properties from collection endpoints.
///
/// Generated struct:
/// ```rust,ignore
/// #[derive(Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
/// pub struct <OriginalName>List {
///     // All fields where list_model != false
///     pub field_name: FieldType,
/// }
///
/// impl From<Model> for <OriginalName>List {
///     fn from(model: Model) -> Self {
///         Self {
///             field_name: model.field_name,
///             // ... other included fields
///         }
///     }
/// }
/// ```
///
/// Usage:
/// ```rust,ignore
/// pub struct Model {
///     pub id: Uuid,
///     pub name: String,
///     #[crudcrate(list_model = false)]  // Exclude from list view
///     pub large_description: Option<String>,
///     #[crudcrate(list_model = false)]  // Exclude relationships from list
///     pub related_items: Vec<RelatedItem>,
/// }
/// ```
#[proc_macro_derive(ToListModel, attributes(crudcrate))]
pub fn to_list_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let list_name = format_ident!("{}List", name);

    let fields = extract_named_fields(&input);
    let list_struct_fields = macro_implementation::generate_list_struct_fields(&fields);
    let list_from_assignments = macro_implementation::generate_list_from_assignments(&fields);

    let expanded = quote! {
        #[derive(Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
        pub struct #list_name {
            #(#list_struct_fields),*
        }

        impl From<#name> for #list_name {
            fn from(model: #name) -> Self {
                Self {
                    #(#list_from_assignments),*
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// =====================
/// `EntityToModels` Macro
/// =====================
/// This macro generates an API struct from a Sea-ORM entity Model struct, along with
/// `ToCreateModel` and `ToUpdateModel` implementations.
///
/// ## Available Struct-Level Attributes
///
/// ```rust,ignore
/// #[crudcrate(
///     api_struct = "TodoItem",              // Override API struct name
///     active_model = "ActiveModel",         // Override ActiveModel path  
///     name_singular = "todo",               // Resource name (singular)
///     name_plural = "todos",                // Resource name (plural)
///     description = "Manages todo items",   // Resource description
///     entity_type = "Entity",               // Entity type for CRUDResource
///     column_type = "Column",               // Column type for CRUDResource
///     fn_get_one = self::custom_get_one,    // Custom get_one function
///     fn_get_all = self::custom_get_all,    // Custom get_all function
///     fn_create = self::custom_create,      // Custom create function
///     fn_update = self::custom_update,      // Custom update function
///     fn_delete = self::custom_delete,      // Custom delete function
///     fn_delete_many = self::custom_delete_many, // Custom delete_many function
/// )]
/// ```
///
/// ## Available Field-Level Attributes
///
/// ```rust,ignore
/// #[crudcrate(
///     primary_key,                          // Mark as primary key
///     sortable,                             // Include in sortable columns
///     filterable,                           // Include in filterable columns
///     create_model = false,                 // Exclude from Create model
///     update_model = false,                 // Exclude from Update model
///     on_create = Uuid::new_v4(),          // Auto-generate on create
///     on_update = chrono::Utc::now(),      // Auto-update on update
///     non_db_attr = true,                  // Non-database field
///     default = vec![],                    // Default for non-DB fields
///     use_target_models,                   // Use target's Create/Update models for relationships
/// )]
/// ```
///
/// Usage:
/// ```ignore
/// use uuid::Uuid;
/// #[derive(EntityToModels)]
/// #[crudcrate(api_struct = "Experiment", active_model = "spice_entity::experiments::ActiveModel")]
/// pub struct Model {
///     #[crudcrate(update_model = false, create_model = false, on_create = Uuid::new_v4())]
///     pub id: Uuid,
///     pub name: String,
///     #[crudcrate(non_db_attr = true, default = vec![])]
///     pub regions: Vec<RegionInput>,
/// }
/// ```
///
/// This generates:
/// - An API struct with the specified name (e.g., `Experiment`)
/// - `ToCreateModel` and `ToUpdateModel` implementations
/// - `From<Model>` implementation for the API struct
/// - Support for non-db attributes
///
/// Derive macro for generating complete CRUD API structures from Sea-ORM entities.
///
/// # Struct-Level Attributes (all optional)
///
/// **Boolean Flags** (can be used as just `flag` or `flag = true/false`):
/// - `generate_router` - Auto-generate Axum router with all CRUD endpoints
/// - `debug_output` - Print generated code to console (requires `--features debug`)
///
/// **Named Parameters**:
/// - `api_struct = "Name"` - Override API struct name (default: table name in `PascalCase`)
/// - `active_model = "Path"` - Override `ActiveModel` path (default: `ActiveModel`)
/// - `name_singular = "name"` - Resource singular name (default: table name)
/// - `name_plural = "names"` - Resource plural name (default: singular + "s")
/// - `description = "desc"` - Resource description for documentation
/// - `entity_type = "Entity"` - Entity type for `CRUDResource` (default: "Entity")
/// - `column_type = "Column"` - Column type for `CRUDResource` (default: "Column")
/// - `fulltext_language = "english"` - Default language for full-text search
///
/// **Function Overrides** (for custom CRUD behavior):
/// - `fn_get_one = path::to::function` - Custom `get_one` function override
/// - `fn_get_all = path::to::function` - Custom `get_all` function override
/// - `fn_create = path::to::function` - Custom create function override
/// - `fn_update = path::to::function` - Custom update function override
/// - `fn_delete = path::to::function` - Custom delete function override
/// - `fn_delete_many = path::to::function` - Custom `delete_many` function override
///
/// # Field-Level Attributes
///
/// **Boolean Flags** (can be used as just `flag` or `flag = true/false`):
/// - `primary_key` - Mark field as primary key (only one allowed)
/// - `sortable` - Include field in `sortable_columns()`
/// - `filterable` - Include field in `filterable_columns()`
/// - `fulltext` - Enable full-text search for this field
/// - `non_db_attr` - Field is not in database, won't appear in DB operations
/// - `use_target_models` - Use target's Create/Update models instead of full entity model
///
/// **Named Parameters**:
/// - `create_model = false` - Exclude from Create model (default: true)
/// - `update_model = false` - Exclude from Update model (default: true)  
/// - `list_model = false` - Exclude from List model (default: true)
/// - `on_create = expression` - Auto-generate value on create (e.g., `Uuid::new_v4()`)
/// - `on_update = expression` - Auto-generate value on update (e.g., `Utc::now()`)
/// - `default = expression` - Default value for non-DB fields
/// - `fulltext_language = "english"` - Language for full-text search
///
/// **Model Exclusion** (Rust-idiomatic alternative to negative boolean flags):
/// - `exclude(create)` - Exclude from Create model (same as `create_model = false`)
/// - `exclude(update)` - Exclude from Update model (same as `update_model = false`)
/// - `exclude(list)` - Exclude from List model (same as `list_model = false`)
/// - `exclude(create, update)` - Exclude from multiple models
/// - `exclude(create, update, list)` - Exclude from all models
///
/// **Join Configuration** (for relationship loading):
/// - `join(one)` - Load this relationship in `get_one()` calls
/// - `join(all)` - Load this relationship in `get_all()` calls  
/// - `join(one, all)` - Load in both `get_one()` and `get_all()` calls
/// - `join(one, all, depth = 2)` - Recursive loading with specified depth
/// - `join(one, all, relation = "CustomRelation")` - Use custom Sea-ORM relation name
///
/// # Example
///
/// ```rust,ignore
/// use uuid::Uuid;
/// use crudcrate_derive::EntityToModels;
/// use sea_orm::prelude::*;
///
/// #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
/// #[sea_orm(table_name = "customers")]
/// #[crudcrate(api_struct = "Customer", generate_router)]
/// pub struct Model {
///     #[sea_orm(primary_key, auto_increment = false)]
///     #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
///     pub id: Uuid,
///     
///     #[crudcrate(sortable, filterable)]
///     pub name: String,
///     
///     #[crudcrate(filterable)]
///     pub email: String,
///     
///     #[crudcrate(sortable, exclude(create, update), on_create = Utc::now())]
///     pub created_at: DateTime<Utc>,
///     
///     #[crudcrate(sortable, exclude(create, update), on_create = Utc::now(), on_update = Utc::now())]
///     pub updated_at: DateTime<Utc>,
///     
///     // Join field - loads vehicles automatically with depth=3 recursive loading  
///     #[sea_orm(ignore)]
///     #[crudcrate(non_db_attr, join(one, all))]  // depth=3 by default
///     pub vehicles: Vec<Vehicle>,
/// }
///
/// #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
/// pub enum Relation {}
///
/// impl ActiveModelBehavior for ActiveModel {}
/// ```
/// 
/// # Panics
/// 
/// This function will panic in the following cases:
/// - When deprecated syntax is used (e.g., `create_model = false` instead of `exclude(create)`)
/// - When there are cyclic join dependencies without explicit depth specification
/// - When required Sea-ORM relation enums are missing for join fields
#[proc_macro_derive(EntityToModels, attributes(crudcrate))]
pub fn entity_to_models(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;
    let fields = match extract_entity_fields(&input) {
        Ok(f) => f,
        Err(e) => return e,
    };

    let (api_struct_name, active_model_path) =
        parse_entity_attributes(&input, struct_name);
    let table_name = attribute_parser::extract_table_name(&input.attrs).unwrap_or_else(|| struct_name.to_string());
    let crud_meta = match attribute_parser::parse_crud_resource_meta(&input.attrs) {
        Ok(meta) => meta.with_defaults(&table_name, &api_struct_name.to_string()),
        Err(e) => return e.to_compile_error().into(),
    };

    // Validate active model path
    if syn::parse_str::<syn::Type>(&active_model_path).is_err() {
        return syn::Error::new_spanned(
            &input,
            format!("Invalid active_model path: {active_model_path}"),
        )
        .to_compile_error()
        .into();
    }

    let field_analysis = analyze_entity_fields(fields);
    if let Err(e) = validate_field_analysis(&field_analysis) {
        return e;
    }

    // Generate compile-time validation for join relationships
    let join_validation = relation_validator::generate_join_relation_validation(&field_analysis);

    // Check for cyclic dependencies and emit compile-time error if detected
    let cyclic_dependency_check = relation_validator::generate_cyclic_dependency_check(&field_analysis, &api_struct_name.to_string());
    if !cyclic_dependency_check.is_empty() {
        return cyclic_dependency_check.into();
    }

    let (api_struct_fields, from_model_assignments) =
        generate_api_struct_content(&field_analysis);
    let api_struct =
        generate_api_struct(&api_struct_name, &api_struct_fields, &active_model_path, &crud_meta, &field_analysis);
    let from_impl =
        generate_from_impl(struct_name, &api_struct_name, &from_model_assignments);
    let crud_impl = generate_conditional_crud_impl(
        &api_struct_name,
        &crud_meta,
        &active_model_path,
        &field_analysis,
        &table_name,
    );
    
    // Generate List model struct and implementation
    let list_name = format_ident!("{}List", &api_struct_name);
    let raw_fields = extract_named_fields(&input);
    let list_struct_fields = macro_implementation::generate_list_struct_fields(&raw_fields);
    let list_from_assignments = macro_implementation::generate_list_from_assignments(&raw_fields);
    let list_from_model_assignments = macro_implementation::generate_list_from_model_assignments(&field_analysis);
    
    let list_model = quote! {
        #[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
        pub struct #list_name {
            #(#list_struct_fields),*
        }

        impl From<#api_struct_name> for #list_name {
            fn from(model: #api_struct_name) -> Self {
                Self {
                    #(#list_from_assignments),*
                }
            }
        }

        impl From<#struct_name> for #list_name {
            fn from(model: #struct_name) -> Self {
                Self {
                    #(#list_from_model_assignments),*
                }
            }
        }
    };

    let expanded = quote! {
        #api_struct
        #from_impl
        #crud_impl
        #list_model
        #join_validation
    };

    // Print debug output if requested (either via attribute or cargo feature)
    #[cfg(feature = "debug")]
    if crud_meta.debug_output {
        debug_output::print_debug_output(&expanded, &api_struct_name.to_string());
    }

    TokenStream::from(expanded)
}
