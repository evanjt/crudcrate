# Changelog

All notable changes to the crudcrate project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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