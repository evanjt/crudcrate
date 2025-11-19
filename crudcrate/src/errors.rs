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
/// **Conversion Rules:**
/// - `DbErr::RecordNotFound` → 404 Not Found
/// - All other `DbErr` variants → 500 Internal Server Error (logged internally, sanitized for users)
///
/// **Note:** Lifecycle hooks that return `Result<(), DbErr>` can only produce 404 or 500 errors.
/// If you need custom status codes (400, 401, 403, 409), handle errors at the handler level
/// or create custom handlers that don't use the trait system.
///
/// # Examples
///
/// ```rust,ignore
/// // In lifecycle hooks - limited to 500 or 404
/// async fn before_delete(&self, db: &DatabaseConnection, id: Uuid) -> Result<(), DbErr> {
///     if !user_has_permission(id) {
///         // This will become a 500 Internal Server Error
///         return Err(DbErr::Custom("Permission check failed".into()));
///     }
///     Ok(())
/// }
///
/// // For custom status codes, use ApiError directly in your custom handlers:
/// async fn delete_with_permission(
///     State(db): State<DatabaseConnection>,
///     Path(id): Path<Uuid>,
/// ) -> Result<StatusCode, ApiError> {
///     if !check_permission(id) {
///         return Err(ApiError::forbidden("You don't have permission to delete this resource"));
///     }
///     // ... rest of delete logic
///     Ok(StatusCode::NO_CONTENT)
/// }
/// ```
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
            // All other database errors become 500 Internal Server Error
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

    // ============================================================================
    // Constructor Tests
    // ============================================================================

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
    fn test_unauthorized() {
        let err = ApiError::unauthorized("Invalid credentials");
        assert_eq!(err.status_code(), StatusCode::UNAUTHORIZED);
        assert_eq!(err.user_message(), "Invalid credentials");
    }

    #[test]
    fn test_forbidden() {
        let err = ApiError::forbidden("Insufficient permissions");
        assert_eq!(err.status_code(), StatusCode::FORBIDDEN);
        assert_eq!(err.user_message(), "Insufficient permissions");
    }

    #[test]
    fn test_conflict() {
        let err = ApiError::conflict("Email already exists");
        assert_eq!(err.status_code(), StatusCode::CONFLICT);
        assert_eq!(err.user_message(), "Email already exists");
    }

    #[test]
    fn test_validation_failed_single_error() {
        let err = ApiError::validation_failed(vec!["Email is required".to_string()]);
        assert_eq!(err.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err.user_message(), "Email is required");
    }

    #[test]
    fn test_validation_failed_multiple_errors() {
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
    fn test_database_error() {
        let db_err = DbErr::Type("Type mismatch error".to_string());
        let err = ApiError::database(db_err);
        assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(err.user_message(), "A database error occurred");
    }

    #[test]
    fn test_internal_error_with_details() {
        let err = ApiError::internal(
            "Processing failed",
            Some("Null pointer exception".to_string()),
        );
        assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(err.user_message(), "Processing failed");
    }

    #[test]
    fn test_internal_error_without_details() {
        let err = ApiError::internal("Processing failed", None);
        assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(err.user_message(), "Processing failed");
    }

    #[test]
    fn test_custom_error() {
        let err = ApiError::custom(
            StatusCode::TOO_MANY_REQUESTS,
            "Rate limit exceeded",
            Some("User hit 100 req/min".to_string()),
        );
        assert_eq!(err.status_code(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(err.user_message(), "Rate limit exceeded");
    }

    // ============================================================================
    // DbErr Conversion Tests (Hook Error Patterns)
    // ============================================================================

    #[test]
    fn test_dberr_record_not_found_conversion() {
        let db_err = DbErr::RecordNotFound("User not found".to_string());
        let api_err: ApiError = db_err.into();
        assert_eq!(api_err.status_code(), StatusCode::NOT_FOUND);
        assert!(api_err.user_message().contains("not found"));
    }

    #[test]
    fn test_dberr_custom_becomes_internal() {
        // All DbErr::Custom variants become 500 Internal Server Error
        let db_err = DbErr::Custom("Something went wrong".to_string());
        let api_err: ApiError = db_err.into();
        assert_eq!(api_err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(api_err.user_message(), "A database error occurred");
    }

    #[test]
    fn test_dberr_type_error() {
        let db_err = DbErr::Type("Type conversion failed".to_string());
        let api_err: ApiError = db_err.into();
        assert_eq!(api_err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(api_err.user_message(), "A database error occurred");
    }

    #[test]
    fn test_dberr_json_error() {
        let db_err = DbErr::Json("JSON parsing failed".to_string());
        let api_err: ApiError = db_err.into();
        assert_eq!(api_err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(api_err.user_message(), "A database error occurred");
    }

    // ============================================================================
    // DbErr Conversion Tests - Simple Behavior
    // ============================================================================

    #[test]
    fn test_dberr_record_not_found_becomes_404() {
        // DbErr::RecordNotFound becomes 404
        let db_err = DbErr::RecordNotFound("Blog post not found".to_string());
        let api_err: ApiError = db_err.into();
        assert_eq!(api_err.status_code(), StatusCode::NOT_FOUND);
        assert!(api_err.user_message().contains("not found"));
    }

    #[test]
    fn test_all_other_dberr_become_500() {
        // All other DbErr types become 500 Internal Server Error
        let test_cases = vec![
            DbErr::Custom("Any custom error".to_string()),
            DbErr::Type("Type error".to_string()),
            DbErr::Json("JSON error".to_string()),
        ];

        for db_err in test_cases {
            let api_err: ApiError = db_err.into();
            assert_eq!(api_err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(api_err.user_message(), "A database error occurred");
        }
    }

    // ============================================================================
    // Display and Error Trait Tests
    // ============================================================================

    #[test]
    fn test_display_trait() {
        let err = ApiError::bad_request("Test error");
        assert_eq!(format!("{}", err), "Test error");
    }

    #[test]
    fn test_error_trait() {
        let err = ApiError::bad_request("Test error");
        let _: &dyn std::error::Error = &err; // Verify it implements Error trait
    }

    // ============================================================================
    // Status Code Coverage Tests
    // ============================================================================

    #[test]
    fn test_all_status_codes() {
        let test_cases = vec![
            (ApiError::not_found("Test", None), StatusCode::NOT_FOUND),
            (
                ApiError::bad_request("Test"),
                StatusCode::BAD_REQUEST,
            ),
            (
                ApiError::unauthorized("Test"),
                StatusCode::UNAUTHORIZED,
            ),
            (ApiError::forbidden("Test"), StatusCode::FORBIDDEN),
            (ApiError::conflict("Test"), StatusCode::CONFLICT),
            (
                ApiError::validation_failed(vec!["Test".to_string()]),
                StatusCode::UNPROCESSABLE_ENTITY,
            ),
            (
                ApiError::database(DbErr::Conn(sea_orm::RuntimeErr::Internal("Test".to_string()))),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (
                ApiError::internal("Test", None),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (
                ApiError::custom(StatusCode::IM_A_TEAPOT, "Test", None),
                StatusCode::IM_A_TEAPOT,
            ),
        ];

        for (err, expected_status) in test_cases {
            assert_eq!(err.status_code(), expected_status);
        }
    }
}
