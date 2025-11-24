# CRUDOperations API Reference

The `CRUDOperations` trait provides hooks to customize CRUD behavior.

## Trait Definition

```rust
#[async_trait]
pub trait CRUDOperations: Send + Sync {
    /// The resource type this operates on
    type Resource: CRUDResource;

    /// Called before creating a new record
    async fn before_create(
        &self,
        db: &DatabaseConnection,
        data: &mut <Self::Resource as CRUDResource>::CreateModel,
    ) -> Result<(), ApiError> {
        Ok(())  // Default: no-op
    }

    /// Called after successfully creating a record
    async fn after_create(
        &self,
        db: &DatabaseConnection,
        created: &Self::Resource,
    ) -> Result<(), ApiError> {
        Ok(())  // Default: no-op
    }

    /// Called before updating a record
    async fn before_update(
        &self,
        db: &DatabaseConnection,
        id: <Self::Resource as CRUDResource>::PrimaryKey,
        data: &mut <Self::Resource as CRUDResource>::UpdateModel,
    ) -> Result<(), ApiError> {
        Ok(())  // Default: no-op
    }

    /// Called after successfully updating a record
    async fn after_update(
        &self,
        db: &DatabaseConnection,
        updated: &Self::Resource,
    ) -> Result<(), ApiError> {
        Ok(())  // Default: no-op
    }

    /// Called before deleting a record
    async fn before_delete(
        &self,
        db: &DatabaseConnection,
        id: <Self::Resource as CRUDResource>::PrimaryKey,
    ) -> Result<(), ApiError> {
        Ok(())  // Default: no-op
    }

    /// Called after successfully deleting a record
    async fn after_delete(
        &self,
        db: &DatabaseConnection,
        id: <Self::Resource as CRUDResource>::PrimaryKey,
    ) -> Result<(), ApiError> {
        Ok(())  // Default: no-op
    }

    /// Called before get_one
    async fn before_get_one(
        &self,
        db: &DatabaseConnection,
        id: <Self::Resource as CRUDResource>::PrimaryKey,
    ) -> Result<(), ApiError> {
        Ok(())  // Default: no-op
    }

    /// Called before get_all, can modify the condition
    async fn before_get_all(
        &self,
        db: &DatabaseConnection,
        condition: &mut Condition,
    ) -> Result<(), ApiError> {
        Ok(())  // Default: no-op
    }
}
```

## Implementation

### Basic Implementation

```rust
use async_trait::async_trait;
use crudcrate::{CRUDOperations, ApiError};
use sea_orm::DatabaseConnection;

pub struct ArticleOperations;

#[async_trait]
impl CRUDOperations for ArticleOperations {
    type Resource = Article;

    // Override only the hooks you need
    async fn before_create(
        &self,
        _db: &DatabaseConnection,
        data: &mut ArticleCreate,
    ) -> Result<(), ApiError> {
        data.title = data.title.trim().to_string();
        Ok(())
    }
}
```

### Registration

```rust
#[derive(EntityToModels)]
#[crudcrate(operations = ArticleOperations)]
pub struct Model { }
```

## Hook Methods

### `before_create`

Called before inserting a new record. Can modify the input data.

```rust
async fn before_create(
    &self,
    db: &DatabaseConnection,
    data: &mut ArticleCreate,
) -> Result<(), ApiError>;
```

**Use Cases:**
- Input validation
- Data transformation (trim, normalize)
- Uniqueness checks
- Generating derived values (slug from title)

**Example:**

```rust
async fn before_create(
    &self,
    db: &DatabaseConnection,
    data: &mut ArticleCreate,
) -> Result<(), ApiError> {
    // Validate
    if data.title.len() < 5 {
        return Err(ApiError::ValidationFailed(vec![
            ValidationError::new("title", "Too short")
        ]));
    }

    // Transform
    data.slug = Some(slugify(&data.title));

    // Check uniqueness
    if slug_exists(db, data.slug.as_ref().unwrap()).await? {
        return Err(ApiError::Conflict("Slug already exists".into()));
    }

    Ok(())
}
```

