# Custom Operations

Extend CRUDCrate's default behavior with the `CRUDOperations` trait.

## Overview

`CRUDOperations` provides hooks into every CRUD operation:

```rust
#[async_trait]
pub trait CRUDOperations {
    type Resource: CRUDResource;

    // Create hooks
    async fn before_create(&self, db: &DatabaseConnection, data: &mut CreateModel) -> Result<(), ApiError>;
    async fn after_create(&self, db: &DatabaseConnection, created: &Resource) -> Result<(), ApiError>;

    // Update hooks
    async fn before_update(&self, db: &DatabaseConnection, id: PK, data: &mut UpdateModel) -> Result<(), ApiError>;
    async fn after_update(&self, db: &DatabaseConnection, updated: &Resource) -> Result<(), ApiError>;

    // Delete hooks
    async fn before_delete(&self, db: &DatabaseConnection, id: PK) -> Result<(), ApiError>;
    async fn after_delete(&self, db: &DatabaseConnection, id: PK) -> Result<(), ApiError>;

    // Read hooks
    async fn before_get_one(&self, db: &DatabaseConnection, id: PK) -> Result<(), ApiError>;
    async fn before_get_all(&self, db: &DatabaseConnection, condition: &mut Condition) -> Result<(), ApiError>;
}
```

## Basic Setup

### Step 1: Create Operations Struct

```rust
pub struct ArticleOperations;
```

### Step 2: Implement CRUDOperations

```rust
use async_trait::async_trait;
use crudcrate::{CRUDOperations, ApiError};
use sea_orm::DatabaseConnection;

#[async_trait]
impl CRUDOperations for ArticleOperations {
    type Resource = Article;

    async fn before_create(
        &self,
        _db: &DatabaseConnection,
        data: &mut ArticleCreate,
    ) -> Result<(), ApiError> {
        // Trim whitespace
        data.title = data.title.trim().to_string();
        data.content = data.content.trim().to_string();
        Ok(())
    }

    async fn after_create(
        &self,
        _db: &DatabaseConnection,
        created: &Article,
    ) -> Result<(), ApiError> {
        println!("Article created: {}", created.id);
        Ok(())
    }
}
```

### Step 3: Register with Entity

```rust
#[derive(EntityToModels)]
#[crudcrate(generate_router, operations = ArticleOperations)]
pub struct Model {
    // ...
}
```

## Hook Execution Order

### Create

```
1. Parse request body → ArticleCreate
2. before_create(&mut data)
3. Convert to ActiveModel
4. Insert into database
5. after_create(&created)
6. Return response
```

### Update

```
1. Parse request body → ArticleUpdate
2. before_update(id, &mut data)
3. Load existing record
4. Merge updates
5. Update database
6. after_update(&updated)
7. Return response
```

### Delete

```
1. before_delete(id)
2. Delete from database
3. after_delete(id)
4. Return success
```

### Read

```
# get_one
1. before_get_one(id)
2. Query database
3. Return response

# get_all
1. before_get_all(&mut condition)
2. Query database with condition
3. Return response
```

## Common Use Cases

### Input Validation

```rust
async fn before_create(
    &self,
    _db: &DatabaseConnection,
    data: &mut ArticleCreate,
) -> Result<(), ApiError> {
    let mut errors = Vec::new();

    if data.title.len() < 5 {
        errors.push(ValidationError::new("title", "Title must be at least 5 characters"));
    }

    if data.title.len() > 200 {
        errors.push(ValidationError::new("title", "Title cannot exceed 200 characters"));
    }

    if data.content.is_empty() {
        errors.push(ValidationError::new("content", "Content is required"));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::ValidationFailed(errors))
    }
}
```

### Data Transformation

```rust
async fn before_create(
    &self,
    _db: &DatabaseConnection,
    data: &mut ArticleCreate,
) -> Result<(), ApiError> {
    // Generate slug from title
    data.slug = Some(slugify(&data.title));

    // Sanitize HTML content
    data.content = sanitize_html(&data.content);

    // Normalize tags
    if let Some(tags) = &mut data.tags {
        *tags = tags.iter()
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty())
            .collect();
    }

    Ok(())
}
```

### Uniqueness Checks

```rust
async fn before_create(
    &self,
    db: &DatabaseConnection,
    data: &mut ArticleCreate,
) -> Result<(), ApiError> {
    // Check if slug already exists
    let slug = slugify(&data.title);

    let exists = Entity::find()
        .filter(Column::Slug.eq(&slug))
        .count(db)
        .await
        .map_err(|_| ApiError::Internal("Database error".into()))?;

    if exists > 0 {
        // Append random suffix
        data.slug = Some(format!("{}-{}", slug, random_string(6)));
    } else {
        data.slug = Some(slug);
    }

    Ok(())
}
```

### Authorization

```rust
async fn before_update(
    &self,
    db: &DatabaseConnection,
    id: Uuid,
    _data: &mut ArticleUpdate,
) -> Result<(), ApiError> {
    // Get current user from context (you'd need to pass this)
    let current_user = get_current_user();

    // Load the article
    let article = Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(ApiError::NotFound)?;

    // Check ownership
    if article.author_id != current_user.id && !current_user.is_admin {
        return Err(ApiError::Forbidden);
    }

    Ok(())
}

async fn before_delete(
    &self,
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError> {
    // Same authorization check
    let current_user = get_current_user();
    let article = Entity::find_by_id(id).one(db).await?.ok_or(ApiError::NotFound)?;

    if article.author_id != current_user.id && !current_user.is_admin {
        return Err(ApiError::Forbidden);
    }

    Ok(())
}
```

### Cascading Operations

