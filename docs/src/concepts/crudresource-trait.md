# The CRUDResource Trait

`CRUDResource` is the core trait that powers all CRUD operations. CRUDCrate generates its implementation, but understanding it helps you customize behavior.

## Trait Definition

```rust
#[async_trait]
pub trait CRUDResource: Sized + Send + Sync {
    /// The Sea-ORM entity type
    type EntityType: EntityTrait;

    /// Model for creating new records
    type CreateModel: DeserializeOwned + Send + Sync;

    /// Model for updating existing records
    type UpdateModel: DeserializeOwned + Send + Sync;

    /// Model for list responses (can differ from Self)
    type ListModel: Serialize + Send + Sync;

    /// The primary key type
    type PrimaryKey: Send + Sync;

    /// Get a single record by ID
    async fn get_one(
        db: &DatabaseConnection,
        id: Self::PrimaryKey,
    ) -> Result<Self, ApiError>;

    /// Get all records with filtering, sorting, and pagination
    async fn get_all(
        db: &DatabaseConnection,
        condition: Condition,
        order: (Self::EntityType::Column, Order),
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Self::ListModel>, ApiError>;

    /// Create a new record
    async fn create(
        db: &DatabaseConnection,
        data: Self::CreateModel,
    ) -> Result<Self, ApiError>;

    /// Update an existing record
    async fn update(
        db: &DatabaseConnection,
        id: Self::PrimaryKey,
        data: Self::UpdateModel,
    ) -> Result<Self, ApiError>;

    /// Delete a record
    async fn delete(
        db: &DatabaseConnection,
        id: Self::PrimaryKey,
    ) -> Result<(), ApiError>;

    /// Delete multiple records
    async fn delete_many(
        db: &DatabaseConnection,
        ids: Vec<Self::PrimaryKey>,
    ) -> Result<u64, ApiError>;

    /// Get total count matching condition
    async fn total_count(
        db: &DatabaseConnection,
        condition: &Condition,
    ) -> u64;
}
```

## Generated Implementation

For this entity:

```rust
#[derive(EntityToModels)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    #[crudcrate(primary_key)]
    pub id: i32,
    pub title: String,
}
```

CRUDCrate generates:

```rust
#[async_trait]
impl CRUDResource for Task {
    type EntityType = Entity;
    type CreateModel = TaskCreate;
    type UpdateModel = TaskUpdate;
    type ListModel = TaskList;
    type PrimaryKey = i32;

    async fn get_one(
        db: &DatabaseConnection,
        id: i32,
    ) -> Result<Self, ApiError> {
        let model = Entity::find_by_id(id)
            .one(db)
            .await
            .map_err(ApiError::from)?
            .ok_or(ApiError::NotFound)?;

        Ok(model.into())
    }

    async fn get_all(
        db: &DatabaseConnection,
        condition: Condition,
        order: (Column, Order),
        offset: u64,
        limit: u64,
    ) -> Result<Vec<TaskList>, ApiError> {
        let models = Entity::find()
            .filter(condition)
            .order_by(order.0, order.1)
            .offset(offset)
            .limit(limit)
            .all(db)
            .await
            .map_err(ApiError::from)?;

        Ok(models.into_iter().map(|m| m.into()).collect())
    }

    async fn create(
        db: &DatabaseConnection,
        data: TaskCreate,
    ) -> Result<Self, ApiError> {
        let active_model: ActiveModel = data.into();
        let model = active_model.insert(db).await.map_err(ApiError::from)?;
        Ok(model.into())
    }

    async fn update(
        db: &DatabaseConnection,
        id: i32,
        data: TaskUpdate,
    ) -> Result<Self, ApiError> {
        let existing = Entity::find_by_id(id)
            .one(db)
            .await
            .map_err(ApiError::from)?
            .ok_or(ApiError::NotFound)?;

        let mut active_model: ActiveModel = existing.into();
        data.merge_into(&mut active_model);

        let model = active_model.update(db).await.map_err(ApiError::from)?;
        Ok(model.into())
    }

    async fn delete(
        db: &DatabaseConnection,
        id: i32,
    ) -> Result<(), ApiError> {
        let result = Entity::delete_by_id(id)
            .exec(db)
            .await
            .map_err(ApiError::from)?;

        if result.rows_affected == 0 {
            return Err(ApiError::NotFound);
        }

        Ok(())
    }

    async fn delete_many(
        db: &DatabaseConnection,
        ids: Vec<i32>,
    ) -> Result<u64, ApiError> {
        // Safety limit: max 100 items per request
        if ids.len() > 100 {
            return Err(ApiError::BadRequest(
                "Cannot delete more than 100 items at once".into()
            ));
        }

        let result = Entity::delete_many()
            .filter(Column::Id.is_in(ids))
            .exec(db)
            .await
            .map_err(ApiError::from)?;

        Ok(result.rows_affected)
    }

    async fn total_count(
        db: &DatabaseConnection,
        condition: &Condition,
    ) -> u64 {
        Entity::find()
            .filter(condition.clone())
            .count(db)
            .await
            .unwrap_or(0)
    }
}
```

