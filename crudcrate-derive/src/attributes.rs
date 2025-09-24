/*!
# CrudCrate Attribute Definitions

This file provides IDE-friendly attribute definitions for autocomplete support.
These are documentation-only types that help IDEs understand the available attributes.

## Usage

This file is primarily for IDE support and doesn't contain runtime code.
The actual attribute parsing happens in `attribute_parser.rs`.

## Struct-Level Attributes

```rust
#[crudcrate(
    // Boolean flags (use just the name, or name = true/false)
    generate_router,              // Auto-generate Axum router
    debug_output,                 // Print generated code (requires --features debug)
    
    // Named parameters
    api_struct = "CustomName",    // Override API struct name
    active_model = "CustomPath",  // Override ActiveModel path
    name_singular = "item",       // Resource singular name
    name_plural = "items",        // Resource plural name  
    description = "Description",  // Resource description
    entity_type = "Entity",       // Entity type override
    column_type = "Column",       // Column type override
    fulltext_language = "english", // Default fulltext language
    
    // Function overrides
    fn_get_one = custom::get_one,     // Custom get_one function
    fn_get_all = custom::get_all,     // Custom get_all function
    fn_create = custom::create,       // Custom create function
    fn_update = custom::update,       // Custom update function  
    fn_delete = custom::delete,       // Custom delete function
    fn_delete_many = custom::delete_many, // Custom delete_many function
)]
```

## Field-Level Attributes

```rust
#[crudcrate(
    // Boolean flags (use just the name, or name = true/false)
    primary_key,                  // Mark as primary key
    sortable,                     // Include in sortable columns
    filterable,                   // Include in filterable columns
    fulltext,                     // Enable full-text search
    non_db_attr,                  // Field not in database
    enum_field,                   // Enable enum filtering support
    use_target_models,            // Use target's models instead of full entity
    
    // Model exclusion - Traditional syntax (still supported)
    create_model = false,         // Exclude from Create model
    update_model = false,         // Exclude from Update model
    list_model = false,           // Exclude from List model
    
    // Function-style syntax
    exclude(create),              // Exclude from Create model only
    exclude(update),              // Exclude from Update model only  
    exclude(list),                // Exclude from List model only
    exclude(create, update),      // Exclude from both Create and Update
    exclude(create, update, list), // Exclude from all models
    
    // Positive logic aliases
    exclude_create,               // Equivalent to create_model = false
    exclude_update,               // Equivalent to update_model = false
    exclude_list,                 // Equivalent to list_model = false
    skip_create,                  // Alternative alias
    skip_update,                  // Alternative alias
    no_create,                    // Alternative alias
    no_update,                    // Alternative alias
    no_list,                      // Alternative alias
    
    // Expression parameters
    on_create = Uuid::new_v4(),   // Auto-generate on create
    on_update = Utc::now(),       // Auto-generate on update
    default = vec![],             // Default value for non-DB fields
    fulltext_language = "english", // Language for fulltext search
    
    // Join configuration
    join(one),                    // Load in get_one() calls
    join(all),                    // Load in get_all() calls
    join(one, all),              // Load in both get_one() and get_all()
    join(one, all, depth = 3),   // Recursive loading with depth (default: 3)
    join(one, all, relation = "CustomRelation"), // Custom relation name
)]
```

## Examples

### Basic Entity
```text
use chrono::{DateTime, Utc};
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "users")]
#[crudcrate(api_struct = "User", generate_router)]
pub struct Model {
    // Function-style exclude syntax
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,
    
    #[crudcrate(sortable, filterable, fulltext)]
    pub name: String,
    
    #[crudcrate(filterable)]
    pub email: String,
    
    // Auto-managed timestamps
    #[crudcrate(sortable, exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
    
    #[crudcrate(sortable, exclude(create, update), on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,
}
```

### Entity with Joins
```text  
use chrono::{DateTime, Utc};
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

struct Vehicle {}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "customers")]
#[crudcrate(
    api_struct = "Customer",
    generate_router,
    description = "Customer management with vehicle relationships"
)]
pub struct Model {
    // Primary key with function-style exclude syntax
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,
    
    // Searchable field
    #[crudcrate(sortable, filterable, fulltext)]
    pub name: String,
    
    #[crudcrate(filterable)]
    pub email: String,
    
    // Recursive join with default depth (depth=3)
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all))]
    pub vehicles: Vec<Vehicle>,
    
    // Auto-managed timestamps
    #[crudcrate(sortable, exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
    
    #[crudcrate(sortable, exclude(create, update), on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,
}
```

### Custom Functions
```text
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[crudcrate(
    api_struct = "Post",
    fn_get_all = custom::get_posts_with_author,
    fn_create = custom::create_post_with_validation
)]
pub struct Model {
    // ... fields
}
```
*/

// IDE-friendly type definitions (not used at runtime)
#[allow(dead_code)]
mod ide_support {
    /// Struct-level crudcrate attribute options
    pub struct CrudcrateStruct {
        // Boolean flags
        pub generate_router: bool,
        pub debug_output: bool,
        
        // Named parameters  
        pub api_struct: String,
        pub active_model: String,
        pub name_singular: String,
        pub name_plural: String,
        pub description: String,
        pub entity_type: String,
        pub column_type: String,
        pub fulltext_language: String,
        
        // Function overrides
        pub fn_get_one: fn(),
        pub fn_get_all: fn(),
        pub fn_create: fn(),
        pub fn_update: fn(),
        pub fn_delete: fn(),
        pub fn_delete_many: fn(),
    }
    
    /// Field-level crudcrate attribute options
    pub struct CrudcrateField {
        // Core boolean flags
        pub primary_key: bool,
        pub sortable: bool,
        pub filterable: bool,
        pub fulltext: bool,
        pub non_db_attr: bool,
        pub enum_field: bool,
        pub use_target_models: bool,
        
        // Model exclusion - Traditional syntax (still supported)
        pub create_model: bool,
        pub update_model: bool,
        pub list_model: bool,
        
        // Model exclusion - Positive logic aliases
        pub exclude_create: bool,
        pub exclude_update: bool,
        pub exclude_list: bool,
        pub skip_create: bool,
        pub skip_update: bool,
        pub no_create: bool,
        pub no_update: bool,
        pub no_list: bool,
        
        // Expression parameters
        pub on_create: String, // Expression as string
        pub on_update: String, // Expression as string
        pub default: String,   // Expression as string
        pub fulltext_language: String,
    }
    
    /// Join configuration options (function-style syntax)
    pub struct JoinConfig {
        pub one: bool,      // Load in get_one() calls
        pub all: bool,      // Load in get_all() calls
        pub depth: u8,      // Recursive depth (default: 3)
        pub relation: String, // Custom relation name
    }
    
    /// Exclude configuration options (function-style syntax)  
    pub struct ExcludeConfig {
        pub create: bool,   // Exclude from Create model
        pub update: bool,   // Exclude from Update model
        pub list: bool,     // Exclude from List model
    }
    
    /// Available function-style syntax patterns
    pub mod function_style {
        /// join(one, all) - Load relationships
        /// join(one, all, depth = 2) - With custom depth
        /// join(one, all, relation = "CustomRelation") - With custom relation
        pub struct Join;
        
        /// exclude(create) - Single exclusion
        /// exclude(create, update) - Multiple exclusions  
        /// exclude(create, update, list) - All model types
        pub struct Exclude;
    }
}