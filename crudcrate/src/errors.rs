//! # Error Handling for CRUD APIs
//!
//! This module provides a comprehensive error handling system that:
//! - Returns appropriate HTTP status codes
//! - Sends sanitized, user-friendly error messages
//! - Logs detailed internal errors for debugging
//! - Prevents leaking sensitive database/system information
//!
//! ## Philosophy
//!
//! **Never expose internal errors to users**. Database errors, stack traces, and internal
//! implementation details should be logged server-side but never sent to clients.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use crudcrate::ApiError;
//!
//! async fn my_handler() -> Result<Json<MyData>, ApiError> {
//!     // Database errors are automatically converted and logged
//!     let data = MyEntity::find_by_id(id)
//!         .one(db)
//!         .await
//!         .map_err(ApiError::database)?
//!         .ok_or_else(|| ApiError::not_found("User", Some(id.to_string())))?;
//!
//!     Ok(Json(data))
//! }
//! ```
//!
//! ## Logging
//!
//! Internal errors are logged using the `tracing` crate. To enable logging, set up
//! tracing in your application:
//!
//! ```rust,ignore
//! use tracing_subscriber;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Enable tracing (optional - only if you want error logging)
//!     tracing_subscriber::fmt()
//!         .with_target(false)
//!         .compact()
//!         .init();
//!
//!     // Your app...
//! }
//! ```

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use sea_orm::DbErr;
use serde::Serialize;
use std::fmt;

/// API error type with automatic logging and sanitized responses
///
/// This enum provides different error types that map to appropriate HTTP status codes.
/// Internal errors (like database errors) are logged but not exposed to users.
#[derive(Debug)]
pub enum ApiError {
    /// 404 Not Found - Resource doesn't exist
    NotFound {
        /// Resource type (e.g., "User", "Post")
        resource: String,
        /// Optional ID that wasn't found
        id: Option<String>,
    },

    /// 400 Bad Request - Invalid input from user
    BadRequest {
        /// User-facing error message
        message: String,
    },

    /// 401 Unauthorized - Authentication required or failed
    Unauthorized {
        /// User-facing error message
        message: String,
    },

    /// 403 Forbidden - User lacks permission
    Forbidden {
        /// User-facing error message
        message: String,
    },

    /// 409 Conflict - Resource conflict (e.g., duplicate key)
    Conflict {
        /// User-facing error message
        message: String,
    },

    /// 422 Unprocessable Entity - Validation failed
    ValidationFailed {
        /// User-facing validation errors
        errors: Vec<String>,
    },

    /// 500 Internal Server Error - Database error (details logged, not exposed)
    Database {
        /// User-facing generic message
        message: String,
        /// Internal error (logged, not sent to user)
        internal: DbErr,
    },

    /// 500 Internal Server Error - Generic internal error
    Internal {
        /// User-facing generic message
        message: String,
        /// Internal error details (logged, not sent to user)
        internal: Option<String>,
    },

    /// Custom error with specific status code
    Custom {
        /// HTTP status code
        status: StatusCode,
        /// User-facing message
        message: String,
        /// Internal error details (logged, not sent to user)
        internal: Option<String>,
    },
}

impl ApiError {
    // ============================================================================
    // Constructors for common error types
    // ============================================================================

    /// Create a 404 Not Found error
    ///
    /// # Example
    /// ```rust,ignore
    /// return Err(ApiError::not_found("User", Some(user_id.to_string())));
    /// ```
    pub fn not_found(resource: impl Into<String>, id: Option<String>) -> Self {
        Self::NotFound {
            resource: resource.into(),
            id,
        }
    }