## Using CRUDResource Directly

You can call trait methods directly:

```rust
use crudcrate::CRUDResource;

// Get one
let task = Task::get_one(&db, 42).await?;

// Get all with filtering
let condition = Condition::all()
    .add(Column::Status.eq("active"));
let tasks = Task::get_all(&db, condition, (Column::Id, Order::Asc), 0, 10).await?;

// Create
let new_task = Task::create(&db, TaskCreate {
    title: "New task".into(),
}).await?;

// Update
let updated = Task::update(&db, 42, TaskUpdate {
    title: Some("Updated title".into()),
}).await?;

// Delete
Task::delete(&db, 42).await?;

// Count
let count = Task::total_count(&db, &Condition::all()).await;
```

## Relationship Loading

When entities have relationships, `get_one` loads them:

```rust
// Entity with relationship
#[sea_orm(ignore)]
#[crudcrate(non_db_attr, join(one))]
pub comments: Vec<Comment>,

// Generated get_one includes relationship loading
async fn get_one(db: &DatabaseConnection, id: i32) -> Result<Self, ApiError> {
    let model = Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(ApiError::NotFound)?;

    // Load related comments
    let comments = model
        .find_related(comment::Entity)
        .all(db)
        .await?
        .into_iter()
        .map(|c| c.into())
        .collect();

    Ok(Task {
        id: model.id,
        title: model.title,
        comments,  // Loaded!
    })
}
```

## Custom Operations with CRUDOperations

Extend behavior without reimplementing the trait:

```rust
#[derive(EntityToModels)]
#[crudcrate(generate_router, operations = TaskOperations)]
pub struct Model { /* ... */ }

pub struct TaskOperations;

#[async_trait]
impl CRUDOperations for TaskOperations {
    type Resource = Task;

    /// Called before create
    async fn before_create(
        &self,
        db: &DatabaseConnection,
        data: &mut TaskCreate,
    ) -> Result<(), ApiError> {
        // Validate or transform
        data.title = data.title.trim().to_string();
        Ok(())
    }

    /// Called after create
    async fn after_create(
        &self,
        db: &DatabaseConnection,
        created: &Task,
    ) -> Result<(), ApiError> {
        // Send notification, update cache, etc.
        Ok(())
    }

    /// Called before delete
    async fn before_delete(
        &self,
        db: &DatabaseConnection,
        id: i32,
    ) -> Result<(), ApiError> {
        // Check permissions, cascade deletes, etc.
        Ok(())
    }
}
```

## Handler Integration

The generated router uses `CRUDResource` methods:

```rust
// Generated handler (simplified)
async fn get_handler(
    Path(id): Path<i32>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Task>, ApiError> {
    let item = Task::get_one(&db, id).await?;
    Ok(Json(item))
}

async fn list_handler(
    Query(params): Query<FilterOptions>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<(HeaderMap, Json<Vec<TaskList>>), ApiError> {
    let condition = apply_filters::<Entity>(&params)?;
    let (offset, limit) = parse_pagination(&params);
    let order = parse_sorting::<Entity>(&params);

    let items = Task::get_all(&db, condition, order, offset, limit).await?;
    let total = Task::total_count(&db, &condition).await;

    let headers = calculate_content_range("tasks", offset, items.len(), total);

    Ok((headers, Json(items)))
}
```

## Type Associations

The trait's associated types connect everything:

```rust
impl CRUDResource for Task {
    // Links to Sea-ORM entity for database operations
    type EntityType = Entity;

    // Request model for POST /tasks
    type CreateModel = TaskCreate;

    // Request model for PUT /tasks/:id
    type UpdateModel = TaskUpdate;

    // Response model for GET /tasks
    type ListModel = TaskList;

    // Type of the primary key
    type PrimaryKey = i32;
}
```

This enables:
- Type-safe column references (`Entity::Column`)
- Correct serialization/deserialization
- Compile-time verification of operations

## Error Handling

All methods return `Result<T, ApiError>`:

```rust
async fn get_one(db: &DatabaseConnection, id: i32) -> Result<Self, ApiError> {
    Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(ApiError::from)?  // Database error → 500
        .ok_or(ApiError::NotFound)  // Not found → 404
}
```

Database errors are logged internally but return sanitized messages to clients.

## Next Steps

- Learn about [Attributes](./attributes.md) for configuration
- Implement [Custom Operations](../advanced/custom-operations.md)
- Configure [Error Handling](../features/error-handling.md)
