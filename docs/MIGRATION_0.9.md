# Migrating from crudcrate 0.8.x to 0.9.0

0.9.0 is a security-hardening release. Most upgrades are no-code-change, but
the default `SecurityProfile` flipped from `legacy()` to `secure()`, which
changes four observable behaviors at the HTTP layer. This guide walks through
each behavior change and shows how to opt back into the 0.8.x defaults if
your frontend or downstream client depends on them.

## TL;DR

| Scenario | Action |
|---|---|
| No filter / batch-delete usage, no react-admin frontend | Bump the version. Done. |
| React-admin frontend with a custom data provider that calls `/batch` DELETE | `#[crudcrate(security_profile = "react_admin")]` per resource, or layer `Extension(SecurityProfile::react_admin())` globally. |
| Frontend or client that parses malformed `?filter=` JSON expecting it to be ignored | Either fix the client, or pin `SecurityProfile::legacy()`. |
| Frontend that joins filter from a scoped parent on an unscoped child | The 0.8.x behavior was a side-channel. Either scope the child entity (add `#[crudcrate(exclude(scoped))]` to a boolean field) or pin `legacy()`. |
| Can't migrate the client right now | Pin `SecurityProfile::legacy()` everywhere, file a follow-up ticket. |

## The default flipped

`CRUDResource::security_profile()` used to default to `SecurityProfile::legacy()`.
It now defaults to `SecurityProfile::secure()`. The full diff:

| Field | `legacy()` (0.8.x default) | `secure()` (0.9.0 default) |
|---|---|---|
| `strict_filter_parsing` | `false` | `true` |
| `scope_propagation_strict` | `false` | `true` |
| `expose_deleted_ids` | `true` | `false` |
| `max_request_body_bytes` | `2 * 1024 * 1024` | `2 * 1024 * 1024` (unchanged) |

Body limit is unconditional — `legacy()` shares the 2 MiB ceiling because the
underlying issue is pure DoS and there's no legacy use case that needs
unbounded request bodies. The other three fields are the migration concern.

## Per-flag breakdown

### `strict_filter_parsing: true`

**Before (0.8.x):** `GET /resource?filter=garbage` returned `200 OK` with the
unfiltered list. The malformed JSON was silently dropped.

**After (0.9.0):** the same request returns `400 Bad Request`.

**Migrate by fixing the client** if it ever sends invalid JSON — that's a bug
either way. **Or** keep the lenient behavior:

```rust
#[crudcrate(api_struct = "Customer", security_profile = "legacy")]
```

```rust
let app = Router::new()
    .merge(Customer::router(&db))
    .layer(Extension(SecurityProfile { strict_filter_parsing: false, ..SecurityProfile::secure() }));
```

React-admin's filter components do occasionally emit partial JSON during user
input (mid-typing, debounce races). `SecurityProfile::react_admin()` keeps
`strict_filter_parsing = false` for this reason.

### `scope_propagation_strict: true`

**Before (0.8.x):** under an active `ScopeCondition`, a request like
`?filter={"vehicles.color":"red"}` ran a sub-query on `vehicles` with no scope
restriction even if `Customer` itself was scoped. Result cardinality leaked
parent-existence — a public user could probe for the existence of private
customers via their (un-private) child columns.

**After (0.9.0):** the same request returns `400` when the join target child
entity has no `exclude(scoped)` scope-condition. The derive macro generates
`joined_field_has_scope(field) -> bool` from each `Vec<Child>` join target,
inspecting whether the child has `ScopeFilterable::scope_condition() -> Some`.

**Migrate by scoping the child entity** — add a boolean field with
`#[crudcrate(exclude(scoped))]`:

```rust
#[crudcrate(filterable, exclude(scoped, create), on_create = false)]
pub is_private: bool,
```

This is usually the right fix — if the parent is scoped, the joined child
should be too. **Or** disable the strict check for this resource:

```rust
#[crudcrate(api_struct = "Customer", security_profile = "legacy")]
```

### `expose_deleted_ids: false`

**Before (0.8.x):** `DELETE /resource/batch` with body `[id1, id2, fake1]`
returned `[id1, id2]` (only the IDs that actually existed). This leaks
existence information — a caller with delete-permission for one record could
enumerate by submitting batches of guessed UUIDs.

**After (0.9.0):** the response is `{"deleted": 2}`. Partial-mode
(`?partial=true`) returns `{"succeeded_count": 2, "failed": [{"index": 2, "error": "..."}]}`
— still useful for telling the client which input indices failed, but no IDs.

**Migrate by updating the client** to read the count, **or** if the frontend
needs the ID array (react-admin's `useDeleteMany` uses returned IDs for cache
invalidation):

```rust
#[crudcrate(api_struct = "Customer", security_profile = "react_admin")]
```

The `react_admin()` preset accepts the existence-leak trade-off as the
documented cost of frontend cache coherence. Mitigate by gating batch
endpoints behind an authenticated/authorized middleware layer.

### `max_request_body_bytes: 2 * 1024 * 1024`

This matches Axum's existing built-in default — no behavior change at the
default. The new bit is that the limit is now explicit and per-resource
configurable. To loosen or tighten per-resource:

```rust
impl CRUDResource for LargeUploadResource {
    fn security_profile() -> SecurityProfile {
        SecurityProfile {
            max_request_body_bytes: 100 * 1024 * 1024, // 100 MiB
            ..SecurityProfile::secure()
        }
    }
    // ... other associated items ...
}
```

Note: `max_request_body_bytes` is the only field that can NOT be globally
overridden via `Extension<SecurityProfile>`, because the underlying
`DefaultBodyLimit` layer is applied at router-build time, not request time.
The other three fields work fully via the global override path.

## Picking a preset

The three presets cover the common cases. Use struct-update syntax to mix
fields when none of them fit exactly:

```rust
// Like secure(), but allow ID arrays for one resource's bespoke client.
SecurityProfile { expose_deleted_ids: true, ..SecurityProfile::secure() }

// Like react_admin(), but tighter body limit on this endpoint.
SecurityProfile { max_request_body_bytes: 256 * 1024, ..SecurityProfile::react_admin() }
```

For the surveyed crudcrate-backed react-admin UIs (`spice-ui`,
`cryobiobank-ui`, `drop4crop-ui`), the recommended migration is:

```rust
// In each generate_router-using resource:
#[crudcrate(generate_router, security_profile = "react_admin")]
```

Then audit each one against the
[caveats documented on `SecurityProfile::react_admin()`](https://docs.rs/crudcrate/0.9.0/crudcrate/profile/struct.SecurityProfile.html#method.react_admin).

## Other changes

Beyond the default flip, 0.9.0 also:

- Dropped the unmaintained `impls` crate (6.5 years dormant). The replacement
  is an inline 30-line `crudcrate::impls!` macro with identical semantics.
- Replaced the unmaintained `url-escape` dev dep with `percent-encoding`.
- Bumped axum to 0.8.9 (requires Rust 1.80), sea-orm to 1.1.20, plus routine
  patches across tokio, serde_json, uuid, chrono, tower-http, utoipa.
- Documents the transitive `rsa 0.9.10` Marvin-attack risk in the
  `README.md` security section. The `mysql` feature is opt-in and the
  affected code path is server-side; consumers not using MySQL are
  unaffected.

If you hit a migration scenario this guide doesn't cover, please open an
issue with the request shape you depend on and the surprising response.
