# Contributing to CRUDCrate

Thank you for your interest in contributing to CRUDCrate!

## Getting Started

### Prerequisites

- Rust 1.70+
- Git
- A database for testing (SQLite works for most tests)

### Setup

```bash
# Clone the repository
git clone https://github.com/evanjt/crudcrate.git
cd crudcrate

# Build the project
cargo build

# Run tests
cargo test

# Run clippy
cargo clippy
```

## Development Workflow

### 1. Create an Issue

Before starting work, create or find an issue describing:
- Bug reports: Steps to reproduce, expected vs actual behavior
- Features: Use case, proposed API

### 2. Fork and Branch

```bash
# Fork on GitHub, then:
git clone https://github.com/YOUR-USERNAME/crudcrate.git
cd crudcrate
git checkout -b feature/your-feature-name
```

### 3. Make Changes

- Follow the existing code style
- Add tests for new functionality
- Update documentation as needed
- Keep commits focused and atomic

### 4. Test

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with logging
RUST_LOG=debug cargo test

# Check for warnings
cargo clippy

# Format code
cargo fmt
```

### 5. Submit Pull Request

- Push to your fork
- Open PR against `main`
- Fill out the PR template
- Wait for review

## Code Style

### Rust Guidelines

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` for formatting
- Fix all `cargo clippy` warnings
- Document public APIs with doc comments

### Commit Messages

```
type(scope): short description

Longer explanation if needed.

Fixes #123
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

### Example

```
feat(filtering): add _in operator for multiple values

Adds support for filtering with multiple values using
the _in suffix: ?status_in=active,pending

Fixes #42
```

## Project Structure

```
crudcrate/
├── crudcrate/              # Runtime library
│   ├── src/
│   │   ├── lib.rs          # Public API
│   │   ├── core/           # Core traits
│   │   ├── filtering/      # Query parsing
│   │   └── ...
│   └── Cargo.toml
│
├── crudcrate-derive/       # Proc macro crate
│   ├── src/
│   │   ├── lib.rs          # Macro entry points
│   │   ├── codegen/        # Code generation
│   │   └── ...
│   └── Cargo.toml
│
├── examples/               # Example projects
│
└── docs/                   # Documentation (this site)
```

## Testing

### Unit Tests

```rust
#[test]
fn test_filter_parsing() {
    let result = parse_filter(r#"{"status":"active"}"#);
    assert!(result.is_ok());
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_crud_operations() {
    let db = setup_test_db().await;
    // Test full CRUD flow
}
```

### Documentation Tests

````rust
/// Filters items by status.
///
/// # Example
///
/// ```
/// let condition = apply_filters::<Entity>(&params)?;
/// ```
pub fn apply_filters<E>(...) { }
````

## Documentation

### Code Documentation

- All public items need doc comments
- Include examples in doc comments
- Use `///` for items, `//!` for modules

### User Documentation

Documentation is in `docs/src/`. To build locally:

```bash
# Install mdbook
cargo install mdbook

# Build and serve
cd docs
mdbook serve
```

## Review Process

1. **Automated checks**: CI runs tests, clippy, format
2. **Code review**: At least one maintainer review
3. **Documentation**: Ensure docs are updated
4. **Changelog**: Add entry for user-facing changes

## Release Process

(For maintainers)

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Create git tag: `git tag v0.X.Y`
4. Push tag: `git push origin v0.X.Y`
5. CI publishes to crates.io

## Getting Help

- **Questions**: Open a Discussion on GitHub
- **Bugs**: Open an Issue
- **Chat**: Join our Discord (link TBD)

## Code of Conduct

We follow the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct).

Be respectful, constructive, and welcoming to all contributors.

## License

By contributing, you agree that your contributions will be licensed under the same license as the project (MIT/Apache-2.0).
