# Changelog

All notable changes to the crudcrate project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Join loading at depth 2 or more read `related_model.id`, so a child entity
  whose primary key field has another name failed to compile. Loaders now go
  through the new provided `CRUDResource::pk_value(&model)`, which the derive
  overrides with the actual field.
- Generated routers named `tracing::info!` without qualification, so a crate
  without a direct `tracing` dependency failed to compile. Generated code now
  goes through `crudcrate::tracing`.
- Column variant idents for field names with a digit boundary (`is_2fa_enabled`)
  were built with `convert_case` (`Is2FaEnabled`) while Sea-ORM names them with
  heck (`Is2faEnabled`), so such entities failed to compile. All column idents
  now use heck.

### Deprecated

- Relying on the `use` items that `crud_handlers!` leaks into the calling
  module (`StatusCode`, `Json`, `Path`, ...). Import them directly; the leak
  is removed in the next breaking release. The macro no longer needs
  `CRUDResource` in scope at the call site.
Scheduled for removal in the next breaking release:

- `UuidIdResult`, unused since `delete_many` became generic over the primary key type.
- `filtering::sort::generic_sort`: parses every value as a JSON pair, so a plain
  column name falls back to the default column. Use `parse_sorting`.
- `ValidationErrors`: generated validation returns the singular `ValidationError`.
- `DefaultCRUDOperations`: never constructed by generated code.
- `BatchResult::all_succeeded`: check `failed.is_empty()`.
- `generate_crud_router!`: collapses every response model to the resource type.
  Use `#[crudcrate(generate_router)]`.
- The five- and three-argument arms of `crud_handlers!`.
- The `crudcrate::database` and `crudcrate::relationships` modules, which contain no items.

### Removed

- The empty, hidden `crudcrate::routes` module.
- The `spring-rs` feature and its optional `spring` and `spring-web` dependencies.
  No code was gated on it; spring-web applications mount the generated axum
  router directly.

### Changed

- `crudcrate::core::crud_operations` (the handler macros, no nameable items) is
  now `crudcrate::core::handler_macros`. The macros are unchanged and still
  exported at the crate root.
- The handler macros name `apply_filters`, `parse_pagination`, `FilterOptions`,
  `calculate_content_range` and `parse_sorting` at the crate root instead of
  through the `filter`, `models`, `pagination` and `sort` alias modules. The
  aliases remain exported.
- Generated code is pinned by expansion snapshots in
  `crudcrate-derive/tests/expand`, and `test_suite` carries a compile-only test
  naming every `crudcrate::` path generated code emits.

## [0.10.1] - 2026-08-20

### Fixed

- **`get_one_scoped` ignored `read::one::body`.** The scoped get-one handler
  had no custom-body branch, so a resource combining a `ScopeCondition` with a
  custom get-one body silently ran the default query instead. Whatever the
  body existed to do, extra filtering, joins, shaping, never happened on
  scoped requests, and rows the body would have refused were served. The
  handler now checks scope eligibility in SQL first, answers 404 when the
  scope excludes the row, then delegates to the custom body with the same
  pre/transform/post hooks as the unscoped path.
- **Scoped models were generated but never mounted for non-db exclusions.**
  Model generation and router wiring used different definitions of "has
  scoped fields": the models counted `exclude(scoped)` on join and
  `non_db_attr` fields, the router did not. When such a field was the only
  exclusion, the `ScopedList`/`ScopedOne` structs existed but the router
  served the plain models, so the supposedly excluded field still reached
  scoped callers. Both sites now share one predicate.
- **`require_scope` write semantics documented and pinned.** The flag governs
  reads only: an unscoped read fails with 500, while writes are governed
  solely by scope presence (403 when a `ScopeCondition` is present, allowed
  when absent). That split is what lets an application mount its scope on
  safe methods only, so writes arrive unscoped deliberately. The trait doc,
  the multi-tenant guide and new enforcement tests now state the contract
  explicitly. No behaviour change.
- **Validation docs described an API that does not exist.**
  `docs/src/advanced/validation.md` showed a
  `ValidationFailed(Vec<ValidationError>)` tuple variant and never mentioned
  the `Validatable` trait, the mechanism the handlers actually invoke. The
  page now documents `Validatable` and the real
  `ApiError::validation_failed(Vec<String>)` constructor.

## [0.10.0] - 2026-08-19

### Changed

- **SeaORM 2.0.** The workspace now builds against `sea-orm = "2.0"`
  (SeaQuery 1.0, SQLx 0.9). Applications must upgrade their own `sea-orm`
  dependency in the same step; crudcrate 0.9.x remains available for
  SeaORM 1.x. See `docs/MIGRATION_0.10.md`.
- **Batch create uses a single multi-row `INSERT ... RETURNING`** on backends
  that support it (PostgreSQL, SQLite >= 3.35); MySQL keeps the per-row insert
  loop inside the transaction. Response order, all-or-nothing semantics, 409
  duplicate-key mapping and `?partial=true` behaviour are unchanged. The
  `sqlite` feature now enables SeaORM's `sqlite-use-returning-for-3_35`.
  Behaviour change: custom `ActiveModelBehavior::before_save`/`after_save`
  implementations are no longer invoked per row during batch create on the
  RETURNING backends (they still run on MySQL, and single create never invoked
  them). Response order relies on the engine emitting RETURNING rows in VALUES
  order, which PostgreSQL and SQLite implement but do not formally guarantee.

### Added

- **Dense entity format support.** Entities written with SeaORM 2.0's
  `#[sea_orm::model]` (inline `HasMany`/`HasOne`/`BelongsTo` relation fields)
  can derive `EntityToModels` directly. The derive attaches to the scalar
  `Model` and ignores the generated `ModelEx` companion, so relation wrapper
  fields never leak into Create/Update/List models.
- **`crudcrate::table_column_ref(table, column)`** builds a table-qualified
  `ColumnRef` for custom `resolve_joined_filters` implementations, replacing
  the `ColumnRef::TableColumn` variant removed in SeaQuery 1.0.
- **Guided compile errors for misplaced dense-format relations.** A
  `HasMany`/`HasOne`/`BelongsTo` field on a struct not processed by
  `#[sea_orm::model]` now produces a spanned error naming the fix instead of
  silently generating nothing. Both crates also declare `rust-version = "1.85"`
  so an old toolchain fails with a clear MSRV message.
