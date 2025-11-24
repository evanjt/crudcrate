# Validation

Validate input data before it reaches your database.

## ValidationError Type

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

## Basic Validation

### In Hooks

```rust
#[crudcrate(create::one::pre = validate_user)]

async fn validate_user(
    _db: &DatabaseConnection,
    data: &mut UserCreate,
) -> Result<(), ApiError> {
    let mut errors = Vec::new();

    // Required field
    if data.name.trim().is_empty() {
        errors.push(ValidationError::new("name", "Name is required"));
    }

    // Email format
    if !data.email.contains('@') {
        errors.push(ValidationError::new("email", "Invalid email format"));
    }

    // Length constraints
    if data.password.len() < 8 {
        errors.push(ValidationError::new("password", "Must be at least 8 characters"));
    }

    if data.password.len() > 128 {
        errors.push(ValidationError::new("password", "Must be at most 128 characters"));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::ValidationFailed(errors))
    }
}
```

### In CRUDOperations

```rust
impl CRUDOperations for UserOperations {
    type Resource = User;

    async fn before_create(
        &self,
        db: &DatabaseConnection,
        data: &mut UserCreate,
    ) -> Result<(), ApiError> {
        validate_user_data(data)?;
        check_email_unique(db, &data.email).await?;
        Ok(())
    }
}

fn validate_user_data(data: &UserCreate) -> Result<(), ApiError> {
    let mut errors = Vec::new();

    if data.name.is_empty() {
        errors.push(ValidationError::new("name", "Required"));
    }

    if !errors.is_empty() {
        return Err(ApiError::ValidationFailed(errors));
    }

    Ok(())
}
```

## Common Validators

### Required Fields

```rust
fn required(value: &str, field: &str) -> Option<ValidationError> {
    if value.trim().is_empty() {
        Some(ValidationError::new(field, "This field is required"))
    } else {
        None
    }
}
```

### String Length

```rust
fn min_length(value: &str, min: usize, field: &str) -> Option<ValidationError> {
    if value.len() < min {
        Some(ValidationError::new(field, &format!("Must be at least {} characters", min)))
    } else {
        None
    }
}

fn max_length(value: &str, max: usize, field: &str) -> Option<ValidationError> {
    if value.len() > max {
        Some(ValidationError::new(field, &format!("Must be at most {} characters", max)))
    } else {
        None
    }
}
```

### Email Format

```rust
fn valid_email(email: &str, field: &str) -> Option<ValidationError> {
    // Simple validation
    if !email.contains('@') || !email.contains('.') {
        return Some(ValidationError::new(field, "Invalid email format"));
    }

    // Or use regex for more thorough validation
    let email_regex = regex::Regex::new(
        r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$"
    ).unwrap();

    if !email_regex.is_match(email) {
        Some(ValidationError::new(field, "Invalid email format"))
    } else {
        None
    }
}
```

### Numeric Range

```rust
fn in_range<T: PartialOrd + std::fmt::Display>(
    value: T,
    min: T,
    max: T,
    field: &str,
) -> Option<ValidationError> {
    if value < min || value > max {
        Some(ValidationError::new(
            field,
            &format!("Must be between {} and {}", min, max)
        ))
    } else {
        None
    }
}
```

### Pattern Matching

```rust
fn matches_pattern(value: &str, pattern: &str, field: &str, message: &str) -> Option<ValidationError> {
    let regex = regex::Regex::new(pattern).unwrap();
    if !regex.is_match(value) {
        Some(ValidationError::new(field, message))
    } else {
        None
    }
}

// Usage
fn validate_username(username: &str) -> Option<ValidationError> {
    matches_pattern(
        username,
        r"^[a-zA-Z0-9_]{3,20}$",
        "username",
        "Username must be 3-20 characters, letters, numbers, and underscores only"
    )
}
```

## Database Validation

### Uniqueness Check

```rust
async fn check_unique_email(
    db: &DatabaseConnection,
    email: &str,
    exclude_id: Option<Uuid>,
) -> Result<(), ApiError> {
    let mut query = Entity::find().filter(Column::Email.eq(email));

    // Exclude current record during update
    if let Some(id) = exclude_id {
        query = query.filter(Column::Id.ne(id));
    }

    let exists = query.count(db).await.map_err(|_| ApiError::Internal("Database error".into()))?;

    if exists > 0 {
        Err(ApiError::ValidationFailed(vec![
            ValidationError::new("email", "Email already in use")
        ]))
    } else {
        Ok(())
    }
}
```

### Foreign Key Existence

```rust
async fn check_category_exists(
    db: &DatabaseConnection,
    category_id: i32,
) -> Result<(), ApiError> {
    let exists = category::Entity::find_by_id(category_id)
        .count(db)
        .await
        .map_err(|_| ApiError::Internal("Database error".into()))?;

    if exists == 0 {
        Err(ApiError::ValidationFailed(vec![
            ValidationError::new("category_id", "Category does not exist")
        ]))
    } else {
        Ok(())
    }
}
```

