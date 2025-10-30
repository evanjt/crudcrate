mod structs;
mod attribute_parser;
mod field_analyzer;
mod macro_implementation;
// mod join_generators; // Removed - functions moved to macro_implementation.rs
mod relation_validator;
mod attributes;
mod two_pass_generator;
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

/// Helper function to resolve inner types (Vec<Model>, Option<Model>, Model) to API structs
/// Vec<super::vehicle::Model> -> Vec<Vehicle>
/// Option<super::customer::Model> -> Option<Customer>
/// super::vehicle::Model -> Vehicle
fn resolve_inner_type_to_api_struct(field_type: &syn::Type) -> proc_macro2::TokenStream {
    // Check if this is a container type (Vec, Option) with inner Model type
    if let syn::Type::Path(type_path) = field_type {
        if let Some(segment) = type_path.path.segments.last() {
            // Handle Vec<T> and Option<T>
            if (segment.ident == "Vec" || segment.ident == "Option") {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        // Recursively resolve the inner type
                        let resolved_inner = resolve_base_model_to_api_struct(inner_ty);

                        if segment.ident == "Vec" {
                            return quote! { Vec<#resolved_inner> };
                        } else {
                            return quote! { Option<#resolved_inner> };
                        }
                    }
                }
            }
        }
    }

    // Direct Model type - resolve it
    resolve_base_model_to_api_struct(field_type)
}

