<p align="center">
  <img src="assets/logo.svg" alt="crudcrate" width="80">
</p>

# crudcrate

[![Tests](https://github.com/evanjt/crudcrate/actions/workflows/test.yml/badge.svg)](https://github.com/evanjt/crudcrate/actions/workflows/test.yml)
[![codecov](https://codecov.io/gh/evanjt/crudcrate/branch/main/graph/badge.svg)](https://codecov.io/gh/evanjt/crudcrate)
[![Crates.io](https://img.shields.io/crates/v/crudcrate.svg)](https://crates.io/crates/crudcrate)
[![Documentation](https://docs.rs/crudcrate/badge.svg)](https://docs.rs/crudcrate)

Every Sea-ORM entity needs the same handful of handler functions, request structs,
response structs, filter parsing, and pagination logic. `crudcrate` derives all of
it from your model definition — on [Axum](https://github.com/tokio-rs/axum).

```rust
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "customers")]
#[crudcrate(generate_router)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable, fulltext)]
    pub name: String,

    #[crudcrate(filterable)]
    pub email: String,

    #[crudcrate(exclude(create, update), on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,
}

// Mount it:
let app = Router::new().nest("/customers", Customer::router(&db));
```

> **Security:** the generated router is unauthenticated **and has no default
> request body size limit**. Before shipping, you MUST (1) add an Axum
> authentication middleware and (2) set `DefaultBodyLimit` on the router —
> without the body-size cap, a single `POST /batch` request can exhaust server
> memory. See [`docs/src/advanced/security.md`](docs/src/advanced/security.md)
> and the [`scoped_access`](examples/scoped_access/main.rs) example.

This generates `Customer`, `CustomerCreate`, `CustomerUpdate`, and `CustomerList`
structs, a full `CRUDResource` implementation, and an Axum router with:

| Endpoint | Description |
|---|---|
| `GET /customers` | List with filtering, sorting, fulltext search, pagination |
| `GET /customers/{id}` | Single resource with relationship loading |
| `POST /customers` | Create |
| `PUT /customers/{id}` | Partial update |
| `DELETE /customers/{id}` | Delete |
| `POST /customers/batch` | Batch create |
| `PATCH /customers/batch` | Batch update |
| `DELETE /customers/batch` | Batch delete (with optional `?partial=true`) |

## What gets generated

From the entity above, `EntityToModels` produces:

```rust
struct Customer       { id: Uuid, name: String, email: String, updated_at: DateTime<Utc> }
struct CustomerCreate { name: String, email: String }
struct CustomerUpdate { name: Option<Option<String>>, email: Option<Option<String>> }
struct CustomerList   { id: Uuid, name: String, email: String, updated_at: DateTime<Utc> }

// Plus: CRUDResource impl, Axum router, OpenAPI schemas
```

`Option<Option<T>>` in update structs distinguishes "not provided" (`None`) from
"set to null" (`Some(None)`).

## See it work

List with pagination (range header):

```bash
$ curl -s localhost:3000/customers -H 'Range: items=0-1' | jq
```
```json
[
  { "id": "d4f2...", "name": "Alice", "email": "alice@example.com", "updated_at": "2026-03-12T10:00:00Z" },
  { "id": "8ba1...", "name": "Bob", "email": "bob@example.com", "updated_at": "2026-03-12T09:30:00Z" }
]
```

Create:

```bash
$ curl -s localhost:3000/customers -X POST \
    -H 'Content-Type: application/json' \
    -d '{"name": "Charlie", "email": "charlie@example.com"}' | jq
```
```json
{ "id": "f1a3...", "name": "Charlie", "email": "charlie@example.com", "updated_at": "2026-03-12T12:00:00Z" }
```

Filter and search:

```bash
$ curl -s 'localhost:3000/customers?filter={"name_like":"Ali"}&sort=["name","ASC"]' | jq
```
```json
[
  { "id": "d4f2...", "name": "Alice", "email": "alice@example.com", "updated_at": "2026-03-12T10:00:00Z" }
]
```

## How it compares

For a single entity with filtering, sorting, pagination, and batch operations:

| | Manual | crudcrate |
|---|---|---|
| Entity definition | ~25 lines | ~25 lines |
| Request/response structs | ~60 lines | generated |
| Handler functions | ~120 lines | generated |
| Filter/sort/pagination | ~100 lines (shared) | built-in |
| Router wiring | ~10 lines | generated |
| **Total per entity** | **~200+ lines** | **~25 lines** |

Based on comparison with production APIs using Sea-ORM + Axum.

## Install

```bash
cargo add crudcrate
```

By default this enables SQLite + derive macros. For PostgreSQL or MySQL:

```toml
crudcrate = { version = "0.7", default-features = false, features = ["postgresql", "derive"] }
```

## Documentation

**[crudcrate.evanjt.com](https://crudcrate.evanjt.com)** — Tutorials, walkthroughs, and reference.

**[docs.rs/crudcrate](https://docs.rs/crudcrate)** — API reference and attribute documentation.

The tutorials walk through everything from a minimal setup to hook-based
customization, relationship loading, and production deployment.

## Highlights

**Hooks** — Inject logic at any stage of any operation. Pre-validate, override
the body, transform results, or run side effects after completion.

```rust
#[crudcrate(
    generate_router,
    create::one::pre = validate_input,
    read::one::transform = enrich_with_metadata,
    delete::one::post = cleanup_s3_assets,
)]
```

**Relationships** — Load nested data automatically. Batch-loaded at depth 1
(2 queries instead of N+1), recursive up to depth 5.

```rust
#[sea_orm(ignore)]
#[crudcrate(non_db_attr, join(one, all, depth = 2))]
pub vehicles: Vec<Vehicle>,
```

**Filtering & Search** — Rich query API generated from field attributes.

```
GET /customers?filter={"name_like":"John","email_neq":"spam@example.com"}
GET /customers?q=urgent&sort=["name","ASC"]&range=[0,24]
```

**Field control** — Decide exactly what appears in each generated model.

```rust
#[crudcrate(exclude(create, update), on_create = Utc::now())]  // auto-managed timestamp
#[crudcrate(exclude(list))]                                      // heavy field, detail view only
#[crudcrate(exclude(one, list))]                                 // internal, never exposed
```

## Examples

```bash
cargo run --example minimal            # Todo API in ~60 lines
cargo run --example recursive_join     # Multi-level relationship loading
```

## License

MIT
