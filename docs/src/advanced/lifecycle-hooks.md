# Lifecycle Hooks

Configure operation hooks directly through attributes for simple cases.

## Overview

Lifecycle hooks provide attribute-based customization without implementing `CRUDOperations`:

```rust
#[derive(EntityToModels)]
#[crudcrate(
    generate_router,
    create::one::pre = validate_article,
    create::one::post = notify_subscribers,
    update::one::pre = check_permissions,
    delete::one::pre = archive_instead_of_delete,
)]
pub struct Model {
    // ...
}
```

## Hook Types

### Pre Hooks (`::pre`)

Execute **before** the database operation:

```rust
#[crudcrate(create::one::pre = validate_article)]

// Hook function signature
async fn validate_article(
    db: &DatabaseConnection,
    data: &mut ArticleCreate,
) -> Result<(), ApiError> {
    if data.title.is_empty() {
        return Err(ApiError::ValidationFailed(vec![
            ValidationError::new("title", "Title is required")
        ]));
    }
    Ok(())
}
```

### Post Hooks (`::post`)

Execute **after** the database operation succeeds:

```rust
#[crudcrate(create::one::post = notify_subscribers)]

// Hook function signature
async fn notify_subscribers(
    db: &DatabaseConnection,
    created: &Article,
) -> Result<(), ApiError> {
    send_notifications(created.author_id).await;
    Ok(())
}
```

### Body Replacement (`::body`)

**Replace** the entire operation logic:

```rust
#[crudcrate(delete::one::body = soft_delete)]

// Completely replaces the delete handler
async fn soft_delete(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError> {
    // Instead of deleting, mark as deleted
    let mut article: ActiveModel = Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(ApiError::NotFound)?
        .into();

    article.deleted_at = Set(Some(chrono::Utc::now()));
    article.update(db).await?;

    Ok(())
}
```

## Available Operations

| Operation | Description |
|-----------|-------------|
| `create::one` | Create single item (POST /items) |
| `update::one` | Update single item (PUT /items/:id) |
| `delete::one` | Delete single item (DELETE /items/:id) |
| `delete::many` | Bulk delete (DELETE /items with body) |
| `get::one` | Get single item (GET /items/:id) |
| `get::all` | List items (GET /items) |

## Function Signatures

### Create Hooks

```rust
// Pre hook - can modify the input
async fn create_pre(
    db: &DatabaseConnection,
    data: &mut ArticleCreate,
) -> Result<(), ApiError>;

// Post hook - receives the created entity
async fn create_post(
    db: &DatabaseConnection,
    created: &Article,
) -> Result<(), ApiError>;

// Body replacement - full control
async fn create_body(
    db: &DatabaseConnection,
    data: ArticleCreate,
) -> Result<Article, ApiError>;
```

### Update Hooks

```rust
// Pre hook
async fn update_pre(
    db: &DatabaseConnection,
    id: Uuid,
    data: &mut ArticleUpdate,
) -> Result<(), ApiError>;

// Post hook
async fn update_post(
    db: &DatabaseConnection,
    updated: &Article,
) -> Result<(), ApiError>;

// Body replacement
async fn update_body(
    db: &DatabaseConnection,
    id: Uuid,
    data: ArticleUpdate,
) -> Result<Article, ApiError>;
```

### Delete Hooks

```rust
// Pre hook
async fn delete_pre(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError>;

// Post hook
async fn delete_post(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError>;

// Body replacement
async fn delete_body(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError>;
```

### Get Hooks

```rust
// Pre hook for get_one
async fn get_one_pre(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError>;

// Pre hook for get_all - can modify the condition
async fn get_all_pre(
    db: &DatabaseConnection,
    condition: &mut Condition,
) -> Result<(), ApiError>;
```

## Examples

### Input Validation

```rust
#[crudcrate(create::one::pre = validate_user)]

async fn validate_user(
    _db: &DatabaseConnection,
    data: &mut UserCreate,
) -> Result<(), ApiError> {
    let mut errors = Vec::new();

    // Email validation
    if !data.email.contains('@') {
        errors.push(ValidationError::new("email", "Invalid email format"));
    }

    // Password strength
    if data.password.len() < 8 {
        errors.push(ValidationError::new("password", "Must be at least 8 characters"));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::ValidationFailed(errors))
    }
}
```

