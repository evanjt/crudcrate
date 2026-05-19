//! Security profile: deployment-shape configuration for crudcrate-generated endpoints.
//!
//! A [`SecurityProfile`] bundles the security-sensitive defaults that vary between
//! deployments — strict filter parsing, scope propagation, ID exposure in batch
//! responses, and request body size limits. Pick a preset for the common cases:
//!
//! - [`SecurityProfile::secure`] — hardened defaults. Strict scope handling, no ID
//!   enumeration via batch delete, rejects malformed filters. Recommended for any
//!   new project.
//! - [`SecurityProfile::react_admin`] — tolerates malformed filters from
//!   react-admin's filter components and returns deleted-ID arrays so react-admin's
//!   `useDeleteMany` can update its local cache.
//! - [`SecurityProfile::legacy`] — matches pre-0.9.0 crudcrate behavior exactly.
//!
//! Override individual fields with Rust's struct-update syntax:
//!
//! ```rust
//! use crudcrate::SecurityProfile;
//!
//! let profile = SecurityProfile {
//!     max_request_body_bytes: 1024 * 1024,
//!     ..SecurityProfile::react_admin()
//! };
//! ```
//!
//! Apply globally via an Axum `Extension`, or per-resource by overriding
//! [`CRUDResource::security_profile`](crate::CRUDResource::security_profile).
//! When both are present, the `Extension` wins.
//!
//! # Caveats
//!
//! `max_request_body_bytes` is enforced via a `DefaultBodyLimit` tower layer that
//! is baked into the router at startup. Setting it through a request-time
//! `Extension` cannot change the limit once the router is built — the other three
//! fields work fully at request time.
//!
//! [`CRUDResource`]: crate::CRUDResource

/// Deployment-shape configuration for crudcrate-generated endpoints.
///
/// See the [module documentation](self) for the field rationale and preset selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecurityProfile {
    /// Reject malformed JSON in `?filter=` with `400 Bad Request` instead of silently
    /// dropping the filter.
    pub strict_filter_parsing: bool,

    /// Reject joined filters that target a child entity without
    /// `#[crudcrate(exclude(scoped))]` when the request carries a `ScopeCondition`.
    pub scope_propagation_strict: bool,

    /// Include the array of deleted UUIDs in batch-delete responses. When `false`,
    /// the response is `{ "deleted": <count> }` instead of `[uuid, uuid, ...]`.
    ///
    /// Required by react-admin's `dataProvider.deleteMany`, whose `useDeleteMany` hook
    /// uses the returned IDs to update the local cache. Setting this to `true`
    /// exposes which submitted IDs existed in the database.
    pub expose_deleted_ids: bool,

    /// Maximum total request body size in bytes. Applied via Axum's `DefaultBodyLimit`
    /// layer at router-build time.
    pub max_request_body_bytes: usize,
}

const DEFAULT_BODY_LIMIT_BYTES: usize = 4 * 1024 * 1024;

impl SecurityProfile {
    /// Hardened defaults. Strict scope handling, no ID enumeration via batch delete,
    /// rejects malformed filters. Recommended for any new project.
    #[must_use]
    pub const fn secure() -> Self {
        Self {
            strict_filter_parsing: true,
            scope_propagation_strict: true,
            expose_deleted_ids: false,
            max_request_body_bytes: DEFAULT_BODY_LIMIT_BYTES,
        }
    }

    /// react-admin compatible. Tolerates malformed filters that react-admin's filter
    /// components emit during user input (partial typing, debounce races) and returns
    /// deleted-ID arrays so react-admin's `useDeleteMany` can update the local cache.
    ///
    /// Trade-offs:
    /// - `expose_deleted_ids = true` leaks DB row existence via batch delete response.
    ///   Mitigate by gating batch endpoints behind authenticated middleware.
    /// - `strict_filter_parsing = false` means clients can probe for unfiltered
    ///   responses by sending malformed JSON.
    #[must_use]
    pub const fn react_admin() -> Self {
        Self {
            strict_filter_parsing: false,
            scope_propagation_strict: true,
            expose_deleted_ids: true,
            max_request_body_bytes: DEFAULT_BODY_LIMIT_BYTES,
        }
    }

    /// Pre-0.9.0 crudcrate behavior. Provided so consumers can bump to 0.9.0 without
    /// behavior changes, then opt into stricter settings incrementally.
    #[must_use]
    pub const fn legacy() -> Self {
        Self {
            strict_filter_parsing: false,
            scope_propagation_strict: false,
            expose_deleted_ids: true,
            max_request_body_bytes: DEFAULT_BODY_LIMIT_BYTES,
        }
    }
}

