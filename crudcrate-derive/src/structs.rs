use convert_case::{Case, Casing};

/// Extracts `CRUDResource` metadata from struct-level crudcrate attributes
pub(super) struct CRUDResourceMeta {
    pub(super) name_singular: Option<String>,
    pub(super) name_plural: Option<String>,
    pub(super) description: Option<String>,
    pub(super) entity_type: Option<String>,
    pub(super) column_type: Option<String>,
    pub(super) fn_get_one: Option<syn::Path>,
    pub(super) fn_get_all: Option<syn::Path>,
    pub(super) fn_create: Option<syn::Path>,
    pub(super) fn_update: Option<syn::Path>,
    pub(super) fn_delete: Option<syn::Path>,
    pub(super) fn_delete_many: Option<syn::Path>,
    pub(super) generate_router: bool,
    pub(super) fulltext_language: Option<String>,
    /// Whether to derive `PartialEq` on generated structs (default: true for backward compatibility)
    pub(super) derive_partial_eq: bool,
    /// Whether to derive `Eq` on generated structs (default: false, only added to main API struct when true)
    pub(super) derive_eq: bool,
    #[cfg(feature = "debug")]
    pub(super) debug_output: bool,
}


impl Default for CRUDResourceMeta {
    fn default() -> Self {
        Self {
            name_singular: None,
            name_plural: None,
            description: None,
            entity_type: None,
            column_type: None,
            fn_get_one: None,
            fn_get_all: None,
            fn_create: None,
            fn_update: None,
            fn_delete: None,
            fn_delete_many: None,
            generate_router: false,
            fulltext_language: None,
            // Default to true for backward compatibility - most common types implement PartialEq
            // Users can opt-out with no_partial_eq if needed
            derive_partial_eq: true,
            // Default to false - Eq is more restrictive, users must opt in with derive_eq
            derive_eq: false,
            #[cfg(feature = "debug")]
            debug_output: false,
        }
    }
}

impl CRUDResourceMeta {
    /// Apply smart defaults based on table name and api struct name
    pub(super) fn with_defaults(mut self, table_name: &str, _api_struct_name: &str) -> Self {
        if self.name_singular.is_none() {
            self.name_singular = Some(table_name.to_case(Case::Snake));
        }
        if self.name_plural.is_none() {
            // Simple pluralization - add 's' if doesn't end with 's'
            let singular = self.name_singular.as_ref().unwrap();
            self.name_plural = Some(if singular.ends_with('s') {
                singular.clone()
            } else {
                format!("{singular}s")
            });
        }
        if self.description.is_none() {
            self.description = Some(format!(
                "This resource manages {} items",
                self.name_singular.as_ref().unwrap()
            ));
        }
        if self.entity_type.is_none() {
            self.entity_type = Some("Entity".to_string());
        }
        if self.column_type.is_none() {
            self.column_type = Some("Column".to_string());
        }
        self
    }
}

pub(super) struct EntityFieldAnalysis<'a> {
    pub(super) db_fields: Vec<&'a syn::Field>,
    pub(super) non_db_fields: Vec<&'a syn::Field>,
    pub(super) primary_key_field: Option<&'a syn::Field>,
    pub(super) sortable_fields: Vec<&'a syn::Field>,
    pub(super) filterable_fields: Vec<&'a syn::Field>,
    pub(super) fulltext_fields: Vec<&'a syn::Field>,
    pub(super) join_on_one_fields: Vec<&'a syn::Field>,
    pub(super) join_on_all_fields: Vec<&'a syn::Field>,
    // pub(super) join_configs: std::collections::HashMap<String, crate::attribute_parser::JoinConfig>, // Removed due to HashMap key issues
}
