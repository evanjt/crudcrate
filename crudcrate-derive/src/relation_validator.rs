use super::structs::EntityFieldAnalysis;
use super::attribute_parser::get_join_config;
use convert_case::{Case, Casing};
use heck::ToPascalCase;

/// Generates compile-time validation code for join relations
/// Since proc macros cannot access sibling Relation enums, we generate code that
/// references the required relations - if they don't exist, compilation fails
pub fn generate_join_relation_validation(
    analysis: &EntityFieldAnalysis,
) -> proc_macro2::TokenStream {
    use quote::quote;

    let mut validation_checks = Vec::new();

    // Generate validation checks for join_on_one fields (only if custom relation is specified)
    for field in &analysis.join_on_one_fields {
        if let Some(field_name) = &field.ident {
            if let Some(join_config) = get_join_config(field) {
                if let Some(custom_relation) = join_config.relation {
                    // Only validate if a custom relation name is explicitly provided
                    let expected_relation_ident = syn::Ident::new(&custom_relation, field_name.span());

                    // Generate a compile-time check that references the relation
                    validation_checks.push(quote! {
                        // Compile-time validation: This will fail if Relation::#expected_relation_ident doesn't exist
                        const _: () = {
                            fn _validate_relation_exists() {
                                let _ = Relation::#expected_relation_ident;
                            }
                        };
                    });
                }
                // If no custom relation is specified, we use entity path resolution - no validation needed
            }
        }
    }

    // Generate validation checks for join_on_all fields (only if custom relation is specified)
    for field in &analysis.join_on_all_fields {
        if let Some(field_name) = &field.ident {
            if let Some(join_config) = get_join_config(field) {
                if let Some(custom_relation) = join_config.relation {
                    // Only validate if a custom relation name is explicitly provided
                    let expected_relation_ident = syn::Ident::new(&custom_relation, field_name.span());

                    validation_checks.push(quote! {
                        // Compile-time validation: This will fail if Relation::#expected_relation_ident doesn't exist
                        const _: () = {
                            fn _validate_relation_exists() {
                                let _ = Relation::#expected_relation_ident;
                            }
                        };
                    });
                }
                // If no custom relation is specified, we use entity path resolution - no validation needed
            }
        }
    }

    quote! {
        #( #validation_checks )*
    }
}

/// Convert a field name to the expected relation variant name
/// Example: "entities" -> "Entities", "`related_items`" -> "`RelatedItems`"
fn field_name_to_relation_variant(field_name: &syn::Ident) -> String {
    let field_str = field_name.to_string();
    // Convert to PascalCase for relation variant name
    field_str.to_pascal_case()
}

#[cfg(test)]
fn is_optional_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last() {
        return segment.ident == "Option";
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_name_to_relation_variant() {
        use quote::format_ident;
        assert_eq!(
            field_name_to_relation_variant(&format_ident!("entities")),
            "Entities"
        );
        assert_eq!(
            field_name_to_relation_variant(&format_ident!("related_items")),
            "RelatedItems"
        );
        assert_eq!(
            field_name_to_relation_variant(&format_ident!("item")),
            "Item"
        );
    }

    #[test]
    fn test_type_validation_helpers() {
        use crate::macro_implementation::is_vec_type;

        let vec_type: syn::Type = syn::parse_quote!(Vec<String>);
        let option_type: syn::Type = syn::parse_quote!(Option<String>);
        let plain_type: syn::Type = syn::parse_quote!(String);

        // Test is_vec_type function
        assert!(is_vec_type(&vec_type));
        assert!(!is_vec_type(&option_type));
        assert!(!is_vec_type(&plain_type));

        assert!(!is_optional_type(&vec_type));
        assert!(is_optional_type(&option_type));
        assert!(!is_optional_type(&plain_type));
    }
}
