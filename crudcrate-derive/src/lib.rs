mod helpers;
mod structs;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, parse_macro_input};

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

    let active_model_type = helpers::extract_active_model_type(&input, name);
    let fields = helpers::extract_named_fields(&input);
    let create_struct_fields = helpers::generate_create_struct_fields(&fields);
    let conv_lines = helpers::generate_create_conversion_lines(&fields);

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

    let active_model_type = helpers::extract_active_model_type(&input, name);
    let fields = helpers::extract_named_fields(&input);
    let included_fields = helpers::filter_update_fields(&fields);
    let update_struct_fields = helpers::generate_update_struct_fields(&included_fields);
    let (included_merge, excluded_merge) =
        helpers::generate_update_merge_code(&fields, &included_fields);

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

    let fields = helpers::extract_named_fields(&input);
    let list_struct_fields = helpers::generate_list_struct_fields(&fields);
    let list_from_assignments = helpers::generate_list_from_assignments(&fields);

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
/// - From<Model> implementation for the API struct
/// - Support for non-db attributes
///
/// Derive macro for generating complete CRUD API structures from Sea-ORM entities.
///
/// # Struct-Level Attributes (all optional)
///
/// - `api_struct = "Name"` - Override API struct name (default: table name in `PascalCase`)
/// - `active_model = "Path"` - Override `ActiveModel` path (default: `ActiveModel`)
/// - `name_singular = "name"` - Resource singular name (default: table name)
/// - `name_plural = "names"` - Resource plural name (default: singular + "s")
/// - `description = "desc"` - Resource description for documentation
/// - `entity_type = "Entity"` - Entity type for `CRUDResource` (default: "Entity")
/// - `column_type = "Column"` - Column type for `CRUDResource` (default: "Column")
/// - `fn_get_one = path::to::function` - Custom `get_one` function override
/// - `fn_get_all = path::to::function` - Custom `get_all` function override
/// - `fn_create = path::to::function` - Custom create function override
/// - `fn_update = path::to::function` - Custom update function override
/// - `fn_delete = path::to::function` - Custom delete function override
/// - `fn_delete_many = path::to::function` - Custom `delete_many` function override
///
/// # Field-Level Attributes
///
/// - `primary_key` - Mark field as primary key (only one allowed)
/// - `sortable` - Include field in `sortable_columns()`
/// - `filterable` - Include field in `filterable_columns()`
/// - `create_model = false` - Exclude from Create model (default: true)
/// - `update_model = false` - Exclude from Update model (default: true)  
/// - `on_create = expression` - Auto-generate value on create
/// - `on_update = expression` - Auto-generate value on update
/// - `non_db_attr = true` - Field is not in database (default: false)
/// - `default = expression` - Default value for non-DB fields
/// - `use_target_models` - Use target's Create/Update models instead of full entity model
///
/// # Example
///
/// ```rust,ignore
/// use uuid::Uuid;
/// use crudcrate_derive::EntityToModels;
/// use sea_orm::prelude::*;
///
/// #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, EntityToModels)]
/// #[sea_orm(table_name = "todos")]
/// #[crudcrate(api_struct = "Todo", description = "Todo items")]
/// pub struct Model {
///     #[sea_orm(primary_key, auto_increment = false)]
///     #[crudcrate(primary_key, sortable, create_model = false, update_model = false, on_create = Uuid::new_v4())]
///     pub id: Uuid,
///     
///     #[crudcrate(sortable, filterable)]
///     pub title: String,
///     
///     #[crudcrate(filterable, on_create = false)]
///     pub completed: bool,
/// }
///
/// #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
/// pub enum Relation {}
///
/// impl ActiveModelBehavior for ActiveModel {}
/// ```
#[proc_macro_derive(EntityToModels, attributes(crudcrate))]
pub fn entity_to_models(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;
    let fields = match helpers::extract_entity_fields(&input) {
        Ok(f) => f,
        Err(e) => return e,
    };

    let (api_struct_name, active_model_path) =
        helpers::parse_entity_attributes(&input, struct_name);
    let table_name = helpers::extract_table_name(&input.attrs).unwrap_or_else(|| struct_name.to_string());
    let crud_meta = helpers::parse_crud_resource_meta(&input.attrs).with_defaults(
        &table_name,
        &api_struct_name.to_string(),
    );

    // Validate active model path
    if syn::parse_str::<syn::Type>(&active_model_path).is_err() {
        return syn::Error::new_spanned(
            &input,
            format!("Invalid active_model path: {active_model_path}"),
        )
        .to_compile_error()
        .into();
    }

    let field_analysis = helpers::analyze_entity_fields(fields);
    if let Err(e) = helpers::validate_field_analysis(&field_analysis) {
        return e;
    }

    let (api_struct_fields, from_model_assignments) =
        helpers::generate_api_struct_content(&field_analysis);
    let api_struct =
        helpers::generate_api_struct(&api_struct_name, &api_struct_fields, &active_model_path);
    let from_impl =
        helpers::generate_from_impl(struct_name, &api_struct_name, &from_model_assignments);
    let crud_impl = helpers::generate_conditional_crud_impl(
        &api_struct_name,
        &crud_meta,
        &active_model_path,
        &field_analysis,
        &table_name,
    );
    
    // Generate List model struct and implementation
    let list_name = format_ident!("{}List", &api_struct_name);
    let raw_fields = helpers::extract_named_fields(&input);
    let list_struct_fields = helpers::generate_list_struct_fields(&raw_fields);
    let list_from_assignments = helpers::generate_list_from_assignments(&raw_fields);
    let list_from_model_assignments = helpers::generate_list_from_model_assignments(&field_analysis);
    
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
    };

    TokenStream::from(expanded)
}
