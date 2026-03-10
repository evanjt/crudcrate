# Time-Series Aggregation

CrudCrate integrates with [TimescaleDB](https://www.timescale.com/) to provide automatic time-series aggregation endpoints via the `aggregate(...)` attribute.

## Prerequisites

1. **TimescaleDB** — a PostgreSQL extension for time-series data
2. **`aggregation` feature flag** — enables the `sea-orm-timescale` dependency

```toml
[dependencies]
crudcrate = { version = "0.7", features = ["aggregation"] }
```

## Basic Setup

Add the `aggregate(...)` attribute to your entity:

```rust
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "readings")]
#[crudcrate(
    api_struct = "ReadingApi",
    aggregate(
        time_column = "time",
        intervals("1h", "1d", "1w"),
        metrics("value"),
        group_by("parameter_id"),
    )
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(filterable)]
    pub parameter_id: Uuid,

    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(filterable, sortable)]
    pub time: DateTime<Utc>,

    #[crudcrate(filterable)]
    pub value: f64,
}
```

This generates a `GET /aggregate` endpoint that returns pivoted time-series data.

### Configuration

| Sub-attribute | Required | Description |
|---|---|---|
| `time_column = "col"` | Yes | The `TIMESTAMPTZ` column for time bucketing |
| `intervals("1h", "1d")` | Yes | Allowed interval values (short form only) |
| `metrics("value", "temp")` | Yes | Numeric columns to compute aggregates over |
| `group_by("site_id")` | No | Additional grouping columns beyond time |
| `aggregates(avg, min, max)` | No | Aggregate functions (default: `avg, min, max`) |

### Compile-Time Validation

CrudCrate validates your aggregate config at compile time:

- **`time_column`** must reference an existing `DateTime` field
- **`metrics`** must reference existing numeric fields (`f32`, `f64`, `i32`, `Decimal`, etc.)
- **`group_by`** columns must reference existing fields

Typos produce clear compile errors pointing to the exact attribute:

```
error: aggregate metric 'valuee' not found. Available fields: parameter_id, time, value
  --> src/models.rs:10:22
   |
10 |         metrics("valuee"),
   |                  ^^^^^^^
```

## Aggregate Functions

Available functions for the `aggregates(...)` attribute:

| Function | SQL | Notes |
|---|---|---|
| `avg` | `AVG(col)` | Arithmetic mean |
| `min` | `MIN(col)` | Minimum value |
| `max` | `MAX(col)` | Maximum value |
| `first` | `first(col, time)` | Earliest value (TimescaleDB-specific) |
| `last` | `last(col, time)` | Latest value (TimescaleDB-specific) |

Default when omitted: `avg, min, max`.

```rust
#[crudcrate(aggregate(
    time_column = "recorded_at",
    intervals("1h", "1d"),
    metrics("value", "temperature"),
    aggregates(avg, min, max, first, last),
))]
```

## Response Format

The aggregate endpoint returns a pivoted response with a shared time axis:

```json
{
  "resolution": "1h",
  "start": "2024-01-01T00:00:00Z",
  "end": "2024-01-02T00:00:00Z",
  "times": ["2024-01-01T00:00:00Z", "2024-01-01T01:00:00Z"],
  "groups": [
    {
      "parameter_id": "uuid-1",
      "metrics": [
        {
          "column": "value",
          "avg": [22.5, 23.1],
          "min": [20.0, 21.0],
          "max": [25.0, 26.0]
        }
      ],
      "count": [60, 60]
    },
    {
      "parameter_id": "uuid-2",
      "metrics": [
        {
          "column": "value",
          "avg": [7.2, null],
          "min": [6.8, null],
          "max": [7.5, null]
        }
      ],
      "count": [60, null]
    }
  ]
}
```

- **`times`**: Shared time axis across all groups (sorted)
- **`groups`**: Per-group data with group-by keys flattened
- **`metrics`**: List of per-metric aggregate arrays, aligned with `times`
- **`null`**: Sparse data is null-filled at missing time positions

## Query Parameters

| Parameter | Type | Description |
|---|---|---|
| `interval` | string | **Required**. Time bucket interval (must match an allowed value) |
| `start` | string | ISO 8601 datetime, inclusive (e.g., `2024-01-01`) |
| `end` | string | ISO 8601 datetime, exclusive |
| `filter` | JSON | Column filters (same format as CRUD filter) |
| `timezone` | string | IANA timezone for timezone-aware bucketing (e.g., `US/Eastern`) |

### Examples

```bash
# Basic hourly aggregation
GET /readings/aggregate?interval=1h

# Time range
GET /readings/aggregate?interval=1d&start=2024-06-01&end=2024-07-01

# With filter
GET /readings/aggregate?interval=1h&filter={"parameter_id":"uuid-1"}

# Timezone-aware daily buckets
GET /readings/aggregate?interval=1d&timezone=US/Eastern
```

## Timezone Support

When the `timezone` parameter is provided, CrudCrate uses TimescaleDB's `time_bucket(interval, column, timezone => 'tz')` for timezone-aware bucketing. Day/week/month buckets will respect DST transitions.

Timezone values are validated against a strict IANA allowlist (`a-zA-Z0-9/_+-` characters only) to prevent SQL injection.

## Continuous Aggregates

[Continuous aggregates](https://docs.timescale.com/use-timescale/latest/continuous-aggregates/) are TimescaleDB's pre-computed materialized views that maintain up-to-date aggregate data automatically.

### Configuration

```rust
#[crudcrate(aggregate(
    time_column = "time",
    intervals("1h", "1d", "1w"),
    metrics("value"),
    group_by("parameter_id"),
    continuous_aggregates(
        view("1h", "readings_hourly"),
        view("1d", "readings_daily"),
    ),
))]
```

Each `view(interval, name)` maps an interval to a materialized view name. When a request matches a configured interval, the query is automatically routed to the pre-computed view instead of scanning the raw hypertable.

### Startup Auto-Creation

Call `ensure_continuous_aggregates()` at application startup:

```rust
let db = Database::connect(&url).await?;

// One-time setup — fast skip if views already exist
ReadingApi::ensure_continuous_aggregates(&db).await?;

// Mount routes
let app = Router::new()
    .nest("/readings", ReadingApi::aggregate_router(&db).into());
```

This method:
1. Queries `timescaledb_information.continuous_aggregates` (fast, read-only)
2. Skips views that already exist
3. Creates missing views with `CREATE MATERIALIZED VIEW IF NOT EXISTS` (belt + suspenders)

### Query Routing

When a request's interval matches a continuous aggregate, it's automatically routed:

```
GET /readings/aggregate?interval=1h  → queries "readings_hourly" view (fast)
GET /readings/aggregate?interval=1d  → queries "readings_daily" view (fast)
GET /readings/aggregate?interval=1w  → falls back to raw hypertable query
```

Non-matching intervals fall back to the standard `time_bucket()` query against the raw hypertable.

## Aggregate-Only Mode

Entities with `aggregate(...)` but no `generate_router` get aggregate endpoints without CRUD:

```rust
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "readings")]
#[crudcrate(
    api_struct = "ReadingApi",
    aggregate(
        time_column = "time",
        intervals("1h", "1d"),
        metrics("value"),
    )
)]
pub struct Model { /* ... */ }
```

This generates:
- A unit struct `ReadingApi`
- `ReadingApi::aggregate_query()` — programmatic access
- `ReadingApi::aggregate_router()` — standalone router with just `GET /aggregate`

No `CRUDResource` impl, no Create/Update/List models.

## Hooks

Aggregate endpoints support `pre` and `transform` hooks:

```rust
#[crudcrate(
    aggregate(time_column = "time", intervals("1h"), metrics("value")),
    aggregate::one::pre = check_aggregate_auth,
    aggregate::one::transform = enrich_aggregate_response,
)]
```

## Security

- **Interval validation**: Only short-form intervals from the allowlist are accepted
- **Time range**: `start`/`end` are parsed to `DateTime<Utc>` and used via parameterized queries
- **Filters**: Applied via SeaORM's parameterized `Expr::col().eq()` — no string interpolation
- **Timezone**: Validated against a strict character allowlist (`a-zA-Z0-9/_+-`)
- **Metric/column names**: Compile-time constants from proc macro attributes — not user-controllable

## See Also

- [sea-orm-timescale](https://crates.io/crates/sea-orm-timescale) — the underlying TimescaleDB library
- [TimescaleDB documentation](https://docs.timescale.com/)
