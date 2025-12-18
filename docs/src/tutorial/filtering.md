# Finding Tasks

{{#test_link filtering}}

We have lots of tasks now. Let's add a way to filter them.

## Add a Status Field

First, let's give tasks a `completed` status:

```rust
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    pub title: String,

    #[crudcrate(filterable)]  // <-- Enable filtering
    pub completed: bool,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTime<Utc>,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
    pub updated_at: DateTime<Utc>,
}
```

Update your table:

```sql
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    completed BOOLEAN NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)
```

## Filter by Status

The `filterable` attribute enables filtering on that field:

```bash
# Get incomplete tasks
curl 'http://localhost:3000/tasks?filter={"completed":false}'

# Get completed tasks
curl 'http://localhost:3000/tasks?filter={"completed":true}'
```

## Add More Filterable Fields

Let's add priority:

```rust
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable)]  // Can filter by title
    pub title: String,

    #[crudcrate(filterable)]  // Can filter by completed
    pub completed: bool,

    #[crudcrate(filterable)]  // Can filter by priority
    pub priority: i32,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTime<Utc>,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
    pub updated_at: DateTime<Utc>,
}
```

## Filter Examples

```bash
# Exact match
curl 'http://localhost:3000/tasks?filter={"priority":5}'

# Multiple conditions (AND)
curl 'http://localhost:3000/tasks?filter={"completed":false,"priority":5}'

# Greater than
curl 'http://localhost:3000/tasks?filter={"priority_gt":3}'

# Less than or equal
curl 'http://localhost:3000/tasks?filter={"priority_lte":5}'

# Range
curl 'http://localhost:3000/tasks?filter={"priority_gte":3,"priority_lte":7}'
```

## Available Operators

{{#test_link filtering::comparison}}

| Suffix | Meaning | Example |
|--------|---------|---------|
| (none) | equals | `{"priority":5}` |
| `_gt` | greater than | `{"priority_gt":5}` |
| `_gte` | greater than or equal | `{"priority_gte":5}` |
| `_lt` | less than | `{"priority_lt":5}` |
| `_lte` | less than or equal | `{"priority_lte":5}` |
| `_neq` | not equal | `{"priority_neq":5}` |

## Security Note

Only fields marked `filterable` can be filtered. Trying to filter on other fields is silently ignored:

```bash
# This filter is ignored - created_at is not filterable
curl 'http://localhost:3000/tasks?filter={"created_at":"2024-01-01"}'
```

---

## Our Task Model Now

```rust
pub struct Model {
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable)]
    pub title: String,

    #[crudcrate(filterable)]
    pub completed: bool,

    #[crudcrate(filterable)]
    pub priority: i32,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTime<Utc>,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
    pub updated_at: DateTime<Utc>,
}
```

We can find tasks, but they come back in random order. Let's fix that.

**Next:** [Sorting results](./sorting.md) - order tasks by any field.
