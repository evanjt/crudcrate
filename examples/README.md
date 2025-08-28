# Crudcrate Examples

This directory contains examples demonstrating various features of crudcrate.

## Available Examples

### Minimal Axum Example

A complete CRUD API implemented in just ~60 lines of code using crudcrate with Axum.

**Run:**

```bash
cargo run --example minimal
```

**URLs:**

- API: http://localhost:3000/todo
- Documentation: http://localhost:3000/docs

### Spring-rs Example

Demonstrates Spring-rs framework code generation capabilities.

**Run:**

```bash
cargo run --example minimal_spring --features=spring-rs
```

**Features:**

- ✅ Shows generated Spring-rs handler annotations
- ✅ Demonstrates framework abstraction

## Usage

From the workspace root:

```bash
# Run a specific example
cargo run --example minimal
cargo run --example minimal_spring --features=spring-rs
```

## Testing APIs

For the Axum example, you can test the generated API:

```bash
# Create a todo
curl -X POST http://localhost:3000/todo \
  -H 'Content-Type: application/json' \
  -d '{"title": "Learn Rust", "completed": false}'

# List todos
curl http://localhost:3000/todo

# Get OpenAPI documentation
open http://localhost:3000/docs
```
