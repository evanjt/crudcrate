/// Controls query filtering and field visibility for scoped (e.g., public) requests.
///
/// Inject via Axum `Extension` in middleware. When present:
/// - `get_all_handler` merges `condition` into the query filter
/// - `get_one_handler` verifies the fetched record passes the condition
/// - If scoped models exist (`exclude(scoped)` on fields), handlers return the scoped
///   model type which omits those fields from the response
///
/// Auth-system-agnostic — write middleware that converts your auth state into this.
///
/// # Example
///
/// ```rust,ignore
/// use crudcrate::ScopeCondition;
/// use sea_orm::Condition;
///
/// // Middleware: inject scope for unauthenticated users
/// if !is_admin(&req) {
///     req.extensions_mut().insert(ScopeCondition::new(
///         Condition::all().add(article::Column::IsPrivate.eq(false))
///     ));
/// }
/// ```
#[derive(Clone)]
pub struct ScopeCondition {
    pub condition: sea_orm::Condition,
}

/// Trait for types that can be filtered in scoped (public) contexts.
///
/// Types with `exclude(scoped)` boolean fields auto-implement this to return `false`
/// when the record should be hidden from scoped responses. The derive macro generates
/// the impl based on `exclude(scoped)` boolean fields.
///
/// Used by scoped `From` conversions to filter private children out of Vec and Option join fields.
pub trait ScopeFilterable {
    /// Returns `true` if this record should be visible in scoped (public) responses.
    fn is_scope_visible(&self) -> bool {
        true
    }
}

impl ScopeCondition {
    /// Create a scope with a query condition (row filtering).
    pub fn new(condition: sea_orm::Condition) -> Self {
        Self { condition }
    }
}
