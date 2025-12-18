# Relationships

{{#test_link relationships}}

Tasks should belong to users. Let's connect them.

## Add User ID to Tasks

```rust
// task.rs
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
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

    #[crudcrate(filterable)]  // Filter tasks by user
    pub user_id: Uuid,

    #[crudcrate(sortable, exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTime<Utc>,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
    pub updated_at: DateTime<Utc>,
}
```

Now you can filter tasks by user:

```bash
curl 'http://localhost:3000/tasks?filter={"user_id":"550e8400-e29b-41d4-a716-446655440000"}'
```

But what if you want to see the user details along with the task?

## Loading Related Data

Add a join field to include the user in task responses:

```rust
// task.rs
pub struct Model {
    // ... existing fields ...

    #[crudcrate(filterable)]
    pub user_id: Uuid,

    // Load the user with the task
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all))]
    pub user: Option<super::user::User>,

    // ... timestamps ...
}
```

## Define the Relation

Sea-ORM needs to know how entities relate:

```rust
// task.rs
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id"
    )]
    User,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}
```

## Response Now Includes User

```bash
curl http://localhost:3000/tasks/550e8400-e29b-41d4-a716-446655440000
```

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "Learn relationships",
  "completed": false,
  "priority": 5,
  "user_id": "660f9511-f30c-42e5-b827-557766551111",
  "user": {
    "id": "660f9511-f30c-42e5-b827-557766551111",
    "email": "alice@example.com",
    "name": "Alice",
    "created_at": "2024-01-15T10:00:00Z"
  },
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T10:30:00Z"
}
```

## Join Options

| Attribute | When Data is Loaded |
|-----------|-------------------|
| `join(one)` | Only in `GET /:id` (detail view) |
| `join(all)` | Only in `GET /` (list view) |
| `join(one, all)` | Both endpoints |

For performance, use `join(one)` when the related data is only needed in detail views:

```rust
// Only load user in detail view, not in lists
#[crudcrate(non_db_attr, join(one))]
pub user: Option<User>,
```

## The Other Direction

Users can have tasks too. Add to user.rs:

```rust
// user.rs
pub struct Model {
    // ... existing fields ...

    // User's tasks (only in detail view)
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one))]
    pub tasks: Vec<super::task::Task>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::task::Entity")]
    Tasks,
}

impl Related<super::task::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tasks.def()
    }
}
```

```bash
curl http://localhost:3000/users/660f9511-f30c-42e5-b827-557766551111
```

```json
{
  "id": "660f9511-f30c-42e5-b827-557766551111",
  "email": "alice@example.com",
  "name": "Alice",
  "tasks": [
    {"id": "...", "title": "Learn relationships", "completed": false},
    {"id": "...", "title": "Build something cool", "completed": false}
  ],
  "created_at": "2024-01-15T10:00:00Z"
}
```

## Depth Limit

{{#test_link relationships::depth}}

To prevent infinite recursion (user → tasks → user → tasks → ...), use `depth`:

```rust
#[crudcrate(non_db_attr, join(one, depth = 1))]
pub tasks: Vec<Task>,
```

Default max depth is 5. Self-referencing relations (like categories with subcategories) are automatically limited to depth 1.

{{#test_link relationships::recursive}}

---

## Try the Recursive Join Example

See relationships in action with a complete example:

```bash
cd crudcrate/crudcrate
cargo run --example recursive_join
```

This demonstrates Customer → Vehicle → Parts relationships with automatic loading.

- **API**: http://localhost:3000/customers
- **Docs**: http://localhost:3000/docs

---

## Summary

You now have a complete task manager with:
- Auto-generated UUIDs
- Timestamps
- Filtering, sorting, pagination
- Full-text search
- Hidden sensitive fields
- Related data loading

But what if you need custom logic? Like validating input or sending notifications?

**Next:** [Custom logic](./hooks.md) - add validation and side effects.
