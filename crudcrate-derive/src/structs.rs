use convert_case::{Case, Casing};

/// Extracts `CRUDResource` metadata from struct-level crudcrate attributes
#[derive(Default)]
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
    pub(super) enum_case_sensitive: bool,
    pub(super) fulltext_language: Option<String>,
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
}
