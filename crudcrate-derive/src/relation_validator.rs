use super::structs::EntityFieldAnalysis;
use convert_case::{Case, Casing};

/// Generates compile-time validation code for join relations
/// Since proc macros cannot access sibling Relation enums, we generate code that
/// references the required relations - if they don't exist, compilation fails
pub fn generate_join_relation_validation(
    analysis: &EntityFieldAnalysis,
) -> proc_macro2::TokenStream {
    use quote::quote;
    
    let mut validation_checks = Vec::new();
    
    // Generate validation checks for join_on_one fields
    for field in &analysis.join_on_one_fields {
        if let Some(field_name) = &field.ident {
            let expected_relation = field_name_to_relation_variant(field_name);
            let expected_relation_ident = syn::Ident::new(&expected_relation, field_name.span());
            
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
    }
    
    // Generate validation checks for join_on_all fields  
    for field in &analysis.join_on_all_fields {
        if let Some(field_name) = &field.ident {
            let expected_relation = field_name_to_relation_variant(field_name);
            let expected_relation_ident = syn::Ident::new(&expected_relation, field_name.span());
            
            validation_checks.push(quote! {
                // Compile-time validation: This will fail if Relation::#expected_relation_ident doesn't exist
                const _: () = {
                    fn _validate_relation_exists() {
                        let _ = Relation::#expected_relation_ident;
                    }
                };
            });
        }
    }
    
    quote! {
        #( #validation_checks )*
    }
}

/// Convert a field name to the expected relation variant name
/// Example: "vehicles" -> "Vehicles", "maintenance_records" -> "MaintenanceRecords" 
fn field_name_to_relation_variant(field_name: &syn::Ident) -> String {
    let field_str = field_name.to_string();
    // Convert to PascalCase for relation variant name
    field_str.to_case(Case::Pascal)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_field_name_to_relation_variant() {
        use quote::format_ident;
        assert_eq!(field_name_to_relation_variant(&format_ident!("vehicles")), "Vehicles");
        assert_eq!(field_name_to_relation_variant(&format_ident!("maintenance_records")), "MaintenanceRecords");
        assert_eq!(field_name_to_relation_variant(&format_ident!("user")), "User");
    }
    
    #[test]
    fn test_type_validation_helpers() {
        use crate::join_generators::is_vec_type;
        
        let vec_type: syn::Type = syn::parse_quote!(Vec<String>);
        let option_type: syn::Type = syn::parse_quote!(Option<String>);
        let plain_type: syn::Type = syn::parse_quote!(String);
        
        // Test is_vec_type function
        assert!(is_vec_type(&vec_type));
        assert!(!is_vec_type(&option_type));
        assert!(!is_vec_type(&plain_type));
        
        // Test is_optional_type by manually checking the type
        fn is_optional_type(ty: &syn::Type) -> bool {
            if let syn::Type::Path(type_path) = ty {
                if let Some(segment) = type_path.path.segments.last() {
                    return segment.ident == "Option";
                }
            }
            false
        }
        
        assert!(!is_optional_type(&vec_type));
        assert!(is_optional_type(&option_type));
        assert!(!is_optional_type(&plain_type));
    }
}