- **`#[crudcrate(deny_unknown_fields)]`.** Opt-in strict input models: the
  generated `<Name>Create` and `<Name>Update` reject a payload key they do not
  accept instead of ignoring it. Without it, a value sent for a field excluded
  from create or update is silently dropped and the response reports the stored
  value rather than the submitted one. Off by default, since clients that
  round-trip a full record into an update legitimately send read-only fields.
- **`crudcrate::sea_orm` re-export.** Applications can reference the SeaORM
  version crudcrate builds against (ie. `use crudcrate::sea_orm;`) instead of
  pinning a matching `sea-orm` dependency themselves.

### Fixed

- **List pagination could repeat or skip a row.** `get_all` ordered by the
  requested sort column alone, so rows tied on that column had no defined order
  and two `LIMIT`/`OFFSET` queries serving two pages could place a tied row in
  both or neither. The primary key is now appended as a secondary sort key
  whenever the sort column is not the primary key, in the trait default
  `get_all`, the derive-generated `get_all`/`get_all_scoped`, the joined
  (dot-notation) sort, and `CRUDOperations::fetch_all`. A custom
  `get::all::body` override supplies its own ordering and is unaffected.
- **List models dropped field attributes.** `<Name>List` was generated with the
  field name and type only, so `#[serde(skip_serializing_if = ...)]`,
  `#[serde(rename = ...)]`, `#[schema(...)]` and doc comments applied to the
  single-record response but not to entries in the collection response, e.g. a
  field skipped when `None` came back as an explicit `null` in a list. `serde`,
  `schema` and doc attributes now carry over to `<Name>List`, `<Name>ScopedList`
  and `<Name>ScopedResponse`, matching `<Name>Response`, which already did this.
- **`per_page=0` produced an empty page.** `parse_pagination` clamped only the
  upper bound, returning a limit of 0 that yields a page with no rows however
  many rows match (and a zero page size that some paginators reject outright).
  It now clamps to `1..=MAX_PAGE_SIZE`.

### Notes

- SeaORM 2.0's Entity Loader was evaluated and not adopted for join loading:
  it requires the dense entity format on user entities and offers no hook for
  per-child scope conditions. Typed `COLUMN` constants were likewise evaluated
  and skipped; generated code keeps using the `Column` enum paths. Rationale
  in the relationships documentation.

## [0.9.3] - 2026-07-15

### Fixed

- **Comparison filters on non-text columns returned 500 on PostgreSQL.** A
  `_gte`/`_lte`/`_gt`/`_lt`/`_neq` (or equality) filter whose value arrived as a
  JSON string wrapped the column in `UPPER(col)`, which PostgreSQL rejects for
  date, timestamp, numeric, uuid and boolean columns
  (`function upper(timestamp with time zone) does not exist`); SQLite's loose
  typing hid it, and even there the comparison ordered lexically (`'9' > '10'`).
  Comparison values are now parsed to the column's Sea-ORM `ColumnType` and bound
  as a typed parameter, so the backend compares natively. Text and enum columns
  keep case-insensitive matching. Requires the `with-chrono` and
  `with-rust_decimal` Sea-ORM features (now enabled by default).

## [0.9.2] - 2026-06-10

### Added

- **Non-UUID primary keys.** `CRUDResource` is now generic over the entity's
  primary-key value type via `crudcrate::PrimaryKeyType<Self>`, so integer
  (`i32`), string, and other Sea-ORM key types work end to end (CRUD, batch,
  filtering, sorting, pagination, relationship loading, and the generated Axum
  routes). UUID remains fully supported and unchanged.
- **Joined sorting.** `sort=["children.column","ASC|DESC"]` now actually orders
  parent rows by the joined child column, via a correlated sub-query
  (`ORDER BY (SELECT MIN(child.col) FROM child WHERE child.fk = parent.pk)`).
  Previously the dot-notation sort was parsed but silently ignored.
- **Automatic `Validatable` invocation.** When a Create/Update model implements
  `crudcrate::validation::Validatable`, the generated create/update/batch
  handlers now call `validate()` before any write (HTTP 422 on failure). Models
  that don't implement it are unaffected (no-op).
- **Multi-backend coverage in CI.** The PostgreSQL and MySQL CI jobs now upload
  `cargo llvm-cov` results to Codecov, so backend-specific code paths are
  measured (previously only the SQLite job reported coverage).

### Changed

- **BREAKING (trait signatures).** `CRUDResource` and `CRUDOperations` methods
  that took `id: uuid::Uuid` / `Vec<uuid::Uuid>` now take
  `crudcrate::PrimaryKeyType<Self>` / `Vec<PrimaryKeyType<Self>>`, including
  `get_one`, `get_one_scoped`, `update`, `delete`, `delete_many`, `update_many`
  and the `CRUDOperations` `before_*` / `after_*` / `perform_*` / `fetch_*`
  hooks. For UUID resources `PrimaryKeyType<Self>` resolves to `Uuid`, so runtime
  behavior and the generated routes are unchanged. **Migration:** downstream code
  that *overrides* any of these trait methods/hooks must change the `Uuid`
  parameter/return types to `crudcrate::PrimaryKeyType<Self>` (or the concrete PK
  type). Code that only derives `EntityToModels` needs no change.
- `POST /{resource}/batch?partial=true` now builds each item with `create_many`
  semantics, so partial and all-or-nothing batch create return the same
  (flat) response shape and run the same `create::many` hooks. Single
  `POST /{resource}` still applies join loading + `read::one::transform`.
- `delete_many` de-duplicates the returned ids so the reported count matches the
  rows actually deleted.
- Self-referencing `#[schema(no_recursion)]` detection now uses an exact
  inner-type match (not a substring), and deriving `Eq` now also derives
  `PartialEq`.

### Fixed

- **LIKE wildcard escaping was a no-op on SQLite** for the fulltext LIKE-fallback
  and joined `_like` paths: escaping used a backslash but emitted no `ESCAPE`
  clause, so user-supplied `%`/`_` stayed active wildcards. These paths now use
  the `!` escape convention with an explicit `ESCAPE '!'` clause.
- **Panic on oversized multi-byte search queries.** Fulltext truncation sliced at
  a fixed byte index and panicked when it landed mid-UTF-8-codepoint; it now
  snaps to a char boundary.
- **React-Admin `range` pagination overflow** on a huge `end` value (now uses
  saturating arithmetic).
- **Scoped `get_one` masked all errors as 404**, hiding DB/internal faults; it now
  propagates the real error (a scope miss is still 404).
