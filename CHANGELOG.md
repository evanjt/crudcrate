# Changelog

All notable changes to the crudcrate project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
