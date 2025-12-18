# Auto-Generating IDs

In the last chapter, users had to specify their own IDs. That's not ideal - what if two users pick the same ID?

Let's make IDs generate automatically using UUIDs.

## The Problem

```bash
# User has to pick an ID - what if 1 is already taken?
curl -X POST http://localhost:3000/tasks \
  -d '{"id": 1, "title": "My task"}'
```

## The Solution

Two changes:

1. **Exclude** the ID from create requests
2. **Auto-generate** it with `on_create`

```rust
use uuid::Uuid;

#[derive(Clone, Debug, DeriveEntityModel, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(
        primary_key,
        exclude(create, update),      // Users can't set or change ID
        on_create = Uuid::new_v4()    // Generate UUID automatically
    )]
    pub id: Uuid,

    pub title: String,
}
```

Add `uuid` to your `Cargo.toml`:

```toml
uuid = { version = "1.0", features = ["v4", "serde"] }
```

Update your table:

```sql
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL
)
```

## Now It Works

```bash
# No ID needed!
curl -X POST http://localhost:3000/tasks \
  -H "Content-Type: application/json" \
  -d '{"title": "Buy groceries"}'
```

Response - ID is generated:

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "Buy groceries"
}
```

## What `exclude` Does

| Attribute | Effect |
|-----------|--------|
| `exclude(create)` | Field not in POST request body |
| `exclude(update)` | Field not in PUT request body |
| `exclude(create, update)` | Field managed by the system, not users |

## What `on_create` Does

{{#test_link on_create}}

The expression is evaluated when inserting a new record:

```rust
on_create = Uuid::new_v4()      // Generate UUID
on_create = chrono::Utc::now()  // Current timestamp
on_create = 0                   // Default number
on_create = false               // Default boolean
```

---

## Our Task Model Now

```rust
pub struct Model {
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    pub title: String,
}
```

But when was this task created? When was it last updated? We have no idea.

**Next:** [Let's add timestamps](./timestamps.md) to track when things happen.