---

### `after_create`

Called after the record is successfully inserted.

```rust
async fn after_create(
    &self,
    db: &DatabaseConnection,
    created: &Article,
) -> Result<(), ApiError>;
```

**Use Cases:**
- Sending notifications
- Updating search indexes
- Creating audit logs
- Triggering background jobs

**Example:**

```rust
async fn after_create(
    &self,
    db: &DatabaseConnection,
    created: &Article,
) -> Result<(), ApiError> {
    // Log creation
    tracing::info!(article_id = %created.id, "Article created");

    // Index for search
    search_client::index_document("articles", created).await;

    // Notify followers
    notify_followers(db, created.author_id).await?;

    Ok(())
}
```

---

### `before_update`

Called before updating a record. Can modify the update data.

```rust
async fn before_update(
    &self,
    db: &DatabaseConnection,
    id: Uuid,
    data: &mut ArticleUpdate,
) -> Result<(), ApiError>;
```

**Use Cases:**
- Authorization checks (user owns resource)
- Preventing certain field updates
- Validating state transitions
- Transforming input

**Example:**

```rust
async fn before_update(
    &self,
    db: &DatabaseConnection,
    id: Uuid,
    data: &mut ArticleUpdate,
) -> Result<(), ApiError> {
    // Authorization
    let article = Entity::find_by_id(id).one(db).await?.ok_or(ApiError::NotFound)?;
    let user = get_current_user();

    if article.author_id != user.id && !user.is_admin {
        return Err(ApiError::Forbidden);
    }

    // Prevent changing author
    data.author_id = None;

    Ok(())
}
```

---

### `after_update`

Called after the record is successfully updated.

```rust
async fn after_update(
    &self,
    db: &DatabaseConnection,
    updated: &Article,
) -> Result<(), ApiError>;
```

**Use Cases:**
- Invalidating caches
- Updating search indexes
- Creating audit logs
- Sending change notifications

**Example:**

```rust
async fn after_update(
    &self,
    _db: &DatabaseConnection,
    updated: &Article,
) -> Result<(), ApiError> {
    // Update search index
    search_client::update_document("articles", updated.id, updated).await;

    // Invalidate cache
    cache::invalidate(&format!("article:{}", updated.id)).await;

    Ok(())
}
```

---

### `before_delete`

Called before deleting a record.

```rust
async fn before_delete(
    &self,
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError>;
```

**Use Cases:**
- Authorization checks
- Preventing deletion of important records
- Cascading deletes
- Checking references

**Example:**

```rust
async fn before_delete(
    &self,
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError> {
    // Only admins can delete
    let user = get_current_user();
    if !user.is_admin {
        return Err(ApiError::Forbidden);
    }

    // Cascade delete comments
    comment::Entity::delete_many()
        .filter(comment::Column::ArticleId.eq(id))
        .exec(db)
        .await?;

    Ok(())
}
```

---

### `after_delete`

Called after the record is successfully deleted.

```rust
async fn after_delete(
    &self,
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError>;
```

**Use Cases:**
- Cleanup (files, related data)
- Removing from search index
- Creating audit logs
- Sending notifications

**Example:**

```rust
async fn after_delete(
    &self,
    _db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError> {
    // Remove from search
    search_client::delete_document("articles", id).await;

    // Delete uploaded files
    storage::delete_files_for_article(id).await;

    tracing::info!(article_id = %id, "Article deleted");

    Ok(())
}
```

---

### `before_get_one`

Called before fetching a single record.

```rust
async fn before_get_one(
    &self,
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError>;
```

**Use Cases:**
- Authorization checks
- Access logging
- Rate limiting

**Example:**

