# Error Handling

CRUDCrate provides comprehensive error handling with security-conscious responses.

## ApiError Types

```rust
pub enum ApiError {
    /// Resource not found (404)
    NotFound,

    /// Invalid request data (400)
    BadRequest(String),

    /// Authentication required (401)
    Unauthorized,

    /// Permission denied (403)
    Forbidden,

    /// Resource conflict (409)
    Conflict(String),

    /// Validation failed (422)
    ValidationFailed(Vec<ValidationError>),

    /// Database error (500) - details logged, not exposed
    Database(String),

    /// Generic internal error (500)
    Internal(String),
}
```

## HTTP Status Codes

| Error | Status | When |
|-------|--------|------|
| `NotFound` | 404 | Resource doesn't exist |
| `BadRequest` | 400 | Invalid input format |
| `Unauthorized` | 401 | Missing/invalid auth |
| `Forbidden` | 403 | Insufficient permissions |
| `Conflict` | 409 | Duplicate key, constraint violation |
| `ValidationFailed` | 422 | Business rule validation failed |
| `Database` | 500 | Database error (sanitized) |
| `Internal` | 500 | Other server error |

## Response Format

```json
// NotFound
{
  "error": "Not found"
}

// BadRequest
{
  "error": "Invalid filter field: unknown_field"
}

// ValidationFailed
{
  "error": "Validation failed",
  "details": [
    {"field": "email", "message": "Invalid email format"},
    {"field": "age", "message": "Must be at least 18"}
  ]
}

// Database (sanitized)
{
  "error": "Database error"
}
```

## Using ApiError

### In Handlers

```rust
use crudcrate::ApiError;

async fn get_item(
    Path(id): Path<i32>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Item>, ApiError> {
    let item = Entity::find_by_id(id)
        .one(&db)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(item.into()))
}
```

### In CRUDOperations

```rust
impl CRUDOperations for MyOperations {
    type Resource = Item;

    async fn before_create(
        &self,
        db: &DatabaseConnection,
        data: &mut ItemCreate,
    ) -> Result<(), ApiError> {
        // Validation
        if data.name.is_empty() {
            return Err(ApiError::ValidationFailed(vec![
                ValidationError::new("name", "Name cannot be empty")
            ]));
        }

        // Permission check
        if !user_can_create(&data) {
            return Err(ApiError::Forbidden);
        }

        // Uniqueness check
        if name_exists(db, &data.name).await? {
            return Err(ApiError::Conflict("Name already exists".into()));
        }

        Ok(())
    }
}
```

## Validation Errors

For detailed field-level errors:

```rust
use crudcrate::ValidationError;

fn validate_user(data: &UserCreate) -> Result<(), ApiError> {
    let mut errors = Vec::new();

    if data.email.is_empty() {
        errors.push(ValidationError::new("email", "Email is required"));
    } else if !data.email.contains('@') {
        errors.push(ValidationError::new("email", "Invalid email format"));
    }

    if data.password.len() < 8 {
        errors.push(ValidationError::new("password", "Password must be at least 8 characters"));
    }

    if data.age < 18 {
        errors.push(ValidationError::new("age", "Must be at least 18 years old"));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::ValidationFailed(errors))
    }
}
```

Response:

```json
{
  "error": "Validation failed",
  "details": [
    {"field": "email", "message": "Invalid email format"},
    {"field": "password", "message": "Password must be at least 8 characters"}
  ]
}
```

## Error Conversion

ApiError implements `From` for common error types:

```rust
// Sea-ORM errors
impl From<DbErr> for ApiError {
    fn from(err: DbErr) -> Self {
        // Log full error internally
        eprintln!("Database error: {}", err);
        // Return sanitized response
        ApiError::Database("Database error".into())
    }
}

// Use with ? operator
async fn create_item(data: ItemCreate, db: &DatabaseConnection) -> Result<Item, ApiError> {
    let active_model: ActiveModel = data.into();
    let result = active_model.insert(db).await?;  // Auto-converts DbErr
    Ok(result.into())
}
```

## Security Considerations

### Never Expose Internal Details

```rust
// ❌ Bad: exposes internal error
Err(ApiError::Database(format!("SQL error: {}", e)))

// ✅ Good: log internally, sanitize response
eprintln!("Database error: {}", e);
Err(ApiError::Database("Database error".into()))
```

### Handle Unique Constraint Violations

```rust
async fn create_user(data: UserCreate, db: &DatabaseConnection) -> Result<User, ApiError> {
    let result = Entity::insert(data.into())
        .exec(db)
        .await;

    match result {
        Ok(res) => { /* ... */ },
        Err(DbErr::RecordNotInserted) => {
            Err(ApiError::Conflict("User with this email already exists".into()))
        },
        Err(e) => {
            eprintln!("Insert error: {}", e);
            Err(ApiError::Database("Failed to create user".into()))
        }
    }
}
```

### Rate Limit Errors

```rust
async fn check_rate_limit(user_id: i32) -> Result<(), ApiError> {
    // Implementation
    if is_rate_limited(user_id) {
        return Err(ApiError::BadRequest("Too many requests. Please wait.".into()));
    }
    Ok(())
}
```

## Logging with Tracing

Integrate with the `tracing` crate:

```rust
use tracing::{error, warn, info};

impl CRUDOperations for MyOperations {
    async fn before_delete(
        &self,
        db: &DatabaseConnection,
        id: i32,
    ) -> Result<(), ApiError> {
        let item = Entity::find_by_id(id).one(db).await?;

        match item {
            Some(item) => {
                info!(id = %id, name = %item.name, "Deleting item");
                Ok(())
            },
            None => {
                warn!(id = %id, "Attempted to delete non-existent item");
                Err(ApiError::NotFound)
            }
        }
    }
}
```

## Custom Error Types

Extend ApiError for domain-specific errors:

```rust
#[derive(Debug)]
pub enum DomainError {
    InsufficientBalance(f64),
    ItemOutOfStock(String),
    OrderExpired,
}

impl From<DomainError> for ApiError {
    fn from(err: DomainError) -> Self {
        match err {
            DomainError::InsufficientBalance(amount) => {
                ApiError::BadRequest(format!("Insufficient balance. Need ${:.2} more", amount))
            },
            DomainError::ItemOutOfStock(item) => {
                ApiError::Conflict(format!("Item '{}' is out of stock", item))
            },
            DomainError::OrderExpired => {
                ApiError::BadRequest("Order has expired".into())
            },
        }
    }
}
```

## Global Error Handler

Configure Axum for consistent error handling:

```rust
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            ApiError::NotFound => (
                StatusCode::NOT_FOUND,
                json!({"error": "Not found"})
            ),
            ApiError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                json!({"error": msg})
            ),
            ApiError::ValidationFailed(errors) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                json!({
                    "error": "Validation failed",
                    "details": errors
                })
            ),
            // ... other variants
        };

        (status, Json(body)).into_response()
    }
}
```

## Testing Errors

```rust
#[tokio::test]
async fn test_not_found() {
    let app = create_test_app().await;

    let response = app
        .oneshot(Request::get("/items/999999").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body: Value = parse_body(response).await;
    assert_eq!(body["error"], "Not found");
}

#[tokio::test]
async fn test_validation_error() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::post("/users")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"email": "invalid"}"#))
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
```

## Next Steps

- Implement [Validation](../advanced/validation.md)
- Configure [Security](../advanced/security.md)
- Learn about [Custom Operations](../advanced/custom-operations.md)
