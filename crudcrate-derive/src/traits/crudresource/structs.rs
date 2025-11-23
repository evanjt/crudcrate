use convert_case::{Case, Casing};

/// Hook configuration for a single operation phase (pre, body, post)
#[derive(Default, Clone)]
pub(crate) struct OperationHooks {
    pub(crate) pre: Option<syn::Path>,
    pub(crate) body: Option<syn::Path>,
    pub(crate) post: Option<syn::Path>,
}

/// Hooks for an operation with one/many variants
#[derive(Default, Clone)]
pub(crate) struct CrudOperationHooks {
    pub(crate) one: OperationHooks,
    pub(crate) many: OperationHooks,
}

/// All CRUD operation hooks
#[derive(Default, Clone)]
pub(crate) struct CrudHooks {
    pub(crate) create: CrudOperationHooks,
    pub(crate) read: CrudOperationHooks,
    pub(crate) update: CrudOperationHooks,
    pub(crate) delete: CrudOperationHooks,
}

/// Extracts `CRUDResource` metadata from struct-level crudcrate attributes
#[derive(Default)]
pub(crate) struct CRUDResourceMeta {
    pub(crate) name_singular: Option<String>,
    pub(crate) name_plural: Option<String>,
    pub(crate) description: Option<String>,
    // Hook system for CRUD customization
    pub(crate) hooks: CrudHooks,
    // Other options
    pub(crate) operations: Option<syn::Path>,
    pub(crate) generate_router: bool,
    pub(crate) fulltext_language: Option<String>,
    pub(crate) derive_partial_eq: bool,
    pub(crate) derive_eq: bool,
    // Deprecation errors to emit as compile errors
    pub(crate) deprecation_errors: Vec<syn::Error>,
}

impl CRUDResourceMeta {
    /// Apply smart defaults based on table name and api struct name
    pub(crate) fn with_defaults(mut self, table_name: &str) -> Self {
        if self.name_singular.is_none() {
            // Set the table name by default to the snake_case version of the struct name
            self.name_singular = Some(table_name.to_case(Case::Snake));
        }
        if self.name_plural.is_none() {
            // Simple pluralization - add 's' if doesn't end with 's'
            // Probably not the best strategy, but good enough
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
        self
    }
}

pub(crate) struct EntityFieldAnalysis<'a> {
    pub(crate) db_fields: Vec<&'a syn::Field>,
    pub(crate) non_db_fields: Vec<&'a syn::Field>,
    pub(crate) primary_key_field: Option<&'a syn::Field>,
    pub(crate) sortable_fields: Vec<&'a syn::Field>,
    pub(crate) filterable_fields: Vec<&'a syn::Field>,
    pub(crate) fulltext_fields: Vec<&'a syn::Field>,
    pub(crate) join_on_one_fields: Vec<&'a syn::Field>,
    pub(crate) join_on_all_fields: Vec<&'a syn::Field>,
}