/// Resolve the effective `SecurityProfile` for a request.
///
/// Priority order:
/// 1. `axum::Extension<SecurityProfile>` injected into the request (global override).
/// 2. The per-resource `CRUDResource::security_profile()` default (via `fallback`).
///
/// Generated handlers call this with the per-resource trait method as the fallback so a
/// global Extension layer transparently overrides every resource without touching each
/// `impl CRUDResource`.
#[must_use]
pub fn resolve<F>(
    extension: Option<axum::Extension<SecurityProfile>>,
    fallback: F,
) -> SecurityProfile
where
    F: FnOnce() -> SecurityProfile,
{
    extension.map_or_else(fallback, |axum::Extension(profile)| profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_preset_values() {
        let p = SecurityProfile::secure();
        assert!(p.strict_filter_parsing);
        assert!(p.scope_propagation_strict);
        assert!(!p.expose_deleted_ids);
        assert_eq!(p.max_request_body_bytes, 4 * 1024 * 1024);
    }

    #[test]
    fn test_react_admin_preset_values() {
        let p = SecurityProfile::react_admin();
        assert!(!p.strict_filter_parsing);
        assert!(p.scope_propagation_strict);
        assert!(p.expose_deleted_ids);
        assert_eq!(p.max_request_body_bytes, 4 * 1024 * 1024);
    }

    #[test]
    fn test_legacy_preset_values() {
        let p = SecurityProfile::legacy();
        assert!(!p.strict_filter_parsing);
        assert!(!p.scope_propagation_strict);
        assert!(p.expose_deleted_ids);
        assert_eq!(p.max_request_body_bytes, 4 * 1024 * 1024);
    }

    #[test]
    fn test_presets_are_distinct() {
        assert_ne!(SecurityProfile::secure(), SecurityProfile::react_admin());
        assert_ne!(SecurityProfile::secure(), SecurityProfile::legacy());
        assert_ne!(SecurityProfile::react_admin(), SecurityProfile::legacy());
    }

    #[test]
    fn test_struct_update_override() {
        let overridden = SecurityProfile {
            expose_deleted_ids: true,
            ..SecurityProfile::secure()
        };
        assert!(overridden.strict_filter_parsing);
        assert!(overridden.scope_propagation_strict);
        assert!(overridden.expose_deleted_ids);
        assert_eq!(overridden.max_request_body_bytes, 4 * 1024 * 1024);
    }

    #[test]
    fn test_react_admin_keeps_strict_scope() {
        // react_admin() relaxes filter parsing and ID exposure but never scope.
        assert!(SecurityProfile::react_admin().scope_propagation_strict);
    }

    #[test]
    fn test_body_limit_is_4mib_default() {
        // 4 MiB = 4 * 1024 * 1024 = 4194304
        assert_eq!(SecurityProfile::secure().max_request_body_bytes, 4_194_304);
        assert_eq!(SecurityProfile::react_admin().max_request_body_bytes, 4_194_304);
        assert_eq!(SecurityProfile::legacy().max_request_body_bytes, 4_194_304);
    }

    #[test]
    fn test_const_constructibility() {
        // Compile-time check that presets work in const context.
        const _SECURE: SecurityProfile = SecurityProfile::secure();
        const _RA: SecurityProfile = SecurityProfile::react_admin();
        const _LEGACY: SecurityProfile = SecurityProfile::legacy();
    }

    #[test]
    fn test_clone_copy_traits() {
        let p = SecurityProfile::secure();
        let q = p;
        let r = p;
        assert_eq!(q, r);
    }

    #[test]
    fn test_resolve_extension_wins() {
        let resolved = resolve(
            Some(axum::Extension(SecurityProfile::secure())),
            SecurityProfile::legacy,
        );
        assert_eq!(resolved, SecurityProfile::secure());
    }

    #[test]
    fn test_resolve_falls_back_when_no_extension() {
        let resolved = resolve(None, SecurityProfile::react_admin);
        assert_eq!(resolved, SecurityProfile::react_admin());
    }

    #[test]
    fn test_resolve_extension_beats_any_fallback() {
        // Even when the per-resource fallback is more permissive, the global
        // Extension wins.
        let resolved = resolve(
            Some(axum::Extension(SecurityProfile::secure())),
            SecurityProfile::legacy,
        );
        assert!(resolved.strict_filter_parsing);
        assert!(!resolved.expose_deleted_ids);
    }
}
