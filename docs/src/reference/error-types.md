# Error Types Reference

Complete reference for CRUDCrate error types and HTTP responses.

## ApiError Enum

```rust
pub enum ApiError {
    NotFound,
    BadRequest(String),
    Unauthorized,
    Forbidden,
    Conflict(String),
    ValidationFailed(Vec<ValidationError>),
    Database(String),
    Internal(String),
}
```

## Error Types

### NotFound

Resource doesn't exist.

```rust
ApiError::NotFound
```

**HTTP Status:** 404
**Response:**
```json
{"error": "Not found"}
```

**When Used:**
- `get_one` with non-existent ID
- `update` on non-existent record
- `delete` on non-existent record

---

### BadRequest

Invalid request data or parameters.

```rust
ApiError::BadRequest("Invalid filter syntax".into())
```

**HTTP Status:** 400
**Response:**
```json
{"error": "Invalid filter syntax"}
```

**When Used:**
- Invalid query parameters
- Malformed JSON body
- Invalid filter field
- Too many items in bulk operation

---

### Unauthorized

Authentication required or failed.

```rust
ApiError::Unauthorized
```

**HTTP Status:** 401
**Response:**
```json
{"error": "Unauthorized"}
```

**When Used:**
- Missing authentication
- Invalid/expired token
- Custom auth checks in hooks

---

### Forbidden

User lacks permission.

```rust
ApiError::Forbidden
```

**HTTP Status:** 403
**Response:**
```json
{"error": "Forbidden"}
```

**When Used:**
- Authorization checks in hooks
- Row-level security violations
- Role-based access denial

---

### Conflict

Resource conflict (usually uniqueness violation).

```rust
ApiError::Conflict("Email already in use".into())
```

**HTTP Status:** 409
**Response:**
```json
{"error": "Email already in use"}
```

**When Used:**
- Unique constraint violations
- Concurrent modification conflicts
- State machine transitions

---

### ValidationFailed

Input validation errors with field details.

```rust
ApiError::ValidationFailed(vec![
    ValidationError::new("email", "Invalid format"),
    ValidationError::new("password", "Too short"),
])
```

**HTTP Status:** 422
**Response:**
```json
{
  "error": "Validation failed",
  "details": [
    {"field": "email", "message": "Invalid format"},
    {"field": "password", "message": "Too short"}
  ]
}
```

**When Used:**
- Input validation in hooks
- Business rule violations
- Data format errors

---

### Database

Database operation failed (details sanitized).

```rust
ApiError::Database("Database error".into())
```

**HTTP Status:** 500
**Response:**
```json
{"error": "Database error"}
```

**Note:** The actual error is logged server-side, not exposed to clients.

**When Used:**
- Connection failures
- Query errors
- Constraint violations (when not caught as Conflict)

---

### Internal

Generic server error.

```rust
ApiError::Internal("Unexpected error".into())
```

**HTTP Status:** 500
**Response:**
```json
{"error": "Unexpected error"}
```

**When Used:**
- Unhandled errors
- Infrastructure failures
- Configuration errors

## ValidationError

```rust
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl ValidationError {
    pub fn new(field: &str, message: &str) -> Self {
        Self {
            field: field.to_string(),
            message: message.to_string(),
        }
    }
}
```

## Creating Errors

```rust
use crudcrate::{ApiError, ValidationError};

// Not found
return Err(ApiError::NotFound);

// Bad request
return Err(ApiError::BadRequest("Invalid parameter".into()));

// Unauthorized
return Err(ApiError::Unauthorized);

// Forbidden
return Err(ApiError::Forbidden);

// Conflict
return Err(ApiError::Conflict("Duplicate entry".into()));

// Validation failed
return Err(ApiError::ValidationFailed(vec![
    ValidationError::new("email", "Required"),
    ValidationError::new("age", "Must be positive"),
]));

// Database error (use with caution)
return Err(ApiError::Database("Connection failed".into()));

// Internal error
return Err(ApiError::Internal("Configuration error".into()));
```

## Error Conversion

ApiError implements `From` for common types:

```rust
// From sea_orm::DbErr
impl From<DbErr> for ApiError {
    fn from(err: DbErr) -> Self {
        eprintln!("Database error: {}", err);
        ApiError::Database("Database error".into())
    }
}

// Usage with ? operator
async fn get_item(db: &DatabaseConnection, id: i32) -> Result<Item, ApiError> {
    let item = Entity::find_by_id(id)
        .one(db)
        .await?  // DbErr automatically converted
        .ok_or(ApiError::NotFound)?;
    Ok(item.into())
}
```

## HTTP Response Mapping

| Error Type | Status | Body |
|------------|--------|------|
| `NotFound` | 404 | `{"error": "Not found"}` |
| `BadRequest(msg)` | 400 | `{"error": "{msg}"}` |
| `Unauthorized` | 401 | `{"error": "Unauthorized"}` |
| `Forbidden` | 403 | `{"error": "Forbidden"}` |
| `Conflict(msg)` | 409 | `{"error": "{msg}"}` |
| `ValidationFailed(errs)` | 422 | `{"error": "...", "details": [...]}` |
| `Database(msg)` | 500 | `{"error": "{msg}"}` |
| `Internal(msg)` | 500 | `{"error": "{msg}"}` |

## Usage in Hooks

```rust
async fn before_create(
    &self,
    db: &DatabaseConnection,
    data: &mut ArticleCreate,
) -> Result<(), ApiError> {
    // Validation
    let mut errors = Vec::new();

    if data.title.is_empty() {
        errors.push(ValidationError::new("title", "Required"));
    }

    if data.title.len() > 200 {
        errors.push(ValidationError::new("title", "Max 200 characters"));
    }

    if !errors.is_empty() {
        return Err(ApiError::ValidationFailed(errors));
    }

    // Uniqueness check
    let exists = Entity::find()
        .filter(Column::Slug.eq(&data.slug))
        .count(db)
        .await?;

    if exists > 0 {
        return Err(ApiError::Conflict("Article with this slug exists".into()));
    }

    Ok(())
}

async fn before_update(
    &self,
    db: &DatabaseConnection,
    id: Uuid,
    _data: &mut ArticleUpdate,
) -> Result<(), ApiError> {
    let article = Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(ApiError::NotFound)?;

    let user = get_current_user();

    if article.author_id != user.id {
        return Err(ApiError::Forbidden);
    }

    Ok(())
}
```

## Custom Error Extension

Create domain-specific errors:

```rust
pub enum DomainError {
    InsufficientFunds(f64),
    OrderExpired,
    InventoryDepleted(String),
}

impl From<DomainError> for ApiError {
    fn from(err: DomainError) -> Self {
        match err {
            DomainError::InsufficientFunds(needed) => {
                ApiError::BadRequest(format!("Insufficient funds. Need ${:.2}", needed))
            }
            DomainError::OrderExpired => {
                ApiError::BadRequest("Order has expired".into())
            }
            DomainError::InventoryDepleted(item) => {
                ApiError::Conflict(format!("'{}' is out of stock", item))
            }
        }
    }
}

// Usage
fn process_order(order: &Order) -> Result<(), ApiError> {
    if order.is_expired() {
        return Err(DomainError::OrderExpired.into());
    }
    Ok(())
}
```

## See Also

- [Error Handling](../features/error-handling.md)
- [Validation](../advanced/validation.md)
- [Security](../advanced/security.md)