### Password Hashing

```rust
#[crudcrate(create::one::pre = hash_password)]

async fn hash_password(
    _db: &DatabaseConnection,
    data: &mut UserCreate,
) -> Result<(), ApiError> {
    // Hash the password before storing
    data.password_hash = bcrypt::hash(&data.password, bcrypt::DEFAULT_COST)
        .map_err(|_| ApiError::Internal("Failed to hash password".into()))?;

    // Clear the plain password
    data.password.clear();

    Ok(())
}
```

### Soft Delete

```rust
#[crudcrate(delete::one::body = soft_delete_article)]

async fn soft_delete_article(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError> {
    let article = Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(ApiError::NotFound)?;

    let mut active: ActiveModel = article.into();
    active.deleted_at = Set(Some(chrono::Utc::now()));
    active.update(db).await?;

    Ok(())
}
```

### Row-Level Security

```rust
#[crudcrate(get::all::pre = filter_by_tenant)]

async fn filter_by_tenant(
    _db: &DatabaseConnection,
    condition: &mut Condition,
) -> Result<(), ApiError> {
    let tenant_id = get_current_tenant_id();

    *condition = condition.clone().add(Column::TenantId.eq(tenant_id));

    Ok(())
}
```

### Audit Trail

```rust
#[crudcrate(
    create::one::post = log_create,
    update::one::post = log_update,
    delete::one::post = log_delete,
)]

async fn log_create(
    db: &DatabaseConnection,
    created: &Article,
) -> Result<(), ApiError> {
    save_audit_log(db, "CREATE", "Article", &created.id.to_string()).await
}

async fn log_update(
    db: &DatabaseConnection,
    updated: &Article,
) -> Result<(), ApiError> {
    save_audit_log(db, "UPDATE", "Article", &updated.id.to_string()).await
}

async fn log_delete(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError> {
    save_audit_log(db, "DELETE", "Article", &id.to_string()).await
}
```

### Cascade Updates

```rust
#[crudcrate(update::one::post = update_search_index)]

async fn update_search_index(
    _db: &DatabaseConnection,
    updated: &Article,
) -> Result<(), ApiError> {
    // Update search engine
    search_client::update_document("articles", updated.id, updated).await;
    Ok(())
}
```

### Authorization Check

```rust
#[crudcrate(
    update::one::pre = check_edit_permission,
    delete::one::pre = check_delete_permission,
)]

async fn check_edit_permission(
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

async fn check_delete_permission(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError> {
    let article = Entity::find_by_id(id).one(db).await?.ok_or(ApiError::NotFound)?;
    let user = get_current_user();

    if !user.is_admin {
        return Err(ApiError::Forbidden);
    }

    Ok(())
}
```

## Hooks vs CRUDOperations

| Feature | Hooks | CRUDOperations |
|---------|-------|----------------|
| Configuration | Attribute | Trait impl |
| Scope | Per-operation | All operations |
| State | Stateless functions | Can hold state |
| Testing | Function mocking | Trait mocking |
| Use when | Simple cases | Complex logic |

### When to Use Hooks

- Simple validation
- Data transformation
- Logging/notifications
- Single operation customization

### When to Use CRUDOperations

- Complex authorization logic
- Shared state across operations
- Database-dependent validation
- Multiple operations need same logic

## Combining Both

You can use both hooks and CRUDOperations:

```rust
#[derive(EntityToModels)]
#[crudcrate(
    generate_router,
    operations = ArticleOperations,
    create::one::pre = validate_article,  // Runs before CRUDOperations::before_create
)]
pub struct Model { }
```

Execution order:
1. Attribute hook (`create::one::pre`)
2. `CRUDOperations::before_create`
3. Database operation
4. `CRUDOperations::after_create`
5. Attribute hook (`create::one::post`)

## Next Steps

- Learn about [Validation](./validation.md)
- Configure [Security](./security.md)
- Set up [Performance Optimization](./performance.md)
