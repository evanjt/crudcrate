# Migrating from crudcrate 0.10.x to 0.11.0

Array filters are now parsed against the column's SQL type. An element that
cannot be parsed rejects the request with `400 Bad Request` instead of dropping
the clause, so the two filter-expression builders gained an error channel.

## TL;DR

| Scenario | Action |
|---|---|
| You use `#[crudcrate(generate_router)]` or `crud_handlers!` only | Nothing to do. Generated code is updated. |
| Hand-written `resolve_joined_filters` calling `build_filter_expr` or `build_comparison_expr` | Add `?` at the call site (both now return `Result<Option<Expr>, ApiError>`). |
| Clients send array filters over date, timestamp, decimal or float columns | They now work on Postgres. A malformed element returns 400 rather than an unfiltered response. |

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
- Scalar values that do not parse for the column type are still dropped
  silently, matching 0.10.
- Text, JSON and enum columns are unchanged.
