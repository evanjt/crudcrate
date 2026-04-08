# Changelog

All notable changes to the crudcrate project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.0] - 2026-04-07

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

- **SQL-level column exclusion for `exclude(list)`**: Fields marked `#[crudcrate(exclude(list))]` with `Option<T>` types are now skipped at the SQL level in list queries — the database never transfers the data. Previously, `exclude(list)` only removed the field from the response struct while still fetching all columns. This dramatically improves performance for entities with large fields (photos, blobs, documents). Benchmarked at **7x improvement** (1,013 → 7,121 req/s) on an endpoint with base64 photo data.

- **`ScopeCondition` for auth-aware query filtering**: New `ScopeCondition` type that can be injected via Axum `Extension` to add conditions to read queries. Auth-system-agnostic — users write middleware to convert their auth state into a `ScopeCondition`. When present, `get_all_handler` merges the condition into the query filter, and `get_one_handler` verifies the fetched record passes the condition. Write operations are unaffected.
  ```rust
  use crudcrate::ScopeCondition;
  let public = Article::read_only_router(&db)
      .layer(Extension(ScopeCondition(
          Condition::all().add(article::Column::IsPrivate.eq(false))
      )));
  ```

- **`read_only_router()` method**: Generates a router with only GET endpoints (get_one + get_all), no create/update/delete. Use with `ScopeCondition` for public/filtered API endpoints.

### Fixed

- **Stack overflow with many joins**: All join-loading futures are now `Box::pin`ned, moving large async state off the stack. Prevents stack overflow in debug builds with many join fields.
- **Async state machine bloat in debug builds**: All join-loading futures are wrapped in `Box::pin`, preventing debug-build async state machine bloat from `Related<E>` monomorphization.

### Changed

- **`depth = 0` is now a compile error**: Use `depth = 1` for shallow loading. Previously `depth = 0` could cause infinite recursion at runtime.

## [0.7.2] - 2026-03-27

### Added

- **Automatic enum field detection**: Fields with types implementing `sea_orm::ActiveEnum` are now detected at compile time — no `#[crudcrate(enum_field)]` annotation needed. Uses zero-cost compile-time trait detection (inherent impl trick) to check each field's type.
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
- Batch loading uses `.remove()` from HashMap instead of `.get().cloned()` — moves data instead of copying

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
  - `impl From<DbErr>` for seamless Sea-ORM integration with automatic internal logging
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

[Unreleased]: https://github.com/evanjt/crudcrate/compare/0.7.1...HEAD
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
