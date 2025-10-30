//! Lazy loading join field support for recursive relationships
//!
//! This module provides utilities for handling join fields that cannot be resolved
//! at compile time due to macro expansion order issues.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use utoipa::{PartialSchema, ToSchema};

/// A generic wrapper for join fields that can hold any serializable data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[derive(Default)]
pub enum JoinField<T> {
    Loaded(T),
    #[default]
    NotLoaded,
}

impl<T> PartialEq for JoinField<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (JoinField::Loaded(a), JoinField::Loaded(b)) => a == b,
            (JoinField::NotLoaded, JoinField::NotLoaded) => true,
            _ => false,
        }
    }
}


// Implement additional traits needed for CRUD operations
impl<T> JoinField<T>
where
    T: Clone,
{
    pub fn as_option(&self) -> Option<&T> {
        match self {
            JoinField::Loaded(data) => Some(data),
            JoinField::NotLoaded => None,
        }
    }

    pub fn map<U, F>(self, f: F) -> JoinField<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            JoinField::Loaded(data) => JoinField::Loaded(f(data)),
            JoinField::NotLoaded => JoinField::NotLoaded,
        }
    }
}

impl<T> JoinField<T> {
    pub fn is_loaded(&self) -> bool {
        matches!(self, JoinField::Loaded(_))
    }

    pub fn get(&self) -> Option<&T> {
        match self {
            JoinField::Loaded(data) => Some(data),
            JoinField::NotLoaded => None,
        }
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        match self {
            JoinField::Loaded(data) => Some(data),
            JoinField::NotLoaded => None,
        }
    }

    pub fn set(&mut self, data: T) {
        *self = JoinField::Loaded(data);
    }
}

// Implement conversion for collections
impl<T> From<T> for JoinField<T> {
    fn from(data: T) -> Self {
        JoinField::Loaded(data)
    }
}

impl<T> FromIterator<T> for JoinField<Vec<T>> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        JoinField::Loaded(iter.into_iter().collect())
    }
}

// Optional: Add a convenient constructor
impl<T> JoinField<T> {
    pub fn loaded(data: T) -> Self {
        JoinField::Loaded(data)
    }
}

// Implement ToSchema for JoinField without requiring T: ToSchema
// This allows join fields to work even when the inner type doesn't support OpenAPI schemas
impl<T> ToSchema for JoinField<T> {
    fn name() -> std::borrow::Cow<'static, str> {
        // Use a generic name based on the type parameter name
        let type_name = std::any::type_name::<T>()
            .split("::")
            .last()
            .unwrap_or("Unknown");

        if std::any::type_name::<T>().contains("Vec") {
            format!("JoinFieldOf{type_name}").into()
        } else {
            format!("JoinFieldOf{type_name}").into()
        }
    }
}

impl<T> PartialSchema for JoinField<T> {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        use utoipa::openapi::schema::{ObjectBuilder, ArrayBuilder, Schema};

        // Check if T is a Vec type and return appropriate schema
        if std::any::type_name::<T>().contains("Vec") {
            utoipa::openapi::RefOr::T(Schema::Array(
                ArrayBuilder::new()
                    .items(utoipa::openapi::RefOr::T(Schema::Object(
                        ObjectBuilder::new().into()
                    )))
                    .into()
            ))
        } else {
            utoipa::openapi::RefOr::T(Schema::Object(
                ObjectBuilder::new()
                    .into()
            ))
        }
    }
}

/// Type registry for resolving join field types at runtime
pub static JOIN_TYPE_REGISTRY: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Register a join field type mapping
pub fn register_join_type(field_name: &str, type_name: &str) {
    let mut registry = JOIN_TYPE_REGISTRY.lock().unwrap();
    registry.insert(field_name.to_string(), type_name.to_string());
}

/// Get the registered type name for a join field
pub fn get_join_type_name(field_name: &str) -> Option<String> {
    let registry = JOIN_TYPE_REGISTRY.lock().unwrap();
    registry.get(field_name).cloned()
}

/// Macro to generate lazy join field accessors
#[macro_export]
macro_rules! lazy_join_field {
    ($field_name:ident, $field_type:ty) => {
        pub fn $field_name(&self) -> &$crate::join_fields::JoinField<$field_type> {
            &self.$field_name
        }

        pub fn $field_name_mut(&mut self) -> &mut $crate::join_fields::JoinField<$field_type> {
            &mut self.$field_name
        }

        pub fn with_$field_name(mut self, data: $field_type) -> Self {
            self.$field_name.set(data);
            self
        }
    };
}