- **Validation length/email helpers counted UTF-8 bytes, not characters**, wrongly
  rejecting multibyte input within the advertised character limit.
- **`Option<T>` belongs_to relations were not loaded in list endpoints** (the batch
  loader resolved the FK in the wrong direction); now resolved via `find_related`.
- **Duplicate-key inserts returned 500** instead of the documented 409 Conflict.
- **Foreign-key violations returned 500** instead of 409 Conflict; they are now
  mapped to 409 like unique-constraint violations (matching the documented
  response), across all backends.
- **Array filters on integer/boolean columns returned 500 on PostgreSQL.**
  `?filter={"id":[1,3]}` built `id IN ('1','3')` (string literals), which
  PostgreSQL rejects as `operator does not exist: integer = text`; SQLite's loose
  typing hid it. Array values are now bound as their native JSON type.
- **Generated `depth > 1` join code required a `tracing` dependency** in the
  downstream crate (it emitted unqualified `tracing::warn!`). The generated calls
  are now qualified as `crudcrate::tracing::warn!`, so consumers no longer need to
  depend on `tracing` directly.
- Examples: corrected `error_handling` printed messages to match `ApiError`
  output, fixed the misleading `recursive_join`/`recursive_join_5_levels` depth
  claims, trimmed `joined_filter` boilerplate, and listed all examples in the
  examples README.

### Security

- **Partial batch-delete leaked row existence under the secure profile.** With
  `expose_deleted_ids = false`, the `?partial=true` response serialized per-item
  not-found errors (embedding the submitted UUIDs), re-creating the
  enumeration oracle that profile is meant to close. It now returns
  `failed_count` instead of the per-item `failed` list.

### Known limitations

- Joined-filter sub-queries and `Vec<Child>` batch loading apply no per-relation
  row cap; they are bounded only by `MAX_FILTER_CLAUSES` (100) and the parent
  page size. On very large child tables this is a query-amplification
  consideration for untrusted callers.