    /// Create a 400 Bad Request error
    ///
    /// # Example
    /// ```rust,ignore
    /// return Err(ApiError::bad_request("Invalid email format"));
    /// ```
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest {
            message: message.into(),
        }
    }

    /// Create a 401 Unauthorized error
    ///
    /// # Example
    /// ```rust,ignore
    /// return Err(ApiError::unauthorized("Invalid credentials"));
    /// ```
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::Unauthorized {
            message: message.into(),
        }
    }

    /// Create a 403 Forbidden error
    ///
    /// # Example
    /// ```rust,ignore
    /// return Err(ApiError::forbidden("Insufficient permissions"));
    /// ```
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Forbidden {
            message: message.into(),
        }
    }

    /// Create a 409 Conflict error
    ///
    /// # Example
    /// ```rust,ignore
    /// return Err(ApiError::conflict("Email already exists"));
    /// ```
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict {
            message: message.into(),
        }
    }

    /// Create a 422 Validation Failed error
    ///
    /// # Example
    /// ```rust,ignore
    /// return Err(ApiError::validation_failed(vec![
    ///     "Email is required".to_string(),
    ///     "Password must be at least 8 characters".to_string(),
    /// ]));
    /// ```
    pub fn validation_failed(errors: Vec<String>) -> Self {
        Self::ValidationFailed { errors }
    }

    /// Create a 500 Internal Server Error from a database error
    ///
    /// The database error details are logged but NOT sent to the user.
    ///
    /// # Example
    /// ```rust,ignore
    /// let user = entity.insert(db).await.map_err(ApiError::database)?;
    /// ```
    pub fn database(err: DbErr) -> Self {
        Self::Database {
            message: "A database error occurred".to_string(),
            internal: err,
        }
    }

    /// Create a 500 Internal Server Error with optional details
    ///
    /// # Example
    /// ```rust,ignore
    /// return Err(ApiError::internal("Failed to process request", Some(err.to_string())));
    /// ```
    pub fn internal(message: impl Into<String>, internal: Option<String>) -> Self {
        Self::Internal {
            message: message.into(),
            internal,
        }
    }

    /// Create a custom error with specific status code
    ///
    /// # Example
    /// ```rust,ignore
    /// return Err(ApiError::custom(
    ///     StatusCode::TOO_MANY_REQUESTS,
    ///     "Rate limit exceeded",
    ///     None
    /// ));
    /// ```
    pub fn custom(
        status: StatusCode,
        message: impl Into<String>,
        internal: Option<String>,
    ) -> Self {
        Self::Custom {
            status,
            message: message.into(),
            internal,
        }
    }

    // ============================================================================
    // Internal methods
    // ============================================================================

    /// Get the HTTP status code for this error
    fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::BadRequest { .. } => StatusCode::BAD_REQUEST,
            Self::Unauthorized { .. } => StatusCode::UNAUTHORIZED,
            Self::Forbidden { .. } => StatusCode::FORBIDDEN,
            Self::Conflict { .. } => StatusCode::CONFLICT,
            Self::ValidationFailed { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Database { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Custom { status, .. } => *status,
        }
    }

    /// Get the user-facing error message (sanitized)
    fn user_message(&self) -> String {
        match self {
            Self::NotFound { resource, id } => {
                if let Some(id) = id {
                    format!("{} with ID '{}' not found", resource, id)
                } else {
                    format!("{} not found", resource)
                }
            }
            Self::BadRequest { message } => message.clone(),
            Self::Unauthorized { message } => message.clone(),
            Self::Forbidden { message } => message.clone(),
            Self::Conflict { message } => message.clone(),
            Self::ValidationFailed { errors } => {
                if errors.len() == 1 {
                    errors[0].clone()
                } else {
                    format!("Validation failed: {}", errors.join(", "))
                }
            }
            Self::Database { message, .. } => message.clone(),
            Self::Internal { message, .. } => message.clone(),
            Self::Custom { message, .. } => message.clone(),
        }
    }

    /// Log internal error details (not sent to user)
    ///
    /// Uses the `tracing` crate - only logs if user has enabled tracing.
    /// No output if tracing is not configured.
    fn log_internal(&self) {
        match self {
            Self::Database { internal, .. } => {
                tracing::error!(
                    error = ?internal,
                    "Database error occurred"
                );
            }
            Self::Internal { internal: Some(details), .. } => {
                tracing::error!(
                    details = %details,
                    "Internal error occurred"
                );
            }
            Self::Custom { internal: Some(details), status, .. } => {
                tracing::error!(
                    status = %status,
                    details = %details,
                    "Custom error occurred"
                );
            }
            _ => {
                // Other errors don't have internal details to log
                // Still log at debug level for visibility
                tracing::debug!(
                    error = %self.user_message(),
                    status = %self.status_code(),
                    "API error"
                );
            }
        }
    }
}

