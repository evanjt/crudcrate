# Searching Text

{{#test_link fulltext}}

Filtering requires exact values. But what if you want to find tasks containing "meeting" anywhere in the title?

## Enable Fulltext Search

Add `fulltext` to searchable fields:

```rust
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable, fulltext)]  // Searchable!
    pub title: String,

    #[crudcrate(fulltext)]  // Also searchable
    pub description: Option<String>,

    #[crudcrate(filterable)]
    pub completed: bool,

    #[crudcrate(filterable, sortable)]
    pub priority: i32,

    #[crudcrate(sortable, exclude(create, update), on_create = chrono::Utc::now())]
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
    description TEXT,
    completed BOOLEAN NOT NULL DEFAULT 0,
    priority INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)
```

## Search with `q`

```bash
# Find tasks with "meeting" in title or description
curl 'http://localhost:3000/tasks?q=meeting'

# Multiple words
curl 'http://localhost:3000/tasks?q=team+meeting'
```

## Combine with Other Features

```bash
# Search incomplete tasks, sorted by priority
curl 'http://localhost:3000/tasks?q=meeting&filter={"completed":false}&sort=["priority","DESC"]'

# Search with pagination
curl 'http://localhost:3000/tasks?q=urgent&range=[0,9]'
```

## How It Works

CRUDCrate searches all `fulltext` fields together. A match in any field returns the result.

| Database | Search Method |
|----------|---------------|
| PostgreSQL | Native fulltext with `to_tsvector` |
| MySQL | `FULLTEXT` index |
| SQLite | `LIKE` pattern matching |

---

## Our Task Model Now

```rust
pub struct Model {
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable, fulltext)]
    pub title: String,

    #[crudcrate(fulltext)]
    pub description: Option<String>,

    #[crudcrate(filterable)]
    pub completed: bool,

    #[crudcrate(filterable, sortable)]
    pub priority: i32,

    #[crudcrate(sortable, exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTime<Utc>,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
    pub updated_at: DateTime<Utc>,
}
```

Now let's say you add user accounts. Users have passwords, but you definitely don't want to return password hashes in API responses.

**Next:** [Hiding data](./hiding-fields.md) - keep sensitive fields out of responses.
