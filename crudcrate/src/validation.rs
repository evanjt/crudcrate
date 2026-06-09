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

    /// Convert to `Result`, returning `Err(self)` when errors are present.
    ///
    /// # Errors
    /// Returns `Err(ValidationErrors)` if any validation errors were added.
    pub fn result(self) -> Result<(), Self> {
        if self.is_empty() { Ok(()) } else { Err(self) }
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
            write!(f, "\n  - {error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ValidationErrors {}

/// Trait for types that can be validated
///
/// Implement this trait on your Create/Update models to add custom validation.
/// When you use the generated CRUD handlers (`#[crudcrate(generate_router)]`), the
/// generated `create`/`create_many`/`update`/`update_many` implementations call
/// `validate()` automatically before any database write, returning HTTP 422 on
/// failure. Models that do not implement `Validatable` are unaffected (the
/// generated check is a no-op for them — see [`__auto`]).
pub trait Validatable {
    /// Validate the instance.
    ///
    /// # Errors
    /// Returns `Err(ValidationError)` if validation fails.
    fn validate(&self) -> Result<(), ValidationError>;
}

/// Autoref-specialization machinery used by the derive-generated CRUD handlers to
/// invoke [`Validatable::validate`] only for models that implement it, with a no-op
/// fallback otherwise — without requiring nightly `specialization`.
///
/// The generated code calls `Probe(&model).crudcrate_auto_validate()`. When the
/// model implements [`Validatable`], the inherent method on [`Probe`] wins method
/// resolution and runs the real validation; otherwise the blanket
/// [`ValidatableFallback`] trait method is used and does nothing.
#[doc(hidden)]
pub mod __auto {
    use super::{Validatable, ValidationError};

    /// No-op fallback implemented for every type. Lower priority than the inherent
    /// method on `Probe<T: Validatable>`.
    pub trait ValidatableFallback {
        /// No-op validation for types that do not implement [`Validatable`].
        ///
        /// # Errors
        /// Never returns an error.
        fn crudcrate_auto_validate(&self) -> Result<(), ValidationError>;
    }

    impl<T> ValidatableFallback for T {
        fn crudcrate_auto_validate(&self) -> Result<(), ValidationError> {
            Ok(())
        }
    }

    /// Wrapper whose inherent method shadows the [`ValidatableFallback`] trait
    /// method when the wrapped type implements [`Validatable`].
    pub struct Probe<'a, T>(pub &'a T);

    impl<T: Validatable> Probe<'_, T> {
        /// Runs the real [`Validatable::validate`]. Selected over the fallback
        /// because inherent methods take priority in method resolution.
        ///
        /// # Errors
        /// Returns the wrapped type's [`ValidationError`] on failure.
        pub fn crudcrate_auto_validate(&self) -> Result<(), ValidationError> {
            self.0.validate()
        }
    }
}

/// Helper validators for common patterns
pub mod validators {
    use super::ValidationError;

    /// Validate string length is within range.
    ///
    /// # Errors
    /// Returns `Err(ValidationError)` if the length is outside the given bounds.
    pub fn validate_length(
        field: &str,
        value: &str,
        min: Option<usize>,
        max: Option<usize>,
    ) -> Result<(), ValidationError> {
        // Count Unicode scalar values, not UTF-8 bytes: the messages promise a limit
        // in "characters", so a multibyte value (accented names, CJK) within the
        // character limit must not be rejected for exceeding a byte count.
        let len = value.chars().count();

        if let Some(min_len) = min
            && len < min_len
        {
            return Err(ValidationError::new(
                field,
                format!("Must be at least {min_len} characters"),
            ));
        }

        if let Some(max_len) = max
            && len > max_len
        {
            return Err(ValidationError::new(
                field,
                format!("Must be at most {max_len} characters"),
            ));
        }

        Ok(())
    }

    /// Validate number is within range.
    ///
    /// # Errors
    /// Returns `Err(ValidationError)` if the value is outside the given bounds.
    #[allow(clippy::needless_pass_by_value)]
    pub fn validate_range<T: PartialOrd + fmt::Display>(
        field: &str,
        value: T,
        min: Option<T>,
        max: Option<T>,
    ) -> Result<(), ValidationError> {
        if let Some(min_val) = min
            && value < min_val
        {
            return Err(ValidationError::new(
                field,
                format!("Must be at least {min_val}"),
            ));
        }

        if let Some(max_val) = max
            && value > max_val
        {
            return Err(ValidationError::new(
                field,
                format!("Must be at most {max_val}"),
            ));
        }

        Ok(())
    }

    /// Basic email validation.
    ///
    /// # Errors
    /// Returns `Err(ValidationError)` if the value lacks `@`/`.` or exceeds 255 chars.
    pub fn validate_email(field: &str, value: &str) -> Result<(), ValidationError> {
        if !value.contains('@') || !value.contains('.') {
            return Err(ValidationError::new(field, "Invalid email format"));
        }

        if value.chars().count() > 255 {
            return Err(ValidationError::new(
                field,
                "Email must be at most 255 characters",
            ));
        }

        Ok(())
    }

    /// Validate value is not empty.
    ///
    /// # Errors
    /// Returns `Err(ValidationError)` if the trimmed value is empty.
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

    /// A7 regression: limits are in characters, not UTF-8 bytes. "José" is 4 chars
    /// but 5 bytes, and "日本語" is 3 chars but 9 bytes — both must pass a max of 4/3.
    #[test]
    fn test_validate_length_counts_characters_not_bytes() {
        use validators::validate_length;
        assert!(
            validate_length("name", "José", None, Some(4)).is_ok(),
            "4-char multibyte value must pass a 4-char max"
        );
        assert!(
            validate_length("name", "日本語", Some(3), Some(3)).is_ok(),
            "3-char CJK value must satisfy a 3-char min/max"
        );
        // And a genuinely-too-long multibyte value is still rejected.
        assert!(validate_length("name", "Joséé", None, Some(4)).is_err());
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
