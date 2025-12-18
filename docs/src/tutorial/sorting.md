# Sorting Results

{{#test_link sorting}}

Tasks come back in database order. Let's control the order.

## Enable Sorting

Add `sortable` to fields you want to sort by:

```rust
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable)]  // Can filter AND sort
    pub title: String,

    #[crudcrate(filterable)]
    pub completed: bool,

    #[crudcrate(filterable, sortable)]  // Can filter AND sort
    pub priority: i32,

    #[crudcrate(sortable, exclude(create, update), on_create = chrono::Utc::now())]  // Sortable!
    pub created_at: DateTime<Utc>,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
    pub updated_at: DateTime<Utc>,
}
```

## Sort Syntax

```bash
# Sort by priority, highest first
curl 'http://localhost:3000/tasks?sort=["priority","DESC"]'

# Sort by creation date, newest first
curl 'http://localhost:3000/tasks?sort=["created_at","DESC"]'

# Sort by title alphabetically
curl 'http://localhost:3000/tasks?sort=["title","ASC"]'
```

## Combine with Filtering

```bash
# Incomplete tasks, highest priority first
curl 'http://localhost:3000/tasks?filter={"completed":false}&sort=["priority","DESC"]'

# High priority tasks, newest first
curl 'http://localhost:3000/tasks?filter={"priority_gte":8}&sort=["created_at","DESC"]'
```

---

# Pagination

{{#test_link pagination}}

What if you have 10,000 tasks? You don't want to load them all at once.

## The Range Parameter

{{#test_link pagination::range}}

```bash
# First 10 tasks (items 0-9)
curl 'http://localhost:3000/tasks?range=[0,9]'

# Next 10 tasks (items 10-19)
curl 'http://localhost:3000/tasks?range=[10,19]'

# Tasks 50-74 (25 tasks)
curl 'http://localhost:3000/tasks?range=[50,74]'
```

## Response Headers

CRUDCrate tells you about pagination in the response headers:

```
Content-Range: tasks 0-9/150
```

This means: "Returning tasks 0-9 out of 150 total."

## Combine Everything

```bash
# Incomplete tasks, highest priority first, first page
curl 'http://localhost:3000/tasks?filter={"completed":false}&sort=["priority","DESC"]&range=[0,19]'
```

## Safety Limits

CRUDCrate caps results at 1000 items per request. This prevents accidentally loading your entire database.

---

## Our Task Model Now

```rust
pub struct Model {
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub title: String,

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

Filtering is precise - you need to know the exact value. But what if you want to find tasks containing "meeting" somewhere in the title?

**Next:** [Searching text](./search.md) - find tasks by keywords.
