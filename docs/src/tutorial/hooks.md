# Custom Logic with Hooks

{{#test_link hooks}}

Sometimes you need custom logic: validate input, send notifications, or log events. Hooks let you run code before or after CRUD operations.

## The Hook System

Hooks use this syntax: `{operation}::{cardinality}::{phase}`

| Part | Options | Meaning |
|------|---------|---------|
| operation | `create`, `update`, `delete`, `read` | Which action |
| cardinality | `one`, `many` | Single item or batch |
| phase | `pre`, `post`, `body` | When to run |

## Example: Validate Task Title

Let's require task titles to be at least 3 characters:

```rust
use crudcrate::errors::ApiError;

#[derive(Clone, Debug, DeriveEntityModel, EntityToModels)]
#[crudcrate(
    generate_router,
    create::one::pre = validate_task  // Run before create
)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    // ... fields ...
}

async fn validate_task(
    _db: &DatabaseConnection,
    data: &mut TaskCreate,
) -> Result<(), ApiError> {
    if data.title.len() < 3 {
        return Err(ApiError::BadRequest("Title must be at least 3 characters".into()));
    }
    Ok(())
}
```

Now:

```bash
# This fails
curl -X POST http://localhost:3000/tasks \
  -d '{"title": "Hi"}'
# Error: "Title must be at least 3 characters"

# This works
curl -X POST http://localhost:3000/tasks \
  -d '{"title": "Hello"}'
```

## Hook Phases

### `pre` - Before the Operation

Validate or modify input. Return `Err` to cancel the operation.

```rust
#[crudcrate(create::one::pre = validate_task)]

async fn validate_task(
    db: &DatabaseConnection,
    data: &mut TaskCreate,  // Can modify!
) -> Result<(), ApiError> {
    // Validate
    if data.title.is_empty() {
        return Err(ApiError::BadRequest("Title required".into()));
    }

    // Or modify
    data.title = data.title.trim().to_string();

    Ok(())
}
```

### `post` - After the Operation

Run side effects like notifications or logging. The operation already succeeded.

```rust
#[crudcrate(create::one::post = notify_created)]

async fn notify_created(
    _db: &DatabaseConnection,
    task: &Task,  // The created task
) -> Result<(), ApiError> {
    println!("New task created: {}", task.title);
    // Send email, update analytics, etc.
    Ok(())
}
```

### `body` - Replace the Operation

Completely replace the default behavior. Use for soft deletes or custom logic.

```rust
#[crudcrate(delete::one::body = soft_delete)]

async fn soft_delete(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError> {
    // Instead of deleting, set deleted_at
    let mut task: ActiveModel = Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(ApiError::NotFound)?
        .into();

    task.deleted_at = Set(Some(chrono::Utc::now()));
    task.update(db).await?;

    Ok(())
}
```

## Multiple Hooks

Combine hooks for complete workflows:

```rust
#[derive(Clone, Debug, DeriveEntityModel, EntityToModels)]
#[crudcrate(
    generate_router,
    create::one::pre = validate_task,
    create::one::post = log_created,
    update::one::pre = validate_task,
    update::one::post = log_updated,
    delete::one::pre = check_can_delete,
)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    // ...
}
```

## Hook Function Signatures

### Create

{{#test_link hooks::create}}

```rust
// Pre: can modify input
async fn create_pre(db: &DatabaseConnection, data: &mut TaskCreate) -> Result<(), ApiError>;

// Post: receives created item
async fn create_post(db: &DatabaseConnection, task: &Task) -> Result<(), ApiError>;

// Body: replace create logic
async fn create_body(db: &DatabaseConnection, data: TaskCreate) -> Result<Task, ApiError>;
```

### Update

{{#test_link hooks::update}}

```rust
// Pre: receives id and can modify input
async fn update_pre(db: &DatabaseConnection, id: Uuid, data: &mut TaskUpdate) -> Result<(), ApiError>;

// Post: receives updated item
async fn update_post(db: &DatabaseConnection, task: &Task) -> Result<(), ApiError>;
```

### Delete

{{#test_link hooks::delete}}

```rust
// Pre: can prevent deletion
async fn delete_pre(db: &DatabaseConnection, id: Uuid) -> Result<(), ApiError>;

// Post: runs after deletion
async fn delete_post(db: &DatabaseConnection, id: Uuid) -> Result<(), ApiError>;

// Body: replace delete logic
async fn delete_body(db: &DatabaseConnection, id: Uuid) -> Result<(), ApiError>;
```

## Execution Order

1. `pre` hook runs
2. Default operation (or `body` if specified)
3. `post` hook runs

If `pre` returns an error, nothing else runs.

---

## Complete Example

```rust
use chrono::{DateTime, Utc};
use crudcrate::{EntityToModels, errors::ApiError};
use sea_orm::{entity::prelude::*, DatabaseConnection};
use uuid::Uuid;

#[derive(Clone, Debug, DeriveEntityModel, EntityToModels)]
#[crudcrate(
    generate_router,
    create::one::pre = validate_task,
    create::one::post = log_create,
    update::one::pre = validate_task,
    delete::one::pre = check_delete_permission,
)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable, fulltext)]
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

async fn validate_task(
    _db: &DatabaseConnection,
    data: &mut TaskCreate,
) -> Result<(), ApiError> {
    if data.title.trim().is_empty() {
        return Err(ApiError::BadRequest("Title cannot be empty".into()));
    }
    if data.title.len() > 200 {
        return Err(ApiError::BadRequest("Title too long (max 200)".into()));
    }
    // Normalize the title
    data.title = data.title.trim().to_string();
    Ok(())
}

async fn log_create(
    _db: &DatabaseConnection,
    task: &Task,
) -> Result<(), ApiError> {
    tracing::info!("Task created: {} ({})", task.title, task.id);
    Ok(())
}

async fn check_delete_permission(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError> {
    let task = Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(ApiError::NotFound)?;

    if task.completed {
        return Err(ApiError::BadRequest("Cannot delete completed tasks".into()));
    }
    Ok(())
}
```

---

## You Did It!

You've built a complete task manager with:

- Auto-generated UUIDs
- Auto-managed timestamps
- Filtering, sorting, pagination
- Full-text search
- Hidden sensitive fields
- Related data loading
- Custom validation and logic

**What's next?**

- See complete [Examples](../examples/minimal.md)
- Learn about [Security](../advanced/security.md)
- Check the [Reference](../reference/field-attributes.md) for all options
