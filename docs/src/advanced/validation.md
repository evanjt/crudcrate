# Validation

Validate input data before it reaches your database.

The primary mechanism is the `Validatable` trait. The generated CRUD handlers
call it automatically on every create and update, including the batch variants.
Hook-based validation remains available for checks that need database access.

## The `Validatable` Trait

```rust
pub trait Validatable {
    fn validate(&self) -> Result<(), ValidationError>;
}
```

Implement it on your Create or Update model. The generated
`create`/`create_many`/`update`/`update_many` implementations invoke
`validate()` before any database write. A failure maps to HTTP 422.

No registration is needed. The generated code probes for the impl via autoref
specialisation (`crudcrate::validation::__auto`), so models without a
`Validatable` impl get a no-op and are unaffected.

### Implementing for Create Models

```rust
use crudcrate::validation::{Validatable, ValidationError};

fn name_is_valid(name: &str) -> Result<(), ValidationError> {
    if name.trim().is_empty() {
        return Err(ValidationError::new("name", "Name is required"));
    }
    if name.chars().count() < 3 {
        return Err(ValidationError::new("name", "Name must be at least 3 characters"));
    }
    Ok(())
}

impl Validatable for ProductCreate {
    fn validate(&self) -> Result<(), ValidationError> {
        name_is_valid(&self.name)?;
        if self.price <= 0 {
            return Err(ValidationError::new("price", "Price must be positive"));
        }
        Ok(())
    }
}
```

### Implementing for Update Models

Update models double-wrap every column as `Option<Option<T>>`. The outer
`None` means the field was absent from the request. The inner `None` means
"set to NULL". Validate a value only when it is actually present:

```rust
impl Validatable for ProductUpdate {
    fn validate(&self) -> Result<(), ValidationError> {
        if let Some(Some(name)) = &self.name {
            name_is_valid(name)?;
        }
        Ok(())
    }
}
```

### Response Format

A failed `validate()` returns HTTP 422 with the field and message joined as
one string per error:

```json
{
  "error": "Validation failed",
  "details": ["name: Name must be at least 3 characters"]
}
```

## Built-in Validators

The `crudcrate::validation::validators` module ships helpers for common
patterns. Length limits count characters, not UTF-8 bytes, so multibyte
values within the character limit are not rejected.

```rust
use crudcrate::validation::validators;

impl Validatable for UserCreate {
    fn validate(&self) -> Result<(), ValidationError> {
        validators::validate_required("name", &self.name)?;
        validators::validate_length("name", &self.name, Some(2), Some(100))?;
        validators::validate_email("email", &self.email)?;
        validators::validate_range("age", self.age, Some(18), Some(150))?;
        Ok(())
    }
}
```

## Hook-Based Validation

`Validatable` is synchronous and sees only the payload. For checks that need
the database, or to report several errors at once, use a lifecycle hook and
construct the error with `ApiError::validation_failed`, which takes a
`Vec<String>`:

```rust
#[crudcrate(create::one::pre = validate_user)]

async fn validate_user(
    db: &DatabaseConnection,
    data: &mut UserCreate,
) -> Result<(), ApiError> {
    let mut errors = Vec::new();

    if data.name.trim().is_empty() {
        errors.push("name: Name is required".to_string());
    }
    if !data.email.contains('@') {
        errors.push("email: Invalid email format".to_string());
    }

    if !errors.is_empty() {
        return Err(ApiError::validation_failed(errors));
    }

    check_email_unique(db, &data.email).await?;
    Ok(())
}
```

The same construction works inside `CRUDOperations::before_create` and
`before_update`.

## Database Validation

### Uniqueness Check

```rust
async fn check_email_unique(
    db: &DatabaseConnection,
    email: &str,
) -> Result<(), ApiError> {
    let exists = Entity::find()
        .filter(Column::Email.eq(email))
        .count(db)
        .await?;

    if exists > 0 {
        return Err(ApiError::validation_failed(vec![
            "email: Email already in use".to_string(),
        ]));
    }
    Ok(())
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
        .await?;

    if exists == 0 {
        return Err(ApiError::validation_failed(vec![
            "category_id: Category does not exist".to_string(),
        ]));
    }
    Ok(())
}
```

## Using the `validator` Crate

The `validator` crate's derive works alongside `Validatable`. Convert its
errors in your impl:

```rust
use validator::Validate;

#[derive(Validate, Deserialize)]
pub struct UserCreate {
    #[validate(length(min = 2, max = 100))]
    pub name: String,

    #[validate(email)]
    pub email: String,
}

impl Validatable for UserCreate {
    fn validate(&self) -> Result<(), crudcrate::validation::ValidationError> {
        Validate::validate(self).map_err(|errors| {
            let first = errors
                .field_errors()
                .into_iter()
                .next()
                .map(|(field, errs)| {
                    let message = errs
                        .first()
                        .and_then(|e| e.message.as_ref())
                        .map_or_else(|| "Invalid".to_string(), ToString::to_string);
                    (field.to_string(), message)
                })
                .unwrap_or_else(|| ("input".to_string(), "Invalid".to_string()));
            crudcrate::validation::ValidationError::new(first.0, first.1)
        })
    }
}
```

For reporting every error at once, run the `validator` derive from a hook
instead and collect into `ApiError::validation_failed`.

## Next Steps

- Configure [Security](./security.md)
- Set up [Performance Optimization](./performance.md)
- Learn about [Multi-Database Support](./multi-database.md)