/// Helper to resolve a base Model type to its API struct name
/// super::vehicle::Model -> Vehicle
/// vehicle_part::Model -> VehiclePart
fn resolve_base_model_to_api_struct(field_type: &syn::Type) -> proc_macro2::TokenStream {
    // First try: use global registry
    if let Some(base_type_str) = two_pass_generator::extract_base_type_string(field_type) {
        if let Some(api_name) = two_pass_generator::find_api_struct_name(&base_type_str) {
            let api_struct_ident = quote::format_ident!("{}", api_name);
            return quote! { #api_struct_ident };
        }
    }

    // Second try: extract module name from path and use naming convention
    // super::vehicle_part::Model -> VehiclePart (convert module snake_case to PascalCase)
    if let syn::Type::Path(type_path) = field_type {
        let segments: Vec<_> = type_path.path.segments.iter().collect();

        // Look for pattern: [any segments]::module_name::Model
        if segments.len() >= 2 {
            let last_seg = &segments[segments.len() - 1];
            let module_seg = &segments[segments.len() - 2];

            if last_seg.ident == "Model" {
                // Convert snake_case module name to PascalCase
                let module_name = module_seg.ident.to_string();
                let pascal_case = module_name
                    .split('_')
                    .map(|s| {
                        let mut chars = s.chars();
                        match chars.next() {
                            Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                            None => String::new(),
                        }
                    })
                    .collect::<String>();

                let api_struct_ident = quote::format_ident!("{}", pascal_case);

                #[cfg(feature = "debug")]
                eprintln!("DEBUG: Inferred API struct name from path: {} -> {}", module_name, pascal_case);

                return quote! { #api_struct_ident };
            }
        }
    }

    // Fallback: keep original type if we can't resolve
    #[cfg(feature = "debug")]
    eprintln!("WARNING: Could not resolve Model type to API struct: {:?}", quote! { #field_type });
    quote! { #field_type }
}

/// Resolve join field type - extract inner type from JoinField and transform Model -> API struct
/// This is used for API struct field definitions for join fields
/// JoinField<Vec<super::vehicle::Model>> -> Vec<Vehicle> (unwrap JoinField and resolve Model to API struct)
/// Vec<super::vehicle::Model> -> Vec<Vehicle> (resolve directly, no JoinField wrapper needed)
fn resolve_join_field_type_preserving_container(field_type: &syn::Type) -> proc_macro2::TokenStream {
    // Check if the field type is already JoinField<...>
    if let syn::Type::Path(type_path) = field_type {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "JoinField" {
                // Extract inner type from JoinField<T> and resolve it
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        // Recursively resolve the inner type (Vec<Model> -> Vec<APIStruct>)
                        // NO LONGER wrap in JoinField - use direct types with #[schema(no_recursion)]
                        let resolved_inner = resolve_inner_type_to_api_struct(inner_ty);
                        #[cfg(feature = "debug")]
                        eprintln!("DEBUG: JoinField inner type resolved (unwrapped): {:?} -> {:?}", quote! { #inner_ty }, quote! { #resolved_inner });
                        return resolved_inner;
                    }
                }
                // Couldn't extract inner type, keep as-is
                #[cfg(feature = "debug")]
                eprintln!("DEBUG: Field already JoinField, but couldn't extract inner type: {:?}", quote! { #field_type });
                return quote! { #field_type };
            }
        }
    }

    // Try to extract base type from the field type using the global registry
    if let Some(base_type_str) = two_pass_generator::extract_base_type_string(field_type) {
        // Look up the API struct name for this base type
        if let Some(api_name) = two_pass_generator::find_api_struct_name(&base_type_str) {
            let api_struct_ident = quote::format_ident!("{}", api_name);

            // Check if this is a Vec<T> to preserve collection structure
            if let syn::Type::Path(type_path) = field_type {
                if let Some(segment) = type_path.path.segments.last() {
                    if segment.ident == "Vec" {
                        // Vec<super::vehicle::Model> -> Vec<Vehicle> (NO JoinField wrapper)
                        return quote! { Vec<#api_struct_ident> };
                    } else if segment.ident == "Option" {
                        // Option<super::vehicle::Model> -> Option<Vehicle> (NO JoinField wrapper)
                        return quote! { Option<#api_struct_ident> };
                    }
                }
            }

            // Direct type: super::vehicle::Model -> Vehicle (NO JoinField wrapper)
            return quote! { #api_struct_ident };
        }
    }

    // Fallback: if we can't resolve, keep the original type
    #[cfg(feature = "debug")]
    eprintln!("WARNING: Could not resolve join field type, keeping as-is: {:?}", quote! { #field_type });

    quote! { #field_type }
}

fn generate_api_struct_content(
    analysis: &EntityFieldAnalysis,
    api_struct_name: &syn::Ident,
) -> (Vec<proc_macro2::TokenStream>, Vec<proc_macro2::TokenStream>, std::collections::HashSet<String>) {
    let mut api_struct_fields = Vec::new();
    let mut from_model_assignments = Vec::new();
    let mut required_imports = std::collections::HashSet::new();

    for field in &analysis.db_fields {
        let field_name = &field.ident;
        let field_type = &field.ty;

        let api_field_attrs: Vec<_> = field
            .attrs
            .iter()
            .filter(|attr| !attr.path().is_ident("sea_orm"))
            .collect();

        api_struct_fields.push(quote! {
            #(#api_field_attrs)*
            pub #field_name: #field_type
        });

        // Also populate the From<Model> assignment for this field (since it exists in the struct)
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
    }

    for field in &analysis.non_db_fields {
        let field_name = &field.ident;
        let field_type = &field.ty;

        let default_expr = attribute_parser::get_crudcrate_expr(field, "default")
            .unwrap_or_else(|| syn::parse_quote!(Default::default()));

        // Preserve all original crudcrate attributes while ensuring required ones are present
        let crudcrate_attrs: Vec<_> = field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("crudcrate"))
            .collect();

        // Add schema(no_recursion) attribute for join fields to prevent circular dependencies
        // This is the proper utoipa way to handle recursive relationships
        let schema_attrs = if attribute_parser::get_join_config(field).is_some() {
            quote! { #[schema(no_recursion)] }
        } else {
            quote! {}
        };

        // For join fields, resolve type and wrap in JoinField<>
        let final_field_type = if attribute_parser::get_join_config(field).is_some() {
            // For join fields, keep the type as-is since user already declares it as JoinField<Vec<T>>
            // If they use the old Vec<super::vehicle::Model> syntax, resolve it
            resolve_join_field_type_preserving_container(field_type)
        } else {
            // For other non-db fields, use the field type as-is
            quote! { #field_type }
        };

        // Generate field definition with proper type handling
        #[cfg(feature = "debug")]
        eprintln!("DEBUG: API struct field '{}' type: {:?}",
            field_name.as_ref().map(|n| n.to_string()).unwrap_or_default(),
            quote! { #final_field_type });

        let field_definition = quote! {
            #schema_attrs
            #(#crudcrate_attrs)*
            pub #field_name: #final_field_type
        };

        api_struct_fields.push(field_definition);

        // For join fields, initialize with empty collection
        let assignment = if attribute_parser::get_join_config(field).is_some() {
            // For join fields, initialize with appropriate empty value:
            // - Vec<T> fields -> empty vec![]
            // - Option<T> fields -> None
            // - Direct T fields -> Default::default()
            // Join fields are populated by the CRUDResource::get_one() implementation, not by From<Model>
            // Use the RESOLVED type (final_field_type) to determine the empty value, not the original field_type
            let empty_value = if let Ok(resolved_type) = syn::parse2::<syn::Type>(quote! { #final_field_type }) {
                if let syn::Type::Path(type_path) = &resolved_type {
                    if let Some(segment) = type_path.path.segments.last() {
                        if segment.ident == "Vec" {
                            quote! { vec![] }
                        } else if segment.ident == "Option" {
                            quote! { None }
                        } else {
                            quote! { Default::default() }
                        }
                    } else {
                        quote! { Default::default() }
                    }
                } else {
                    quote! { Default::default() }
                }
            } else {
                quote! { Default::default() }
            };

            #[cfg(feature = "debug")]
            eprintln!("DEBUG: From<Model> assignment for join field '{}': {:?}",
                field_name.as_ref().map(|n| n.to_string()).unwrap_or_default(),
                quote! { #field_name: #empty_value });

            quote! {
                #field_name: #empty_value
            }
        } else {
            quote! {
                #field_name: #default_expr
            }
        };

        from_model_assignments.push(assignment);
    }

  
      #[cfg(feature = "debug")]
    eprintln!("DEBUG: required_imports = {:?}", required_imports);

    (api_struct_fields, from_model_assignments, required_imports)
}

/// Extract the target type from a join field (e.g., Vec<Vehicle> -> Vehicle)
fn extract_join_target_type(field_type: &syn::Type) -> Option<syn::Type> {
    match field_type {
        syn::Type::Path(type_path) => {
            if let Some(last_seg) = type_path.path.segments.last() {
                let ident = &last_seg.ident;

                // Handle Vec<T> - extract the inner type T
                if ident == "Vec"
                    && let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments
                    && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
                {
                    Some(inner_ty.clone())
                } else if ident == "Option"
                    && let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments
                    && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
                {
                    // For Option<T>, extract the inner type
                    extract_join_target_type(inner_ty)
                } else {
                    // For direct types (T), return the type as-is
                    Some(field_type.clone())
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

fn generate_api_struct(
    api_struct_name: &syn::Ident,
    api_struct_fields: &[proc_macro2::TokenStream],
    active_model_path: &str,
    crud_meta: &structs::CRUDResourceMeta,
    analysis: &EntityFieldAnalysis,
    required_imports: &std::collections::HashSet<String>,
) -> proc_macro2::TokenStream {
    // Check if we have fields excluded from create/update models
    let _has_create_exclusions = analysis.db_fields.iter()
        .chain(analysis.non_db_fields.iter())
        .any(|field| attribute_parser::get_crudcrate_bool(field, "create_model") == Some(false));
    let _has_update_exclusions = analysis.db_fields.iter()
        .chain(analysis.non_db_fields.iter())
        .any(|field| attribute_parser::get_crudcrate_bool(field, "update_model") == Some(false));

    // Check if we have join fields that require Default implementation
    let has_join_fields = !analysis.join_on_one_fields.is_empty() || !analysis.join_on_all_fields.is_empty();

    // Check if any non-db fields need Default (for join loading or excluded fields)
    let has_fields_needing_default = has_join_fields ||
        analysis.non_db_fields.iter().any(|field| {
            // Fields excluded from create/update need Default for join loading
            attribute_parser::get_crudcrate_bool(field, "create_model") == Some(false) ||
            attribute_parser::get_crudcrate_bool(field, "update_model") == Some(false)
        }) ||
        analysis.db_fields.iter().any(|field| {
            // Database fields excluded from create/update need Default
            attribute_parser::get_crudcrate_bool(field, "create_model") == Some(false) ||
            attribute_parser::get_crudcrate_bool(field, "update_model") == Some(false)
        });

    // Build derive clause based on user preferences
    let mut derives = vec![
        quote!(Clone),
        quote!(Debug),
        quote!(Serialize),
        quote!(Deserialize),
        quote!(ToCreateModel),
        quote!(ToUpdateModel),
    ];

    // Always include ToSchema, but handle circular dependencies with schema(no_recursion)
    // This is the proper utoipa approach for recursive relationships
    derives.push(quote!(ToSchema));

    #[cfg(feature = "debug")]
    eprintln!("DEBUG: Including ToSchema for '{}' (using schema(no_recursion) for join fields)", api_struct_name);

    // Add Default derive if needed for join fields or excluded fields
    // BUT: don't derive Default if we have join fields, as it causes E0282 type inference errors
    // We'll manually implement Default instead
    if has_fields_needing_default && !has_join_fields {
        derives.push(quote!(Default));
    }

    if crud_meta.derive_partial_eq {
        derives.push(quote!(PartialEq));
    }

    if crud_meta.derive_eq {
        derives.push(quote!(Eq));
    }
    
    // Collect import statements for join field target types
    // When we have join fields like `JoinField<Vec<super::vehicle::Model>>`,
    // we need to import the corresponding API struct (`super::vehicle::Vehicle`)
    let mut import_statements: Vec<proc_macro2::TokenStream> = vec![];
    let mut seen_imports: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Collect all join fields from the analysis
    let all_join_fields: Vec<_> = analysis.join_on_one_fields.iter().chain(analysis.join_on_all_fields.iter()).collect();

    #[cfg(feature = "debug")]
    eprintln!("DEBUG generate_api_struct: Found {} join fields", all_join_fields.len());

    for field in all_join_fields {
        #[cfg(feature = "debug")]
        {
            let field_ty = &field.ty;
            eprintln!("DEBUG generate_api_struct: Processing join field: {} with type: {}", field.ident.as_ref().unwrap(), quote!{#field_ty});
        }

        // Extract the base type from the join field (e.g., "Vehicle" from "JoinField<Vec<super::vehicle::Model>>")
        if let Some(base_type_str) = two_pass_generator::extract_base_type_string(&field.ty) {
            #[cfg(feature = "debug")]
            eprintln!("DEBUG generate_api_struct: Base type: {}", base_type_str);

            if let Some(api_name) = two_pass_generator::find_api_struct_name(&base_type_str) {
                #[cfg(feature = "debug")]
                eprintln!("DEBUG generate_api_struct: API name: {}", api_name);

                // The field type is JoinField<Vec<super::vehicle::Model>> or similar
                // We need to extract the inner-most path (super::vehicle::Model) and get its module path
                // Then import super::vehicle::Vehicle (the API struct)

                // Helper function to recursively extract the innermost Type::Path
                fn extract_innermost_path(ty: &syn::Type) -> Option<&syn::TypePath> {
                    if let syn::Type::Path(type_path) = ty {
                        // Check if this is a container type (JoinField, Vec, Option)
                        if let Some(segment) = type_path.path.segments.last() {
                            if segment.ident == "JoinField" || segment.ident == "Vec" || segment.ident == "Option" {
                                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                                        // Recursively extract from inner type
                                        return extract_innermost_path(inner_ty);
                                    }
                                }
                            }
                        }
                        // Base case: return this path (it's not a container type)
                        return Some(type_path);
                    }
                    None
                }

                if let Some(inner_type_path) = extract_innermost_path(&field.ty) {
                    // inner_type_path is now super::vehicle_part::Model
                    // Build the module path by extracting path segments and replacing Model with API struct name
                    let path_segments: Vec<_> = inner_type_path.path.segments.iter()
                        .take(inner_type_path.path.segments.len() - 1) // Take all except the last (Model)
                        .map(|seg| seg.ident.clone())
                        .collect();

                    #[cfg(feature = "debug")]
                    eprintln!("DEBUG generate_api_struct: Path segments: {:?}", path_segments.iter().map(|s| s.to_string()).collect::<Vec<_>>());

                    if !path_segments.is_empty() {
                        let api_ident = quote::format_ident!("{}", api_name);
                        let module_path = quote! { #(#path_segments)::* };

                        // Create a unique key for deduplication
                        let import_key = format!("{}::{}", quote!{#module_path}, api_name);

                        if !seen_imports.contains(&import_key) {
                            seen_imports.insert(import_key);

                            let import_stmt = quote! {
                                use #module_path::#api_ident;
                            };

                            #[cfg(feature = "debug")]
                            eprintln!("DEBUG generate_api_struct: Adding import: {}", quote!{#import_stmt});

                            import_statements.push(import_stmt);
                        }
                    }
                }
            }
        }
    }

    quote! {
        use sea_orm::ActiveValue;
        use utoipa::ToSchema;
        use serde::{Serialize, Deserialize};
        use crudcrate::{ToUpdateModel, ToCreateModel};
        #(#import_statements)*

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
    // Note: join fields should not be considered for CRUD resource generation
    // They are populated by join loading, not direct CRUD operations
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

    // Always include ToSchema for Create models
    // Circular dependencies are handled by schema(no_recursion) on join fields in the main model
    let create_derives = quote! { Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema };

    let expanded = quote! {
        #[derive(#create_derives)]
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

    // Always include ToSchema for Update models
    // Circular dependencies are handled by schema(no_recursion) on join fields in the main model
    let update_derives = quote! { Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema };

    let expanded = quote! {
        #[derive(#update_derives)]
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

    // Always include ToSchema for List models
    // Circular dependencies are handled by schema(no_recursion) on join fields in the main model
    let list_derives = quote! { Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema };

    let expanded = quote! {
        #[derive(#list_derives)]
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

    // === PASS 1: Discovery Phase ===
    // Register this entity in the global type registry for join field resolution
    // Use the API struct name as the entity name to ensure proper type resolution
    let entity_name = api_struct_name.to_string();
    let api_name = api_struct_name.to_string();
    two_pass_generator::register_entity_globally(entity_name, api_name);

    // Generate compile-time validation for join relationships
    let join_validation = relation_validator::generate_join_relation_validation(&field_analysis);

    // Check for cyclic dependencies and emit compile-time error if detected
    let cyclic_dependency_check = relation_validator::generate_cyclic_dependency_check(&field_analysis, &api_struct_name.to_string());
    if !cyclic_dependency_check.is_empty() {
        return cyclic_dependency_check.into();
    }

    let (api_struct_fields, from_model_assignments, required_imports) =
        generate_api_struct_content(&field_analysis, &api_struct_name);
    let api_struct =
        generate_api_struct(&api_struct_name, &api_struct_fields, &active_model_path, &crud_meta, &field_analysis, &required_imports);
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
    
    // Always include ToSchema for List models in EntityToModels
    // Circular dependencies are handled by schema(no_recursion) on join fields in the main model
    let list_derives = quote! { Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema };

    let list_model = quote! {
        #[derive(#list_derives)]
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

    // Generate Response model struct for get_one/create/update responses (excludes exclude(one) fields)
    let response_name = format_ident!("{}Response", &api_struct_name);
    let response_struct_fields = macro_implementation::generate_response_struct_fields(&raw_fields);
    let response_from_assignments = macro_implementation::generate_response_from_assignments(&raw_fields);

    let response_derives = quote! { Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema };

    let response_model = quote! {
        #[derive(#response_derives)]
        pub struct #response_name {
            #(#response_struct_fields),*
        }

        impl From<#api_struct_name> for #response_name {
            fn from(model: #api_struct_name) -> Self {
                Self {
                    #(#response_from_assignments),*
                }
            }
        }
    };

    let expanded = quote! {
        #api_struct
        #from_impl
        #crud_impl
        #list_model
        #response_model
        #join_validation
    };

    // Print debug output if requested (either via attribute or cargo feature)
    #[cfg(feature = "debug")]
    if crud_meta.debug_output {
        debug_output::print_debug_output(&expanded, &api_struct_name.to_string());
    }

    TokenStream::from(expanded)
}