/// Error response sent to users (sanitized)
#[derive(Serialize)]
struct ErrorResponse {
    /// Error message
    error: String,
    /// Optional list of validation errors
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Vec<String>>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Log internal error details (not sent to user)
        self.log_internal();

        let status = self.status_code();

        // Build sanitized response
        let response = match &self {
            Self::ValidationFailed { errors } => ErrorResponse {
                error: "Validation failed".to_string(),
                details: Some(errors.clone()),
            },
            _ => ErrorResponse {
                error: self.user_message(),
                details: None,
            },
        };

        (status, Json(response)).into_response()
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.user_message())
    }
}

impl std::error::Error for ApiError {}

// ============================================================================
// Conversions from common error types
// ============================================================================

/// Convert SeaORM DbErr to ApiError
///
/// Automatically maps common database errors to appropriate HTTP status codes:
/// - RecordNotFound → 404
/// - RecordNotInserted/RecordNotUpdated → 500 (logged)
/// - Other database errors → 500 (logged)
impl From<DbErr> for ApiError {
    fn from(err: DbErr) -> Self {
        match &err {
            DbErr::RecordNotFound(msg) => {
                // Try to extract resource name from error message
                let resource = msg.split_whitespace().next().unwrap_or("Resource");
                Self::NotFound {
                    resource: resource.to_string(),
                    id: None,
                }
            }
            DbErr::Custom(msg) => {
                // Custom errors from user code - these might be user-facing
                // Check if it looks like a validation error
                if msg.to_lowercase().contains("validation")
                    || msg.to_lowercase().contains("invalid")
                    || msg.to_lowercase().contains("required")
                    || msg.to_lowercase().contains("too short")
                    || msg.to_lowercase().contains("too long")
                {
                    Self::BadRequest {
                        message: msg.clone(),
                    }
                } else {
                    // Other custom errors are treated as internal
                    Self::Database {
                        message: "An error occurred".to_string(),
                        internal: err,
                    }
                }
            }
            _ => Self::Database {
                message: "A database error occurred".to_string(),
                internal: err,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_found_with_id() {
        let err = ApiError::not_found("User", Some("123".to_string()));
        assert_eq!(err.status_code(), StatusCode::NOT_FOUND);
        assert_eq!(err.user_message(), "User with ID '123' not found");
    }

    #[test]
    fn test_not_found_without_id() {
        let err = ApiError::not_found("User", None);
        assert_eq!(err.status_code(), StatusCode::NOT_FOUND);
        assert_eq!(err.user_message(), "User not found");
    }

    #[test]
    fn test_bad_request() {
        let err = ApiError::bad_request("Invalid email format");
        assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);
        assert_eq!(err.user_message(), "Invalid email format");
    }

    #[test]
    fn test_validation_failed() {
        let err = ApiError::validation_failed(vec![
            "Email is required".to_string(),
            "Password too short".to_string(),
        ]);
        assert_eq!(err.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            err.user_message(),
            "Validation failed: Email is required, Password too short"
        );
    }

    #[test]
    fn test_database_error_conversion() {
        let db_err = DbErr::RecordNotFound("User not found".to_string());
        let api_err: ApiError = db_err.into();
        assert_eq!(api_err.status_code(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_custom_validation_error_conversion() {
        let db_err = DbErr::Custom("Validation failed: content too short".to_string());
        let api_err: ApiError = db_err.into();
        assert_eq!(api_err.status_code(), StatusCode::BAD_REQUEST);
    }
}
