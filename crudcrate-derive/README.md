# crudcrate-derive

**Note:** This is the procedural macro component of crudcrate. You should use the main `crudcrate` crate instead - it re-exports all macros from this crate.

```bash
# Use this in your projects:
cargo add crudcrate

# NOT this:
cargo add crudcrate-derive
```

## Quick Start

See the main [crudcrate README](../README.md) for complete documentation and examples.

## What This Provides

This crate contains the procedural macros that:

1. Generate complete CRUD API models from Sea-ORM entities
2. Create HTTP routers for Axum applications
3. Provide filtering, sorting, and pagination
4. Support relationship loading and database optimizations

All functionality is accessed through the main `crudcrate` crate:

```rust
use crudcrate::EntityToModels;  // From this crate

#[derive(EntityToModels)]      // Macro from this crate
#[crudcrate(generate_router)] // Attribute from this crate
pub struct Model { /* ... */ }
```

See the main repository for full documentation.
