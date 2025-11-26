//! Validation Support
//!
//! This module provides traits and utilities for validating CRUD payloads.
//! Users can implement the `Validatable` trait on their Create/Update models
//! to add custom validation logic.
//!
//! # Example
//!
//! ```rust,ignore
//! use crudcrate::validation::{Validatable, ValidationError};
//!
//! #[derive(Debug, serde::Serialize)]
//! pub struct ProductCreate {
//!     pub name: String,
//!     pub price: i32,
//! }
//!
//! impl Validatable for ProductCreate {
//!     fn validate(&self) -> Result<(), ValidationError> {
//!         if self.name.len() < 3 {
//!             return Err(ValidationError::new("name", "Name must be at least 3 characters"));
//!         }
//!
//!         if self.price <= 0 {
//!             return Err(ValidationError::new("price", "Price must be positive"));
//!         }
//!
//!         Ok(())
//!     }
//! }
//! ```

use serde::Serialize;
use std::fmt;

/// Validation error with field name and message
#[derive(Debug, Clone, Serialize)]
pub struct ValidationError {
    /// The field that failed validation
    pub field: String,
    /// Human-readable error message
    pub message: String,
}

impl ValidationError {
    /// Create a new validation error
    #[must_use]
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for ValidationError {}

/// Collection of validation errors
#[derive(Debug, Clone, Serialize)]
pub struct ValidationErrors {
    errors: Vec<ValidationError>,
}

impl ValidationErrors {
    /// Create a new empty validation errors collection
    #[must_use]
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Add a validation error
    pub fn add(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    /// Check if there are any errors
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get the number of errors
    #[must_use]
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    /// Get all errors
    #[must_use]
    pub fn errors(&self) -> &[ValidationError] {
        &self.errors
    }

    /// Convert to Result
    pub fn result(self) -> Result<(), Self> {
        if self.is_empty() {
            Ok(())
        } else {
            Err(self)
        }
    }
}

impl Default for ValidationErrors {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Validation failed with {} error(s):", self.errors.len())?;
        for error in &self.errors {
            write!(f, "\n  - {}", error)?;
        }
        Ok(())
    }
}

impl std::error::Error for ValidationErrors {}

/// Trait for types that can be validated
///
/// Implement this trait on your Create/Update models to add custom validation.
/// The validation will be called automatically before database operations if
/// you use the generated CRUD handlers.
pub trait Validatable {
    /// Validate the instance
    ///
    /// Return `Ok(())` if valid, or `Err(ValidationError)` if invalid.
    /// For multiple validation errors, use `ValidationErrors`.
    fn validate(&self) -> Result<(), ValidationError>;
}

/// Helper validators for common patterns
pub mod validators {
    use super::ValidationError;

    /// Validate string length is within range
    pub fn validate_length(
        field: &str,
        value: &str,
        min: Option<usize>,
        max: Option<usize>,
    ) -> Result<(), ValidationError> {
        let len = value.len();

        if let Some(min_len) = min {
            if len < min_len {
                return Err(ValidationError::new(
                    field,
                    format!("Must be at least {} characters", min_len),
                ));
            }
        }

        if let Some(max_len) = max {
            if len > max_len {
                return Err(ValidationError::new(
                    field,
                    format!("Must be at most {} characters", max_len),
                ));
            }
        }

        Ok(())
    }

    /// Validate number is within range
    pub fn validate_range<T: PartialOrd + fmt::Display>(
        field: &str,
        value: T,
        min: Option<T>,
        max: Option<T>,
    ) -> Result<(), ValidationError> {
        if let Some(min_val) = min {
            if value < min_val {
                return Err(ValidationError::new(
                    field,
                    format!("Must be at least {}", min_val),
                ));
            }
        }

        if let Some(max_val) = max {
            if value > max_val {
                return Err(ValidationError::new(
                    field,
                    format!("Must be at most {}", max_val),
                ));
            }
        }

        Ok(())
    }

    /// Basic email validation
    pub fn validate_email(field: &str, value: &str) -> Result<(), ValidationError> {
        if !value.contains('@') || !value.contains('.') {
            return Err(ValidationError::new(field, "Invalid email format"));
        }

        if value.len() > 255 {
            return Err(ValidationError::new(
                field,
                "Email must be at most 255 characters",
            ));
        }

        Ok(())
    }

    /// Validate value is not empty
    pub fn validate_required(field: &str, value: &str) -> Result<(), ValidationError> {
        if value.trim().is_empty() {
            return Err(ValidationError::new(field, "This field is required"));
        }
        Ok(())
    }

    use std::fmt;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_error_creation() {
        let err = ValidationError::new("email", "Invalid email");
        assert_eq!(err.field, "email");
        assert_eq!(err.message, "Invalid email");
    }

    #[test]
    fn test_validation_errors_collection() {
        let mut errors = ValidationErrors::new();
        assert!(errors.is_empty());

        errors.add(ValidationError::new("field1", "error1"));
        assert_eq!(errors.len(), 1);

        errors.add(ValidationError::new("field2", "error2"));
        assert_eq!(errors.len(), 2);

        assert!(errors.result().is_err());
    }

    #[test]
    fn test_validate_length() {
        use validators::validate_length;

        // Too short
        assert!(validate_length("name", "ab", Some(3), None).is_err());

        // Too long
        assert!(validate_length("name", "abcdef", None, Some(5)).is_err());

        // Just right
        assert!(validate_length("name", "abc", Some(3), Some(5)).is_ok());
    }

    #[test]
    fn test_validate_range() {
        use validators::validate_range;

        // Too small
        assert!(validate_range("age", 5, Some(10), None).is_err());

        // Too large
        assert!(validate_range("age", 150, None, Some(120)).is_err());

        // Just right
        assert!(validate_range("age", 25, Some(0), Some(120)).is_ok());
    }

    #[test]
    fn test_validate_email() {
        use validators::validate_email;

        assert!(validate_email("email", "invalid").is_err());
        assert!(validate_email("email", "test@example.com").is_ok());
    }

    #[test]
    fn test_validate_required() {
        use validators::validate_required;

        assert!(validate_required("name", "").is_err());
        assert!(validate_required("name", "   ").is_err());
        assert!(validate_required("name", "John").is_ok());
    }
}
