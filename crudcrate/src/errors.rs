//! Error types for CRUD handlers.
//!
//! [`ApiError`] maps to HTTP status codes and implements [`IntoResponse`].
//! Internal details (database errors, stack traces) are logged via `tracing` but never
//! sent to clients.
//!
//! ```rust,ignore
//! use crudcrate::ApiError;
//!
//! async fn my_handler() -> Result<Json<MyData>, ApiError> {
//!     let data = MyEntity::find_by_id(id)
//!         .one(db)
//!         .await
//!         .map_err(ApiError::database)?
//!         .ok_or_else(|| ApiError::not_found("User", Some(id.to_string())))?;
//!     Ok(Json(data))
//! }
//! ```
//!
//! `DbErr` converts automatically: `RecordNotFound` becomes 404,
//! everything else becomes 500.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sea_orm::DbErr;
use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

// ============================================================================
// Batch Result Types for Partial Success
// ============================================================================

/// A single failure in a batch operation
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BatchFailure {
    /// The index of the failed item in the original request (0-based)
    pub index: usize,
    /// The error message describing why this item failed
    pub error: String,
}

/// Result of a batch operation that may have partial success
///
/// Used when `?partial=true` is specified on batch endpoints.
/// Returns HTTP 207 Multi-Status when some items succeed and some fail.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BatchResult<T> {
    /// Items that were successfully processed
    pub succeeded: Vec<T>,
    /// Items that failed, with their original indices and error messages
    pub failed: Vec<BatchFailure>,
}

impl<T> BatchResult<T> {
    /// Create a new empty batch result
    #[must_use]
    pub fn new() -> Self {
        Self {
            succeeded: Vec::new(),
            failed: Vec::new(),
        }
    }

    /// Add a successful item
    pub fn add_success(&mut self, item: T) {
        self.succeeded.push(item);
    }

    /// Add a failed item
    pub fn add_failure(&mut self, index: usize, error: impl Into<String>) {
        self.failed.push(BatchFailure {
            index,
            error: error.into(),
        });
    }

    /// Returns true if all items failed
    #[must_use]
    pub fn all_failed(&self) -> bool {
        self.succeeded.is_empty() && !self.failed.is_empty()
    }

    /// Returns true if all items succeeded
    #[must_use]
    pub fn all_succeeded(&self) -> bool {
        !self.succeeded.is_empty() && self.failed.is_empty()
    }

    /// Returns true if some items succeeded and some failed
    #[must_use]
    pub fn is_partial(&self) -> bool {
        !self.succeeded.is_empty() && !self.failed.is_empty()
    }
}

impl<T> Default for BatchResult<T> {
    fn default() -> Self {
        Self::new()
    }
}

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
    #[must_use]
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
    #[must_use]
    pub fn database(err: DbErr) -> Self {
        // A unique-constraint violation is a client error (409), not an opaque 500.
        // The generated create/update handlers wrap insert/update errors via this
        // function (`.map_err(ApiError::database)`) and return `Result<_, ApiError>`,
        // so the `From<DbErr>` impl is bypassed on those paths; the mapping must live
        // here too for the documented 409 "Duplicate record" response to be reachable.
        if let Some(sea_orm::SqlErr::UniqueConstraintViolation(_)) = err.sql_err() {
            return Self::Conflict {
                message: "A record with these details already exists".to_string(),
            };
        }
        // A foreign-key violation (referencing a missing record, or removing one that
        // is still referenced) is likewise a client conflict (409), not an opaque 500,
        // matching the documented response. The message is generic so no driver text leaks.
        if let Some(sea_orm::SqlErr::ForeignKeyConstraintViolation(_)) = err.sql_err() {
            return Self::Conflict {
                message: "The operation conflicts with a related record".to_string(),
            };
        }
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
            Self::Database { .. } | Self::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Custom { status, .. } => *status,
        }
    }

    /// Get the user-facing error message (sanitized)
    fn user_message(&self) -> String {
        match self {
            Self::NotFound { resource, id } => {
                if let Some(id) = id {
                    format!("{resource} with ID '{id}' not found")
                } else {
                    format!("{resource} not found")
                }
            }
            Self::ValidationFailed { errors } => {
                if errors.len() == 1 {
                    errors[0].clone()
                } else {
                    format!("Validation failed: {}", errors.join(", "))
                }
            }
            Self::BadRequest { message }
            | Self::Unauthorized { message }
            | Self::Forbidden { message }
            | Self::Conflict { message }
            | Self::Database { message, .. }
            | Self::Internal { message, .. }
            | Self::Custom { message, .. } => message.clone(),
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
            Self::Internal {
                internal: Some(details),
                ..
            } => {
                tracing::error!(
                    details = %details,
                    "Internal error occurred"
                );
            }
            Self::Custom {
                internal: Some(details),
                status,
                ..
            } => {
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

/// Convert `SeaORM` `DbErr` to `ApiError`
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
        // A unique-constraint violation is a client error (409), not an opaque
        // 500. sea-orm normalises the driver-specific signal (Postgres SQLSTATE
        // 23505, SQLite "UNIQUE constraint failed", etc.) into `SqlErr`, so this
        // detection is database-agnostic. We don't reflect the raw driver text.
        if let Some(sea_orm::SqlErr::UniqueConstraintViolation(_)) = err.sql_err() {
            return Self::Conflict {
                message: "A record with these details already exists".to_string(),
            };
        }
        // Foreign-key violations are also normalised by sea-orm across backends and
        // map to 409 Conflict rather than an opaque 500.
        if let Some(sea_orm::SqlErr::ForeignKeyConstraintViolation(_)) = err.sql_err() {
            return Self::Conflict {
                message: "The operation conflicts with a related record".to_string(),
            };
        }
        match &err {
            DbErr::RecordNotFound(msg) => {
                // Try to extract resource name from error message. Only accept
                // alphanumeric/underscore identifiers; fall back to "Resource"
                // for anything else so we never reflect arbitrary text from the
                // DB driver into the client-facing response.
                let resource = msg
                    .split_whitespace()
                    .next()
                    .filter(|s| {
                        !s.is_empty()
                            && s.len() <= 64
                            && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    })
                    .unwrap_or("Resource");
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

/// A failed [`crate::validation::Validatable::validate`] maps to 422 Unprocessable
/// Entity. The field/message are safe to surface (they come from the application's
/// own validation logic, not the database driver).
impl From<crate::validation::ValidationError> for ApiError {
    fn from(err: crate::validation::ValidationError) -> Self {
        Self::ValidationFailed {
            errors: vec![err.to_string()],
        }
    }
}

#[cfg(test)]
#[path = "errors_tests.rs"]
mod tests;
