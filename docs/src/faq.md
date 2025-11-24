# Frequently Asked Questions

## General

### What is CRUDCrate?

CRUDCrate is a Rust library that generates complete REST APIs from Sea-ORM entities using derive macros. It eliminates boilerplate code for CRUD operations, filtering, sorting, pagination, and relationships.

### How does it compare to other solutions?

| Feature | CRUDCrate | Manual Axum | Diesel | Other ORMs |
|---------|-----------|-------------|--------|------------|
| Code Generation | ✅ Derive macro | ❌ Manual | ❌ Manual | Varies |
| Filtering | ✅ Built-in | ❌ Manual | ❌ Manual | Varies |
| Pagination | ✅ Built-in | ❌ Manual | ❌ Manual | Varies |
| Relationships | ✅ Automatic | ❌ Manual | ❌ Manual | Varies |
| Type Safety | ✅ Full | ✅ Full | ✅ Full | Varies |

### What security features are included?

CRUDCrate includes:
- SQL injection prevention
- Pagination DoS protection
- Comprehensive error handling
- Proper logging integration

## Installation

### What are the minimum Rust version requirements?

Rust 1.70+ is required for stable proc-macro features.

### Do I need to install Sea-ORM separately?

Yes. CRUDCrate works alongside Sea-ORM:

```toml
[dependencies]
crudcrate = "0.1"
sea-orm = { version = "1.0", features = ["runtime-tokio-rustls", "sqlx-postgres"] }
```

## Usage

### Can I use CRUDCrate with an existing Sea-ORM project?

Yes! Just add `#[derive(EntityToModels)]` and `#[crudcrate(...)]` attributes to your existing entities.

### How do I customize the generated endpoints?

Three ways:

1. **Attributes**: Configure behavior with `#[crudcrate(...)]`
2. **CRUDOperations**: Implement hooks for business logic
3. **Custom Handlers**: Replace entire handlers when needed

### Can I have some entities without routers?

Yes. Only add `generate_router` when you want endpoints:

```rust
// With router
#[crudcrate(generate_router)]

// Without router (just models)
#[crudcrate()]
```

### How do I add authentication?

Use Axum middleware:

```rust
let app = Router::new()
    .merge(protected_router())
    .layer(middleware::from_fn(auth_middleware));
```

See [Security](./advanced/security.md) for details.

### Can I have different auth for different routes?

Yes. Use nested routers:

```rust
let public = Router::new().merge(public_routes());

let protected = Router::new()
    .merge(admin_routes())
    .layer(admin_auth_layer);

let app = Router::new().merge(public).merge(protected);
```

## Filtering & Search

### Which fields can be filtered?

Only fields marked `#[crudcrate(filterable)]`:

```rust
#[crudcrate(filterable)]  // Can filter
pub status: String,

pub secret: String,  // Cannot filter
```

### How does fulltext search work?

Fields marked `#[crudcrate(fulltext)]` are searched with `?q=`:

```bash
GET /items?q=search terms
```

The query uses database-optimized search (GIN for Postgres, FULLTEXT for MySQL).

### Can I combine filters and search?

Yes:

```bash
GET /items?filter={"status":"active"}&q=urgent&sort=["created_at","DESC"]
```

## Relationships

### How do I load related entities?

1. Define Sea-ORM relations
2. Add join field with `#[crudcrate(non_db_attr, join(one))]`

```rust
#[sea_orm(ignore)]
#[crudcrate(non_db_attr, join(one))]
pub comments: Vec<Comment>,
```

### Why are relationships not loading?

Check that:
1. `#[sea_orm(ignore)]` is present
2. `#[crudcrate(non_db_attr)]` is present
3. Sea-ORM `Related` trait is implemented
4. `join(...)` specifies `one` and/or `all`

### How do I prevent circular references?

Use `depth` limit:

```rust
#[crudcrate(non_db_attr, join(one, depth = 2))]
```

## Performance

### Is CRUDCrate slow?

No. Generated code has zero runtime overhead. All generation happens at compile time.

### How do I optimize for large tables?

1. Add database indexes on filtered/sorted fields
2. Use pagination (built-in limits: 1000 items max)
3. Exclude heavy fields from lists: `#[crudcrate(exclude(list))]`
4. Limit join depth

### Does loading relationships cause N+1 queries?

Currently, yes. Relationships are loaded with additional queries. For performance-critical paths, consider custom handlers with batch loading.

## Troubleshooting

### Compilation error: "cannot find derive macro"

Import it:

```rust
use crudcrate::EntityToModels;
```

### Error: "field not found" when filtering

The field must be marked `filterable`:

```rust
#[crudcrate(filterable)]
pub status: String,
```

### Relationships return empty

Ensure:
1. Database has related records
2. Sea-ORM `Related` trait is implemented
3. Join is configured: `join(one)` or `join(one, all)`

### "Too many items" error on bulk delete

Built-in safety limit is 100 items. Split into multiple requests.

### Timestamp fields not auto-updating

Check:
1. `on_create` and `on_update` are set
2. Field is excluded from update model: `exclude(update)`

```rust
#[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
pub updated_at: DateTimeUtc,
```

## Migration

### Can I migrate from manual handlers?

Yes. CRUDCrate is additive. You can:
1. Start with one entity
2. Keep existing handlers for others
3. Gradually migrate

### How do I migrate to a new version?

Check the [Changelog](./changelog.md) for breaking changes. Most updates are backward compatible.

## Contributing

### How can I contribute?

See [Contributing](./contributing.md) for guidelines.

### Where do I report bugs?

Open an issue on [GitHub](https://github.com/evanjt/crudcrate/issues).

### Is there a roadmap?

Planned features:
- GraphQL support
- OpenAPI generation
- More database optimizations
- Cursor-based pagination option