- Case-insensitive fulltext and `like_filterable` matching on MySQL/SQLite is
  ASCII-only (Rust-side `to_uppercase()` vs the database's `UPPER()`); PostgreSQL
  (ILIKE) is unaffected.
- Join loading assumes the entity's primary-key *field* is named `id`. The PK
  value *type* is generic, but a join target whose PK field has a different name
  will not compile.

## [0.9.1] - 2026-06-01

### Fixed

- **LIKE queries broken on Postgres**. `build_like_condition` and the
  fulltext search functions used `?` as the bind placeholder in
  `Expr::cust_with_values` templates. Sea-query's Postgres backend uses
  `$` as its placeholder character, so the `?` was passed through as a
  literal, and Postgres then parsed `? ESCAPE '!'` as a JSONB operator
  followed by a type cast, producing `type "escape" does not exist`.
  Fixed by using `$1` for Postgres and `?` for MySQL/SQLite.

- **`build_like_condition` missing `ESCAPE` clause**. The LIKE condition
  for `like_filterable` fields used sea-query's `.like()` which never
  emitted an `ESCAPE` clause. Rewritten to use `Expr::cust_with_values`
  with `ESCAPE '!'`, matching the fulltext functions.

- **Fulltext search on non-text columns**. The fallback LIKE search path
  (when `fulltext_searchable_columns()` is empty) applied
  `UPPER(col) LIKE ...` to all searchable columns including booleans.
  Postgres and MySQL reject `UPPER(boolean)`. Fixed by casting columns
  to `TEXT` (Postgres/SQLite) or `CHAR` (MySQL) before `UPPER()`.

- **LIKE escape character conflicts with Postgres string quoting**.
  Switched `escape_like_wildcards` from backslash to `!` as the escape
  character. Backslash inside single-quoted SQL strings is ambiguous
  across backends (Postgres `standard_conforming_strings`).

### Changed

- Removed dead codegen helpers (`runtime_fk_*` functions) from
  `crudcrate-derive`.

- Resolved clippy warnings across the workspace (collapsible ifs,
  duplicate match arms, missing error docs, `unwrap` after `is_some`).

- Updated trybuild snapshot for rustc 1.96.

## [0.9.0] - 2026-05-19

### Security

- **`SecurityProfile` config struct + presets**. New `crudcrate::SecurityProfile`
  bundles the security-sensitive runtime defaults (strict filter parsing,
  scope propagation, deleted-ID exposure, and request body size) under one
  type with three presets: `SecurityProfile::secure()`, `react_admin()`, and
  `legacy()`. Override individual fields via Rust's struct-update syntax:
  `SecurityProfile { expose_deleted_ids: true, ..SecurityProfile::secure() }`.

- **Per-resource override via derive attribute**.
  `#[crudcrate(security_profile = "secure" | "react_admin" | "legacy")]`
  generates a `CRUDResource::security_profile()` impl that returns the named
  preset.

- **Global override via Axum extension**. Apply
  `.layer(Extension(SecurityProfile::secure()))` on your router to override the
  per-resource setting at request time. Resolution order:
  `Extension > CRUDResource::security_profile() > trait default`.

- **Default profile flipped to `secure()`**. New resources ship hardened
  defaults. See [MIGRATION_0.9.md](https://github.com/evanjt/crudcrate/blob/main/docs/MIGRATION_0.9.md) for the per-flag
  breakdown and opt-out instructions.

- **Explicit batch body limit**. The generated router now applies an
  Axum `DefaultBodyLimit::max(...)` layer derived from
  `SecurityProfile::max_request_body_bytes` (default 2 MiB, matching axum-core's
  baseline). Previous behavior relied on Axum's implicit default and broke if
  any consumer wired `DefaultBodyLimit::disable()` up the tree.

- **Scope-propagation side-channel guard**. Under `secure()` profile,
  joined filters (`?filter={"vehicles.color":"..."}`) on a child entity that
  has no `exclude(scoped)` scope condition are rejected with `400 Bad Request`
  when the request carries a `ScopeCondition`. Prevents parent-existence
  side-channels via unscoped child columns.

- **Strict filter parsing**. Under `secure()` profile, a malformed
  `?filter=...` value returns `400` instead of silently dropping the filter
  and returning the unfiltered result.

- **Deleted-ID enumeration guard**. Under `secure()` profile, batch delete
  responses return `{"deleted": N}` instead of the array of UUIDs that
  actually existed in the database, removing the existence-enumeration
  side-channel through the delete endpoint. react-admin frontends that rely on
  the ID array for cache invalidation should pin
  `SecurityProfile::react_admin()` or `legacy()`.

- **Fulltext SQL bind parameterization**. The Postgres / MySQL /
  SQLite fulltext condition builders now route the user query value through
  `Expr::cust_with_values` so the value is bound as a parameter rather than
  interpolated into the SQL string. Defense-in-depth: column names were
  already compile-time-known, but raw `SimpleExpr::Custom(format!(...))` was
  removed everywhere user input could reach it.

### Fixed

- **Join loading with `operations` attribute**. Entities using
  `#[crudcrate(operations = MyOps)]` for create/update/delete hooks had
  their join loading silently bypassed on `get_one` and `get_all`; the
  codegen delegated entirely to `CRUDOperations` which does plain queries
  with no relation loading. The operations path now falls through to the
  standard join-loading codegen when the entity has `join(...)` fields,
  with `before_get_one`/`after_get_one` and `before_get_all`/`after_get_all`
  hooks wrapping the join-loaded body. `get_one_scoped` and `get_all_scoped`
  are also generated for this path (previously missing entirely).

- **FK column resolution in batch loading**. The batch loader and join
  loader now resolve FK columns from the SeaORM `RelationDef` at runtime
  instead of guessing from the struct name convention. Joins with
  non-standard FK names (eg. `author_ref` instead of `author_id`) now
  load correctly.

### Changed

- Replaced unmaintained `impls = "1"` (no release since 2019) with an
  inline `crudcrate::impls!` macro. Same autoref-specialization semantics,
  30 LOC, no behavior change.

- Workspace dependencies bumped: `axum 0.8.6 → 0.8.9`, `sea-orm 1.1.19 →
  1.1.20`, `serde_json → 1.0.149`, `uuid → 1.23.1`, `tokio → 1.52.3`,
  `chrono → 0.4.44`, `tower-http → 0.6.11`, `utoipa → 5.5.0`, plus
  proc-macro and `rust_decimal` patches.

- `url-escape` (unmaintained dev dep) replaced with `percent-encoding`.

### Documentation

- `README.md`: added security caveat for the `mysql` feature, which pulls in
  `rsa 0.9.10` (RUSTSEC-2023-0071, Marvin attack, no upstream fix).

## [0.8.1] - 2026-05-19

### Security

- **Filter clause limit**. Requests with more than 100 filter keys are
  rejected with `400 Bad Request` (`MAX_FILTER_CLAUSES = 100`). Prevents
  query-planning DoS via oversized filter payloads.

- **DB error sanitization**. Internal database error messages are stripped
  from client-facing responses. Only a generic prefix is returned; the
  full error is logged via `tracing`.

### Added

- **Joined filters are now applied by the default handler**. Requests like
  `GET /customers?filter={"vehicles.make":"BMW"}` previously parsed and
  whitelisted the filter but silently dropped it before hitting the
  database; users got unfiltered results. The default `get_all_handler`
  now resolves each `JoinedFilter` into a sub-query on the child table
  (with the child's `ScopeFilterable::scope_condition()` applied), collects
  matching parent-FK values, and adds `id IN (...)` to the main condition.
  Query shape: one extra `SELECT parent_fk FROM child WHERE ...` per
  joined-filter field plus the usual list + count queries: no JOIN, no
  `DISTINCT`. Backed by `test_suite/tests/joined_filter_http_test.rs` and
  a runnable `cargo run --example joined_filter`.

- **New `CRUDResource::resolve_joined_filters` trait method**. Takes the
  parsed condition plus the `&[JoinedFilter]` list and returns the
  augmented condition to use for both the list query and the count query.
  Default impl logs and returns the condition unchanged (backward
  compatible for non-derive users); the derive macro generates an override
  for every resource that declares `join(..., filterable(...))` on any
  `Vec<Child>` field.

- **New public helper `crudcrate::build_comparison_expr`**. Translates a
  column + `FilterOperator` + `serde_json::Value` into an
  `Option<SimpleExpr>` for use in custom filter resolvers.

### Changed

- `crudcrate::filtering::ParsedFilters::joined_filters` is now consumed by
  the handler (previously only populated by the parser and read by tests).
  No API change; the field was already public.

- Pruned unused dependencies from the workspace.

### Documentation

- `docs/src/features/filtering.md` "Filtering on Related Entities"
  rewritten to describe the actual query shape, scope-safety guarantees,
  and the `Vec<Child>`-only limitation. Removed the stale "requires a
  custom `read::many::body` hook" note.
- `docs/src/features/relationships.md` migrated from the deprecated
  `join_filterable(...)` / `join_sortable(...)` syntax to the current
  `filterable(...)` / `sortable(...)` inside `join(...)`.

## [0.8.0] - 2026-04-17

### Security

- **Atomic scope check in `get_one`**: Scoped `get_one` requests now verify the scope condition in a single query (ID + scope filter), eliminating a TOCTOU race where a separate `total_count()` verification could see stale data between the fetch and the check.
- **FK column runtime validation**: The derive macro generates `#[cfg(test)]` functions that verify convention-derived FK column names match the actual `RelationDef` from SeaORM at test time. Catches silent data mismatches from FK naming convention violations before they reach production.
- **SQL-level scope filtering for joins (all endpoints, all depths)**: Child entities with `exclude(scoped)` fields are now filtered at the SQL level (`WHERE is_private = false`) during join loading on **both** `get_one_scoped` and `get_all_scoped`, and at **every** depth when `depth > 1`. The scoped batch loader applies each child's `ScopeFilterable::scope_condition()` to its `Entity::find().filter(FK in parent_ids)` query, and recurses via `get_one_scoped` (not `get_one`) for nested children. The in-memory `ScopeFilterable::is_scope_visible()` filter remains as defense-in-depth, but privacy is now enforced in the database, not just at serialisation time: private rows never leave Postgres on public endpoints.
- **`require_scope` attribute**: New `#[crudcrate(require_scope)]` struct-level attribute. When set, read handlers return HTTP 500 if no `ScopeCondition` middleware is present; this catches misconfigured routes that should be scoped but aren't.

### Added

- **Struct-level join definitions**: Join fields can now be defined at the struct level instead of on the SeaORM Model. This keeps the Model lightweight and avoids stack overflow when loading entities with heavy join types. The join field only exists on the generated API struct.
  ```rust
  #[crudcrate(
      api_struct = "Site",
      join(name = "replicates", result = "Vec<SiteReplicate>", one, all, depth = 1)
  )]
  pub struct Model { /* no replicates field here */ }
  ```
  Field-level joins with `#[sea_orm(ignore)]` + `#[crudcrate(non_db_attr, join(...))]` still work for backward compatibility.

- **SQL-level column exclusion for `exclude(list)`**: Fields marked `#[crudcrate(exclude(list))]` with `Option<T>` types are now skipped at the SQL level in list queries; the database never transfers the data. Previously, `exclude(list)` only removed the field from the response struct while still fetching all columns. This dramatically improves performance for entities with large fields (photos, blobs, documents). Benchmarked at **7x improvement** (1,013 → 7,121 req/s) on an endpoint with base64 photo data.

- **`ScopeCondition` for auth-aware query filtering**: New `ScopeCondition` type that can be injected via Axum `Extension` to add conditions to read queries. Auth-system-agnostic: users write middleware to convert their auth state into a `ScopeCondition`. When present, `get_all_handler` merges the condition into the query filter, and `get_one_handler` verifies the fetched record passes the condition. Write operations are unaffected.
  ```rust
  use crudcrate::ScopeCondition;
  let public = Article::read_only_router(&db)
      .layer(Extension(ScopeCondition(
          Condition::all().add(article::Column::IsPrivate.eq(false))
      )));
  ```

- **`read_only_router()` method**: Generates a router with only GET endpoints (get_one + get_all), no create/update/delete. Use with `ScopeCondition` for public/filtered API endpoints.

- **`fk_column` join parameter**: Optional `fk_column = "ColumnName"` in `join(...)` attributes for entities where the FK column doesn't follow the `{StructName}Id` convention. The convention remains the default; this is an escape hatch for non-standard schemas.
  ```rust
  #[crudcrate(join(one, all, depth = 1, fk_column = "OwnerUuid"))]
  pub items: Vec<Item>,
  ```

- **`ScopeFilterable::scope_condition()`**: New trait method that returns a `sea_orm::Condition` matching an entity's `exclude(scoped)` fields. Auto-generated by the derive macro. Enables SQL-level scope filtering for join queries.

- **`get_one_scoped` / `get_all_scoped`**: New `CRUDResource` trait methods with scope-aware query variants. Default implementations delegate to the non-scoped `get_one` / `get_all` (safe for resources without `join(all)` children). The derive macro overrides both with SQL-level child-scope propagation. `get_all_handler` dispatches to `get_all_scoped` whenever a `ScopeCondition` extension is present.

### Fixed

- **Stack overflow with many joins**: All join-loading futures are now `Box::pin`ned, moving large async state off the stack. Prevents stack overflow in debug builds with many join fields.
- **Async state machine bloat in debug builds**: All join-loading futures are wrapped in `Box::pin`, preventing debug-build async state machine bloat from `Related<E>` monomorphization.

### Changed

- **`depth = 0` is now a compile error**: Use `depth = 1` for shallow loading. Previously `depth = 0` could cause infinite recursion at runtime.
- **Compile-time bidirectional relation detection**: Joins targeting an entity that has a `Related<Self>` impl (bidirectional/cyclic relationship) now produce a compile error unless an explicit `depth` is set. Previously, these silently caused infinite recursion at runtime via SeaORM's `Relation::def()` chain. The error message explains the cycle and suggests the fix.
- **Compile-time warnings for risky join depths**: Self-referencing joins without an explicit `depth` and joins with `depth > 5` now emit `#[deprecated]` warnings at compile time, guiding users to set safe depth values.

## [0.7.2] - 2026-03-27

### Added

- **Automatic enum field detection**: Fields with types implementing `sea_orm::ActiveEnum` are now detected at compile time; no `#[crudcrate(enum_field)]` annotation needed. Uses zero-cost compile-time trait detection (inherent impl trick) to check each field's type.
- **Case-insensitive enum array filtering**: Array/IN filters on enum fields now apply `UPPER(CAST(col AS TEXT))` on Postgres, matching the case-insensitive behavior already used for single-value enum filters.

### Deprecated

- **`#[crudcrate(enum_field)]`**: No longer required. Enum fields are auto-detected from the `ActiveEnum` trait implementation. The attribute still works for backward compatibility but can be safely removed.

### Fixed

- **Array/IN filtering on enum fields**: `process_array_filter()` now handles enum fields by casting to TEXT and uppercasing on Postgres. Previously, array filters on enum columns could fail on native Postgres ENUM types or produce case-sensitive results.

## [0.7.1] - 2026-03-09

### Added

- **Transform Hooks**: New `transform` phase in hook system for result modification
  - Hook execution order: pre → body → transform → post
  - Transform hooks receive the result and return a modified version
  - Allows enriching, decorating, or transforming CRUD results before returning
  - Supported for all operations: create, read, update, delete (one and many)
  - Example: `#[crudcrate(read::one::transform = enrich_with_metadata)]`
- **Partial Success for Batch Operations**: New `?partial=true` query parameter for batch endpoints
  - Returns HTTP 207 Multi-Status when some items succeed and some fail
  - Response includes `succeeded` and `failed` arrays with indices and error messages
  - Available for: `POST /batch`, `PATCH /batch`, `DELETE /batch`
  - New types: `BatchResult<T>`, `BatchFailure`, `BatchOptions`
  - **Note**: Partial mode processes items individually using single-item hooks (`create::one::*`, etc.), not batch hooks (`create::many::*`). Each item commits independently with no shared transaction.
- **Batch Create/Update Endpoints**: `POST /batch` and `PATCH /batch` for bulk operations
  - Transaction-based all-or-nothing semantics by default
  - Pre-validation for batch updates ensures true atomicity across all DB backends
- **Runtime-Configurable Limits**: Override batch and pagination limits per-resource
  - `#[crudcrate(batch_limit = 500)]` - Max items for batch create/update/delete (default: 100)
  - `#[crudcrate(max_page_size = 500)]` - Max items per page (default: 1000)
  - Trait methods `fn batch_limit()` and `fn max_page_size()` can be overridden for runtime logic (env vars, config)
- **Security Startup Log**: Info-level log message when mounting CRUD routes
  - Reports resource name, table, batch_limit, max_page_size, and enabled security defaults
  - Silent when no tracing subscriber is configured
- **Batch Loading for Joins (N+1 Query Fix)**: Optimized `get_all()` with joins
  - Reduced from N+1 queries to 2 queries for depth=1 joins (1 for parents + 1 per join field). Deeper joins (depth > 1) may issue additional queries to load nested relations.
  - Uses `WHERE parent_id IN (...)` with in-memory grouping
- **Documentation Test Links**: New mdbook preprocessor linking documentation examples to test files
- **IDE Documentation**: Comprehensive attribute reference in crate-level documentation

### Changed

- **Documentation Overhaul**: Complete restructure of tutorial documentation
  - New progressive tutorial: First Steps → Auto IDs → Timestamps → Filtering → Sorting → Search → Hiding Fields → Relationships → Hooks
  - Simplified navigation structure in SUMMARY.md
  - Enhanced examples with "Run It Now" sections
  - Net reduction of ~800 lines while covering more features
- **DateTimeWithTimeZone schema fix**: All generated model structs (API, Create, Update, List, Response) now resolve `DateTimeWithTimeZone` to `chrono::DateTime<chrono::FixedOffset>` so utoipa's ToSchema derive recognizes it as a DateTime type
- Generated API struct derives now use fully qualified paths (`serde::Serialize`, `utoipa::ToSchema`, etc.) to avoid conflicts with user imports
- Bumped `sea-orm` from 1.1.17 to 1.1.19
- Batch operation limit checking now uses `Self::batch_limit()` method (configurable per-resource)
- `BATCH_LIMIT` and `MAX_PAGE_SIZE` changed from associated constants to trait methods for runtime overridability
- Batch loading uses `.remove()` from HashMap instead of `.get().cloned()`, moving data instead of copying

### Fixed

- UUID array filtering now passes native `Uuid` values to `is_in()` instead of stringified values, fixing incorrect query generation for UUID column arrays
- `max_page_size()` trait method now enforced in HTTP pagination handler
- `delete_many()` returns only actually-deleted IDs
- `update_many()` removed redundant pre-validation queries outside the transaction (TOCTOU race)
- Self-referencing join errors now logged via `tracing::warn!` instead of silently swallowed
- Nested relation loading errors (`get_one()` fallbacks) now logged via `tracing::warn!`
- `to_snake_case` in FK derivation now handles acronyms correctly
- Batch loading uses PK field name from entity metadata instead of hardcoded `id`
- `update()` trait default used plural instead of singular resource name in not-found error
- `delete_many()` trait default had no batch limit check (now enforces `batch_limit()`)
- Broken cross-reference links in reference documentation
- Clippy doc-markdown warnings

### Removed

- **`BatchUpdateItem<T>`**: Dead struct removed from public API
- **Dead code path**: Unreachable self-referencing branch in batch loading
- **Documentation**: Legacy tutorial structure replaced by progressive tutorials

## [0.7.0] - 2025-11-26

### Security

- Harden search queries with proper wildcard escaping
- Improve input sanitization in filtering and pagination
- Add pagination limits to prevent excessive queries

### Added

- **Join Filtering**: Filter by related entity columns using dot-notation syntax
  - `filterable("col1", "col2")` nested inside `join(...)` attribute
  - Query: `?filter={"vehicles.make":"BMW"}`
  - All standard operators supported (`_gt`, `_gte`, `_lt`, `_lte`, `_neq`)
  - Single-level joins only (nested paths like `vehicles.parts.name` not supported)
- **Join Sorting**: Sort by related entity columns using dot-notation syntax
  - `sortable("col1", "col2")` nested inside `join(...)` attribute
  - Query: `?sort=["vehicles.year","DESC"]` or `?sort_by=vehicles.year&order=DESC`
  - Single-level joins only (nested paths not supported)
- **Hook System**: Attribute-based customization with `{operation}::{cardinality}::{phase}` syntax
  - Operations: `create`, `read`, `update`, `delete`
  - Cardinality: `one` (single), `many` (batch)
  - Phases: `pre`, `body`, `post`
  - Example: `#[crudcrate(create::one::pre = validate_fn)]`
- Batch operations: `create_many` and `update_many` with hook support
- **`ApiError` error type**: Consistent error handling with separate internal/client messages (fixes #3)
  - `impl From<DbErr>` for Sea-ORM error conversion with automatic internal logging
  - Internal errors logged via `tracing`, generic message sent to client
  - Custom errors: `ApiError::custom(StatusCode::IM_A_TEAPOT, "client msg", Some("internal log"))`
  - Variants: `NotFound`, `BadRequest`, `Unauthorized`, `Forbidden`, `Conflict`, `ValidationFailed`, `Database`, `Internal`, `Custom`
- Lifecycle hooks in `CRUDOperations` trait
- Improved test coverage across modules

### Changed

- Major codebase refactoring (38% size reduction)
  - Removed `index_analysis` module
  - Simplified `relation_validator.rs`
  - Consolidated join/recursion handling
  - Modular `codegen/` structure
- Handler code generation refactored for hook flow
- Replace `eprintln!` with `tracing` for logging
- Legacy `fn_*` attributes auto-map to new hook syntax

### Fixed

- Improved error handling in join path parsing
- Fixed flaky tests with serial execution
- All clippy::pedantic warnings resolved

### Removed

- **`index_analysis` module**: Database index recommendations moved to external tooling (pgAdmin, MySQL Workbench, etc.)
- **`register_crud_analyser!` macro**: No longer needed without index analysis
- **`attributes.rs`**: Dead code (IDE autocomplete hints only, never used at runtime)
- **`join_strategies/` module**: Consolidated into `codegen/joins/`
- **`field_analyzer.rs`**: Reorganized into `fields/` module
- **Redundant examples**: `minimal_debug.rs`, `minimal_spring.rs`, `test_router_only.rs`
- **Verbose documentation**: ~400 lines of excessive doc comments trimmed

### Dependencies

- Added `serial_test = "3.2"` for test isolation
- Added `tracing` for structured logging

## [0.6.1] - 2025-11-03

### Fixed

- Global path resolution of joined structs
- Restructuring of crudcrate-derive into smaller modules, bit by bit.

## [0.6.0] - 2025-10-31

### Added

- **Recursive Join Loading**: Multi-level relationship loading with `#[crudcrate(join(one, all))]` attribute
- Cyclic dependency detection at compile-time with actionable error messages
- Unlimited join depth support with default depth warnings for relationships > 3 levels
- `exclude()` function-style syntax for model exclusion: `#[crudcrate(exclude(create, update))]`
- The get one response is now its own model, allowing for exclusion of fields from get one/create/update responses
- New `recursive_join` example demonstrating nested relationship loading
- Debug output functionality for procedural macros with `debug_output` attribute

### Changed

- **derive**: Removed requirement for `Eq` and `PartialEq` derives on generated API structs
- **derive**: Improved multi-pass code generation to handle cyclic dependencies

### Fixed

- Database test cleanup logic for PostgreSQL and MySQL backends
- Relationship loading in `get_one()` and `get_all()` endpoints

### Dependencies

- **derive**: Updated with recursive join support, cyclic dependency detection, and enhanced attribute parsing

## [0.5.0] - 2025-08-28

### Added

- Spring-RS framework support with minimal example in `/examples`
- Restored CRUD benchmarks from 0.4.5

### Changed

- Moved `crudcrate-derive` and examples into repository
- Simplified framework architecture - removed redundant code generation paths
- Refactored macro code generation by splitting helpers.rs into focused modules

### Removed

- BREAKING: Case-sensitive enum filtering functionality

## [0.4.5] - 2025-08-25

### Fixed

- Batch delete endpoints now returns the array of successfully deleted resource UUIDs, suitable for a react-admin batch delete response.

## [0.4.4] - 2025-08-20

### Added

- Index analysis system for database optimization recommendations
- `analyse_indexes_for_resource` and `analyse_all_registered_models` functions
- Database-specific index recommendations with priority-based output

### Changed

- **BREAKING** (if still using CRUDResource manually): Added required `TABLE_NAME`
  constant to `CRUDResource` trait. This does not affect `EntityToModel` functionality.
- Made `validate_field_value` function const
- Improved code organization with extracted helper functions

### Fixed

- All clippy warnings (pessimistic and pedantic)
- Test compilation errors and naming inconsistencies
- Documentation examples and missing trait implementations

## [0.4.3] - 2025-08-19

### Added

- **Testing**: Integration tests for `create_model=false` compatibility with `non_db_attr`
- **Testing**: Comprehensive test suite for `use_target_models` functionality with cross-model referencing

### Fixed

- **derive**: Resolved lingering compilation errors from List model update
- **derive**: Fixed test compatibility issues following List model integration
- **Filter system**: Minor improvements to filtering logic consistency

### Dependencies

- **derive**: Updated to latest version with enhanced List model support and improved compatibility

## [0.4.2] - 2025-08-18

### Added

- **List Model Support**: New `List` model generation capability for customizing fields returned in list/getAll endpoints, similar to Create and Update models
- Generated List model behavior with field deselection support
- Built-in `getAll` query optimization to only return fields specified in List model
- **derive**: Support for reserved field names using `r#` syntax (e.g., `r#type`)
- **derive**: Enhanced target model usage with CRUDResource structs for cross-model referencing
- **derive**: Automatic `From<>` trait generation for List structs from Sea-ORM DB models

### Changed

- **derive**: Improved trait compatibility by re-adding `PartialEq`, `Eq`, `Debug`, and `Clone` derives to models for Sea-ORM compatibility
- **derive**: Route generation now uses root-level paths instead of prefixed routes for better user control
- **derive**: Enhanced `use_target_models` functionality for better cross-model integration

### Fixed

- **derive**: Fixed ActiveModel generation when create model excludes keys
- **derive**: Fixed `create_model=false` compatibility with `non_db_attr`
- **derive**: Improved function linking in crudcrate function overrides
- **derive**: Fixed trait signature for Condition in get_all operations
- **derive**: Various clippy warnings resolved

### Dependencies

- **derive**: Updated to 0.2.6 with List model support, reserved field handling, and enhanced model generation capabilities

## [0.4.1] - 2025-08-05

### Added

- Index analysis functionality with `analyze_indexes_for_resource()` and `analyze_and_display_indexes()` methods
- Full-text search support in filtering system with `fulltext_searchable_columns()` method
- REST-standard pagination and query filters alongside React Admin compatibility
- Multi-database testing support (SQLite, PostgreSQL, MySQL) via `DATABASE_URL` environment variable
- Comprehensive benchmark suite with performance testing across database backends
- Security integration tests for SQL injection protection
- Coverage reporting with Codecov integration
- Database feature flags for selective driver compilation (`mysql`, `postgresql`, `sqlite`)
- Binary size optimization through conditional database driver inclusion

### Changed

- Enhanced filtering system with enum case insensitivity and improved edge case handling
- Updated README with minimal examples and comprehensive testing documentation
- Restructured test infrastructure to support multiple database backends
- Improved error handling in filter parsing with better validation
- Removed Clone requirement from generated API structs (Create/Update models)
- Optimized trait methods to use references instead of owned values where possible
- Sea-ORM dependency now uses `default-features = false` with selective feature enabling
- Enhanced README with database feature selection examples

### Fixed

- Enum filtering now supports case-insensitive matching
- Filter edge cases handle malformed JSON gracefully
- PostgreSQL test isolation issues with race conditions during parallel execution
- Clippy warnings resolved across codebase
- **derive**: Improved integration tests and restructured codebase

### Dependencies

- **derive**: Updated to 0.2.1 with full-text search support and enhanced router generation capabilities
- **derive**: Removed Clone derives from generated structs to reduce memory overhead

## [0.4.0] - 2025-07-17

### Added

- **Enhanced Router Generation**: Automatic router generation via `generate_router` attribute in `EntityToModels` macro
- **Non-Database Field Support**: Complete support for non-DB fields using `#[sea_orm(ignore)]` + `#[crudcrate(non_db_attr = true)]` pattern
- **Single-File API Capability**: Full CRUD API can now be implemented in under 60 lines of code
- Documentation improvements for non-DB field usage with examples
- **derive**: EntityToModels macro with complete entity-to-API generation and CRUDResource implementation
- **derive**: Router generation capability integrated into EntityToModels
- **derive**: Enhanced support for non-database fields with proper Sea-ORM integration
- **derive**: Comprehensive integration tests and restructured codebase

### Changed

- Enhanced `EntityToModels` macro to automatically generate router functions
- Improved documentation with comprehensive non-DB field examples
- Router generation now fully automated with zero boilerplate
- **derive**: Enhanced `ToCreateModel` and `ToUpdateModel` with new trait system
- **derive**: Added `MergeIntoActiveModel` trait implementation

### Fixed

- **derive**: Test infrastructure improvements and better error handling in macro generation

## [0.3.3] - 2025-06-23

### Fixed

- Fix newline formatting in auto-generated OpenAPI documentation
- Remove debug messages from production builds

### Changed

- Accept enum exact comparison in filter queries
- Filter on integer columns support

## [0.3.2] - 2025-06-06

### Changed

- Bump dependencies including crudcrate-derive for improved `into()` casting support

### Dependencies

- **derive**: Updated to 0.1.6 with improved `.into()` casting support and enhanced field attribute handling

## [0.3.1] - 2025-05-12

### Changed

- Update lockfile and enhance filtering capabilities for enum and integer columns

## [0.3.0] - 2025-04-05

### Added

- **Major**: Default implementations for `get_one`, `get_all`, and `update_one` in `CRUDResource` trait
- New `MergeIntoActiveModel` trait for improved update model handling
- Enhanced derive macro integration with new trait system

### Changed

- Restructured core trait system for better usability
- Updated derive macro to reference new `MergeIntoActiveModel` trait

### Dependencies

- **derive**: Updated to 0.1.5 with `IntoActiveModel` trait for `UpdateModel` and improved trait derivations

## [0.2.5] - 2025-04-04

### Added

- Export `serde_with` for better serialization support
- Enhanced error responses in API endpoints
- Documentation for query parameters

### Changed

- Renamed `openapi.rs` to `routes.rs` for better organization
- Updated dependencies

## [0.2.4] - 2025-03-11

### Added

- Description string support in CRUDResource
- Auto-populated summary and description for macro-generated endpoints
- Enhanced OpenAPI documentation generation

### Dependencies

- **derive**: Updated to 0.1.4 with improved serialization support using exported `serde_with`

## [0.2.3] - 2025-03-07

### Added

- Comprehensive OpenAPI macro support
- Better API documentation generation

### Fixed

- Improved error responses in endpoints

## [0.2.2] - 2025-03-06

### Added

- Documentation for query parameters

## [0.2.1] - 2025-03-05

### Added

- Description string support in CRUDResource
- Auto-populated summary and description for macro-generated endpoints

## [0.2.0] - 2025-03-05

### Changed

- **Breaking**: Major refactor from route-based to macro-based approach
- Introduced `crud_handlers!` macro for generating CRUD endpoints
- Simplified API creation process significantly

### Removed

- Legacy route-based implementation

## [0.1.4] - 2025-03-03

### Fixed

- Fixed return type of `delete_one` handler
- Applied clippy suggestions for performance improvements

## [0.1.3] - 2025-02-19

### Changed

- Update crudcrate-derive to allow non-db parameters in update/create models

### Dependencies

- **derive**: Updated to 0.1.3 with support for auxiliary attributes in structs that don't relate to DB model

## [0.1.2] - 2025-02-18

### Changed

- Update proc macro to 0.1.2

### Dependencies

- **derive**: Updated to 0.1.2 with improved trait derivations (Clone instead of Copy where appropriate)

## [0.1.0] - 2025-02-18

### Added

- Initial release of crudcrate
- Basic CRUD operation framework
- Sea-ORM and Axum integration
- OpenAPI documentation support
- Move common functions and traits from existing API
- Import proc-macros from crudcrate-derive

### Dependencies

- **derive**: Initial release (0.1.0) with `ToCreateModel` and `ToUpdateModel` derive macros, field-level attribute support for CRUD customization, and integration with Sea-ORM ActiveModel system

[0.10.1]: https://github.com/evanjt/crudcrate/compare/0.10.0...0.10.1
[0.10.0]: https://github.com/evanjt/crudcrate/compare/0.9.3...0.10.0
[0.9.3]: https://github.com/evanjt/crudcrate/compare/0.9.2...0.9.3
[0.9.2]: https://github.com/evanjt/crudcrate/compare/0.9.1...0.9.2
[0.9.1]: https://github.com/evanjt/crudcrate/compare/0.9.0...0.9.1
[0.9.0]: https://github.com/evanjt/crudcrate/compare/0.8.1...0.9.0
[0.8.1]: https://github.com/evanjt/crudcrate/compare/0.8.0...0.8.1
[0.8.0]: https://github.com/evanjt/crudcrate/compare/0.7.2...0.8.0
[0.7.2]: https://github.com/evanjt/crudcrate/compare/0.7.1...0.7.2
[0.7.1]: https://github.com/evanjt/crudcrate/compare/0.7.0...0.7.1
[0.7.0]: https://github.com/evanjt/crudcrate/compare/0.6.1...0.7.0
[0.6.1]: https://github.com/evanjt/crudcrate/compare/0.6.0...0.6.1
[0.6.0]: https://github.com/evanjt/crudcrate/compare/0.5.0...0.6.0
[0.5.0]: https://github.com/evanjt/crudcrate/compare/0.4.5...0.5.0
[0.4.5]: https://github.com/evanjt/crudcrate/compare/0.4.4...0.4.5
[0.4.4]: https://github.com/evanjt/crudcrate/compare/0.4.3...0.4.4
[0.4.3]: https://github.com/evanjt/crudcrate/compare/0.4.2...0.4.3
[0.4.2]: https://github.com/evanjt/crudcrate/compare/0.4.1...0.4.2
[0.4.1]: https://github.com/evanjt/crudcrate/compare/0.4.0...0.4.1
[0.4.0]: https://github.com/evanjt/crudcrate/compare/0.3.3...0.4.0
[0.3.3]: https://github.com/evanjt/crudcrate/compare/0.3.2...0.3.3
[0.3.2]: https://github.com/evanjt/crudcrate/compare/0.3.1...0.3.2
[0.3.1]: https://github.com/evanjt/crudcrate/compare/0.3.0...0.3.1
[0.3.0]: https://github.com/evanjt/crudcrate/compare/0.2.5...0.3.0
[0.2.5]: https://github.com/evanjt/crudcrate/compare/0.2.4...0.2.5
[0.2.4]: https://github.com/evanjt/crudcrate/compare/0.2.3...0.2.4
[0.2.3]: https://github.com/evanjt/crudcrate/compare/0.2.2...0.2.3
[0.2.2]: https://github.com/evanjt/crudcrate/compare/0.2.1...0.2.2
[0.2.1]: https://github.com/evanjt/crudcrate/compare/0.2.0...0.2.1
[0.2.0]: https://github.com/evanjt/crudcrate/compare/0.1.4...0.2.0
[0.1.4]: https://github.com/evanjt/crudcrate/compare/0.1.3...0.1.4
[0.1.3]: https://github.com/evanjt/crudcrate/compare/0.1.2...0.1.3
[0.1.2]: https://github.com/evanjt/crudcrate/compare/0.1.0...0.1.2
[0.1.0]: https://github.com/evanjt/crudcrate/releases/tag/0.1.0