```rust
async fn before_get_one(
    &self,
    _db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError> {
    // Log access
    tracing::info!(article_id = %id, "Article accessed");

    // Rate limit check
    if is_rate_limited() {
        return Err(ApiError::BadRequest("Rate limited".into()));
    }

    Ok(())
}
```

---

### `before_get_all`

Called before fetching multiple records. Can modify the condition.

```rust
async fn before_get_all(
    &self,
    db: &DatabaseConnection,
    condition: &mut Condition,
) -> Result<(), ApiError>;
```

**Use Cases:**
- Row-level security (filter by tenant/user)
- Adding default filters (exclude deleted)
- Access control

**Example:**

```rust
async fn before_get_all(
    &self,
    _db: &DatabaseConnection,
    condition: &mut Condition,
) -> Result<(), ApiError> {
    let user = get_current_user();

    // Non-admins only see published articles
    if !user.is_admin {
        *condition = condition.clone()
            .add(Column::Status.eq("published"));
    }

    // Always exclude soft-deleted
    *condition = condition.clone()
        .add(Column::DeletedAt.is_null());

    Ok(())
}
```

## Complete Example

```rust
use async_trait::async_trait;
use crudcrate::{CRUDOperations, ApiError, ValidationError};
use sea_orm::{DatabaseConnection, EntityTrait, Condition, ColumnTrait};

pub struct ArticleOperations;

#[async_trait]
impl CRUDOperations for ArticleOperations {
    type Resource = Article;

    async fn before_create(
        &self,
        db: &DatabaseConnection,
        data: &mut ArticleCreate,
    ) -> Result<(), ApiError> {
        // Validate
        let mut errors = Vec::new();
        if data.title.len() < 5 {
            errors.push(ValidationError::new("title", "Minimum 5 characters"));
        }
        if !errors.is_empty() {
            return Err(ApiError::ValidationFailed(errors));
        }

        // Transform
        data.title = data.title.trim().to_string();
        data.slug = Some(slugify(&data.title));

        Ok(())
    }

    async fn after_create(
        &self,
        _db: &DatabaseConnection,
        created: &Article,
    ) -> Result<(), ApiError> {
        tracing::info!(id = %created.id, "Article created");
        search_client::index("articles", created).await;
        Ok(())
    }

    async fn before_update(
        &self,
        db: &DatabaseConnection,
        id: Uuid,
        _data: &mut ArticleUpdate,
    ) -> Result<(), ApiError> {
        let article = Entity::find_by_id(id).one(db).await?.ok_or(ApiError::NotFound)?;
        let user = get_current_user();

        if article.author_id != user.id && !user.is_admin {
            return Err(ApiError::Forbidden);
        }

        Ok(())
    }

    async fn after_update(
        &self,
        _db: &DatabaseConnection,
        updated: &Article,
    ) -> Result<(), ApiError> {
        search_client::update("articles", updated).await;
        cache::invalidate(&format!("article:{}", updated.id)).await;
        Ok(())
    }

    async fn before_delete(
        &self,
        _db: &DatabaseConnection,
        _id: Uuid,
    ) -> Result<(), ApiError> {
        let user = get_current_user();
        if !user.is_admin {
            return Err(ApiError::Forbidden);
        }
        Ok(())
    }

    async fn after_delete(
        &self,
        _db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<(), ApiError> {
        search_client::delete("articles", id).await;
        storage::cleanup(id).await;
        Ok(())
    }

    async fn before_get_all(
        &self,
        _db: &DatabaseConnection,
        condition: &mut Condition,
    ) -> Result<(), ApiError> {
        let user = get_current_user();

        if !user.is_admin {
            *condition = condition.clone().add(Column::Status.eq("published"));
        }

        Ok(())
    }
}
```

## See Also

- [CRUDResource API](./crudresource-api.md)
- [Lifecycle Hooks](../advanced/lifecycle-hooks.md)
- [Custom Operations](../advanced/custom-operations.md)
