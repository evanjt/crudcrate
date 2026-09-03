# Migrating from crudcrate 0.10.x to 0.11.0

Array filters are now parsed against the column's SQL type. An element that
cannot be parsed rejects the request with `400 Bad Request` instead of dropping
the clause, so the two filter-expression builders gained an error channel. The
unused `spring-rs` feature is removed.

## TL;DR

| Scenario | Action |
|---|---|
| You use `#[crudcrate(generate_router)]` or `crud_handlers!` only | Nothing to do. Generated code is updated. |
| Hand-written `resolve_joined_filters` calling `build_filter_expr` or `build_comparison_expr` | Add `?` at the call site (both now return `Result<Option<Expr>, ApiError>`). |
| `features = ["spring-rs"]` in your `Cargo.toml` | Remove it. Nothing was gated on it; mount the generated axum router in spring-web directly. |
| Clients send array filters over date, timestamp, decimal or float columns | They now work on Postgres. A malformed element returns 400 rather than an unfiltered response. |
| Clients send an empty array filter (`{"id":[]}`) | It now matches no rows. It used to drop the clause and return every row. |
| Code relies on the `use` items `crud_handlers!` leaks (`StatusCode`, `Json`, `Path`, ...) | Still compiles, now deprecated. Import them directly before the next breaking release. |

## The signature change

```rust
// 0.10
let expr: Option<Expr> = crudcrate::build_comparison_expr(column, operator, value);

// 0.11
let expr: Option<Expr> = crudcrate::build_comparison_expr(column, operator, value)?;
```

`resolve_joined_filters` already returns `Result<Condition, ApiError>`, so `?`
propagates the 400 to the client without further changes.

`Ok(None)` still means "this filter does not apply to this column, skip it".
`Err` is new and means "the client sent a value this column cannot hold".

## Behaviour changes

- An `IN` list over a `TIMESTAMPTZ`, `TIMESTAMP`, `DATE`, `TIME`, `DECIMAL`,
  `MONEY`, `FLOAT` or `DOUBLE` column binds typed values. On Postgres these
  requests previously failed with `operator does not exist: ... = text`.
- `{"year":[2020.5]}` on an integer column is now a 400. It could never match a
  row.
- `{"id":[]}` matches nothing. An empty `IN` list is rendered as a clause that
  no row satisfies, where 0.10 dropped the clause and returned every row.
- A number against a date, time or uuid column (`{"created_at_gte":5}`) is
  parsed as that column's type and dropped when it does not fit. 0.10 bound the
  number as-is and the database rejected the whole request.
- Scalar strings that do not parse for the column type are still dropped
  silently, matching 0.10.
- Text, JSON and enum columns are unchanged.

## Removed

- The `spring-rs` feature and its optional `spring` and `spring-web`
  dependencies.
- The hidden, empty `crudcrate::routes` module.

## New, no action required

- `#[crudcrate(max_child_rows = N)]` and
  `SecurityProfile::max_child_rows_per_relation` cap the child rows one join
  field may load per request. Exceeding the cap returns `413 Payload Too Large`.
  Unlimited unless set.
- `join(relation = "Variant")` selects the child's `Relation` variant for
  foreign key resolution.
- `CRUDResource::pk_value(&model)` is a new provided method that the derive
  overrides with the entity's primary key field. Joins at depth 2 or more no
  longer require the child's key to be named `id`.
- Generated routers reference `crudcrate::tracing`, so a crate without its own
  `tracing` dependency compiles.