## Validator Builder Pattern

Create a fluent validation API:

```rust
pub struct Validator {
    errors: Vec<ValidationError>,
}

impl Validator {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    pub fn required(mut self, value: &str, field: &str) -> Self {
        if value.trim().is_empty() {
            self.errors.push(ValidationError::new(field, "Required"));
        }
        self
    }

    pub fn min_length(mut self, value: &str, min: usize, field: &str) -> Self {
        if value.len() < min {
            self.errors.push(ValidationError::new(
                field,
                &format!("Must be at least {} characters", min)
            ));
        }
        self
    }

    pub fn max_length(mut self, value: &str, max: usize, field: &str) -> Self {
        if value.len() > max {
            self.errors.push(ValidationError::new(
                field,
                &format!("Must be at most {} characters", max)
            ));
        }
        self
    }

    pub fn email(mut self, value: &str, field: &str) -> Self {
        if !value.contains('@') {
            self.errors.push(ValidationError::new(field, "Invalid email"));
        }
        self
    }

    pub fn validate(self) -> Result<(), ApiError> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(ApiError::ValidationFailed(self.errors))
        }
    }
}

// Usage
fn validate_user(data: &UserCreate) -> Result<(), ApiError> {
    Validator::new()
        .required(&data.name, "name")
        .min_length(&data.name, 2, "name")
        .max_length(&data.name, 100, "name")
        .required(&data.email, "email")
        .email(&data.email, "email")
        .min_length(&data.password, 8, "password")
        .validate()
}
```

## Using the `validator` Crate

Integrate with the popular `validator` crate:

```toml
[dependencies]
validator = { version = "0.18", features = ["derive"] }
```

```rust
use validator::{Validate, ValidationErrors};

#[derive(Validate, Deserialize)]
pub struct UserCreate {
    #[validate(length(min = 2, max = 100))]
    pub name: String,

    #[validate(email)]
    pub email: String,

    #[validate(length(min = 8))]
    pub password: String,

    #[validate(range(min = 18, max = 150))]
    pub age: Option<i32>,

    #[validate(url)]
    pub website: Option<String>,
}

// Convert validator errors to CRUDCrate errors
fn to_api_errors(errors: ValidationErrors) -> ApiError {
    let validation_errors: Vec<ValidationError> = errors
        .field_errors()
        .iter()
        .flat_map(|(field, errs)| {
            errs.iter().map(|e| {
                ValidationError::new(
                    field,
                    e.message.as_ref().map(|m| m.to_string()).unwrap_or_else(|| "Invalid".to_string()).as_str()
                )
            })
        })
        .collect();

    ApiError::ValidationFailed(validation_errors)
}

// In hook
async fn validate_user(
    _db: &DatabaseConnection,
    data: &mut UserCreate,
) -> Result<(), ApiError> {
    data.validate().map_err(to_api_errors)?;
    Ok(())
}
```

## Conditional Validation

```rust
fn validate_article(data: &ArticleCreate) -> Result<(), ApiError> {
    let mut errors = Vec::new();

    // Always required
    if data.title.is_empty() {
        errors.push(ValidationError::new("title", "Title is required"));
    }

    // Required only if publishing
    if data.status == ArticleStatus::Published {
        if data.content.is_empty() {
            errors.push(ValidationError::new("content", "Content required for published articles"));
        }

        if data.excerpt.is_none() {
            errors.push(ValidationError::new("excerpt", "Excerpt required for published articles"));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::ValidationFailed(errors))
    }
}
```

## Async Validation

```rust
async fn validate_article_async(
    db: &DatabaseConnection,
    data: &ArticleCreate,
) -> Result<(), ApiError> {
    let mut errors = Vec::new();

    // Sync validations
    if data.title.is_empty() {
        errors.push(ValidationError::new("title", "Required"));
    }

    // Async validations
    if !errors.is_empty() {
        // Skip async checks if basic validation fails
        return Err(ApiError::ValidationFailed(errors));
    }

    // Check slug uniqueness
    let slug = slugify(&data.title);
    if slug_exists(db, &slug).await? {
        errors.push(ValidationError::new("title", "An article with this title already exists"));
    }

    // Check category exists
    if let Some(cat_id) = data.category_id {
        if !category_exists(db, cat_id).await? {
            errors.push(ValidationError::new("category_id", "Category not found"));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::ValidationFailed(errors))
    }
}
```

## Response Format

Validation errors return HTTP 422:

```json
{
  "error": "Validation failed",
  "details": [
    {"field": "email", "message": "Invalid email format"},
    {"field": "password", "message": "Must be at least 8 characters"},
    {"field": "age", "message": "Must be between 18 and 150"}
  ]
}
```

## Next Steps

- Configure [Security](./security.md)
- Set up [Performance Optimization](./performance.md)
- Learn about [Multi-Database Support](./multi-database.md)
