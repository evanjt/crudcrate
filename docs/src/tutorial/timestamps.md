# Adding Timestamps

We want to know when tasks were created and when they were last modified.

## Add the Fields

```rust
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, Debug, DeriveEntityModel, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    pub title: String,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTime<Utc>,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
    pub updated_at: DateTime<Utc>,
}
```

Add `chrono` to `Cargo.toml`:

```toml
chrono = { version = "0.4", features = ["serde"] }
```

Update your table:

```sql
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)
```

## What's Happening

{{#test_link on_create}} {{#test_link on_update}}

| Field | on_create | on_update | Behavior |
|-------|-----------|-----------|----------|
| `created_at` | `Utc::now()` | - | Set once when created |
| `updated_at` | `Utc::now()` | `Utc::now()` | Set on create, updated on every change |

Both are `exclude(create, update)` so users can't manually set them.

## Try It

```bash
# Create
curl -X POST http://localhost:3000/tasks \
  -H "Content-Type: application/json" \
  -d '{"title": "Learn timestamps"}'
```

Response:

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "Learn timestamps",
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T10:30:00Z"
}
```

```bash
# Update (wait a few seconds first)
curl -X PUT http://localhost:3000/tasks/550e8400-e29b-41d4-a716-446655440000 \
  -H "Content-Type: application/json" \
  -d '{"title": "Master timestamps"}'
```

Response - notice `updated_at` changed:

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "Master timestamps",
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T10:30:45Z"
}
```

---

## Our Task Model Now

```rust
pub struct Model {
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    pub title: String,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTime<Utc>,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
    pub updated_at: DateTime<Utc>,
}
```

Now we have proper task tracking. But as we add more tasks, how do we find specific ones?

**Next:** [Finding tasks](./filtering.md) - filter by field values.
