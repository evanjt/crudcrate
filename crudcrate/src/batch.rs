//! Response bodies of the `?partial=true` batch endpoints.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