```rust
async fn before_delete(
    &self,
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError> {
    // Delete related comments first
    comment::Entity::delete_many()
        .filter(comment::Column::ArticleId.eq(id))
        .exec(db)
        .await
        .map_err(|_| ApiError::Internal("Failed to delete comments".into()))?;

    // Delete related tags
    article_tag::Entity::delete_many()
        .filter(article_tag::Column::ArticleId.eq(id))
        .exec(db)
        .await
        .map_err(|_| ApiError::Internal("Failed to delete tags".into()))?;

    Ok(())
}
```

### Notifications

```rust
async fn after_create(
    &self,
    db: &DatabaseConnection,
    created: &Article,
) -> Result<(), ApiError> {
    // Notify subscribers
    let subscribers = get_subscribers_for_author(db, created.author_id).await?;

    for subscriber in subscribers {
        send_notification(
            subscriber.email,
            format!("New article: {}", created.title),
        ).await;
    }

    Ok(())
}
```

### Audit Logging

```rust
async fn after_create(
    &self,
    db: &DatabaseConnection,
    created: &Article,
) -> Result<(), ApiError> {
    create_audit_log(db, AuditLog {
        action: "CREATE",
        entity: "Article",
        entity_id: created.id.to_string(),
        user_id: get_current_user().id,
        timestamp: chrono::Utc::now(),
        changes: serde_json::to_value(created).ok(),
    }).await?;

    Ok(())
}

async fn after_update(
    &self,
    db: &DatabaseConnection,
    updated: &Article,
) -> Result<(), ApiError> {
    create_audit_log(db, AuditLog {
        action: "UPDATE",
        entity: "Article",
        entity_id: updated.id.to_string(),
        user_id: get_current_user().id,
        timestamp: chrono::Utc::now(),
        changes: None,  // Could include diff
    }).await?;

    Ok(())
}
```

### Row-Level Security

```rust
async fn before_get_all(
    &self,
    _db: &DatabaseConnection,
    condition: &mut Condition,
) -> Result<(), ApiError> {
    let current_user = get_current_user();

    if !current_user.is_admin {
        // Non-admins only see their own articles
        *condition = condition.clone().add(Column::AuthorId.eq(current_user.id));
    }

    Ok(())
}
```

## Complete Example

```rust
use async_trait::async_trait;
use crudcrate::{CRUDOperations, ApiError, ValidationError};
use sea_orm::{DatabaseConnection, EntityTrait, ColumnTrait, QueryFilter};

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
        if data.title.trim().len() < 5 {
            errors.push(ValidationError::new("title", "Title too short"));
        }
        if !errors.is_empty() {
            return Err(ApiError::ValidationFailed(errors));
        }

        // Transform
        data.title = data.title.trim().to_string();
        data.slug = Some(slugify(&data.title));

        // Ensure unique slug
        let mut slug = data.slug.clone().unwrap();
        let mut counter = 1;
        while slug_exists(db, &slug).await? {
            slug = format!("{}-{}", data.slug.as_ref().unwrap(), counter);
            counter += 1;
        }
        data.slug = Some(slug);

        Ok(())
    }

    async fn after_create(
        &self,
        db: &DatabaseConnection,
        created: &Article,
    ) -> Result<(), ApiError> {
        // Log
        tracing::info!(article_id = %created.id, "Article created");

        // Index for search
        index_article_for_search(created).await;

        // Notify
        notify_followers(db, created).await?;

        Ok(())
    }

    async fn before_update(
        &self,
        db: &DatabaseConnection,
        id: Uuid,
        data: &mut ArticleUpdate,
    ) -> Result<(), ApiError> {
        // Authorize
        let article = Entity::find_by_id(id).one(db).await?.ok_or(ApiError::NotFound)?;
        let user = get_current_user();

        if article.author_id != user.id && !user.is_admin {
            return Err(ApiError::Forbidden);
        }

        // Transform title if changed
        if let Some(ref mut title) = data.title {
            *title = title.trim().to_string();
        }

        Ok(())
    }

    async fn before_delete(
        &self,
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<(), ApiError> {
        // Authorize
        let article = Entity::find_by_id(id).one(db).await?.ok_or(ApiError::NotFound)?;
        let user = get_current_user();

        if article.author_id != user.id && !user.is_admin {
            return Err(ApiError::Forbidden);
        }

        // Cascade delete
        comment::Entity::delete_many()
            .filter(comment::Column::ArticleId.eq(id))
            .exec(db)
            .await?;

        Ok(())
    }

    async fn after_delete(
        &self,
        _db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<(), ApiError> {
        // Remove from search index
        remove_from_search_index(id).await;

        tracing::info!(article_id = %id, "Article deleted");

        Ok(())
    }

    async fn before_get_all(
        &self,
        _db: &DatabaseConnection,
        condition: &mut Condition,
    ) -> Result<(), ApiError> {
        // Only show published articles (unless admin)
        let user = get_current_user();
        if !user.is_admin {
            *condition = condition.clone().add(Column::Status.eq("published"));
        }

        Ok(())
    }
}
```

## Default Implementation

If you don't need all hooks, the trait has default no-op implementations:

```rust
#[async_trait]
impl CRUDOperations for ArticleOperations {
    type Resource = Article;

    // Only implement what you need
    async fn before_create(
        &self,
        _db: &DatabaseConnection,
        data: &mut ArticleCreate,
    ) -> Result<(), ApiError> {
        data.title = data.title.trim().to_string();
        Ok(())
    }

    // All other hooks use default (do nothing)
}
```

## Next Steps

- Learn about [Lifecycle Hooks](./lifecycle-hooks.md) for attribute-based hooks
- Configure [Validation](./validation.md)
- Set up [Security](./security.md)
