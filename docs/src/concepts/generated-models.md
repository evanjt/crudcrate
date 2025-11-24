# Generated Models

CRUDCrate generates four model types from your entity. Each serves a specific purpose in your API.

## Overview

From this entity:

```rust
#[derive(EntityToModels)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[crudcrate(primary_key, exclude(create, update))]
    pub id: i32,

    #[crudcrate(filterable)]
    pub email: String,

    #[crudcrate(exclude(one, list))]
    pub password_hash: String,

    pub display_name: Option<String>,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTimeUtc,
}
```

CRUDCrate generates:

| Model | Purpose | Used In |
|-------|---------|---------|
| `User` | Full response | `GET /users/:id` |
| `UserCreate` | Create request | `POST /users` |
| `UserUpdate` | Update request | `PUT /users/:id` |
| `UserList` | List response | `GET /users` |

## Response Model (User)

The main response model, returned from `get_one`:

```rust
// Generated
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: i32,
    pub email: String,
    // password_hash excluded via exclude(one)
    pub display_name: Option<String>,
    pub created_at: DateTimeUtc,
}
```

**Characteristics:**
- Includes all fields except those with `exclude(one)`
- Used for single-item responses
- Can include loaded relationships

## Create Model (UserCreate)

Used for `POST` requests:

```rust
// Generated
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserCreate {
    // id excluded via exclude(create)
    pub email: String,
    pub password_hash: String,  // NOT excluded from create
    pub display_name: Option<String>,
    // created_at excluded via exclude(create)
}
```

**Characteristics:**
- Excludes primary keys (usually auto-generated)
- Excludes timestamp fields
- Includes fields the client should provide

## Update Model (UserUpdate)

Used for `PUT` requests:

```rust
// Generated
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserUpdate {
    // id excluded via exclude(update)
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub display_name: Option<Option<String>>,  // Double Option for nullable fields
    // created_at excluded via exclude(update)
}
```

**Characteristics:**
- All fields are `Option<T>` for partial updates
- Nullable fields become `Option<Option<T>>`
- Only provided fields are updated

### Double Option Explained

For nullable database fields:

```rust
// In entity
pub bio: Option<String>,

// In UserUpdate
pub bio: Option<Option<String>>,

// Usage:
// None = don't change
// Some(None) = set to NULL
// Some(Some("text")) = set to "text"
```

```json
// Don't change bio
{}

// Set bio to null
{"bio": null}

// Set bio to value
{"bio": "Hello world"}
```

## List Model (UserList)

Used for `GET` collection responses:

```rust
// Generated
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserList {
    pub id: i32,
    pub email: String,
    // password_hash excluded via exclude(list)
    pub display_name: Option<String>,
    pub created_at: DateTimeUtc,
}
```

**Characteristics:**
- Often identical to Response model
- Can exclude expensive fields (e.g., large text, relationships)
- Optimized for list performance

## Field Exclusion Matrix

Control which fields appear in which models:

| Field | `exclude(one)` | `exclude(create)` | `exclude(update)` | `exclude(list)` |
|-------|----------------|-------------------|-------------------|-----------------|
| `id` | In response | **Not in create** | **Not in update** | In list |
| `email` | In response | In create | In update | In list |
| `password` | **Not in response** | In create | In update | **Not in list** |
| `created_at` | In response | **Not in create** | **Not in update** | In list |

Example:

```rust
// Never expose password
#[crudcrate(exclude(one, list))]
pub password_hash: String,

// Client can't set ID or timestamps
#[crudcrate(primary_key, exclude(create, update))]
pub id: Uuid,

#[crudcrate(exclude(create, update))]
pub created_at: DateTimeUtc,

// Exclude expensive content from lists
#[crudcrate(exclude(list))]
pub full_content: String,
```

## Model Traits

All generated models implement:

```rust
// Serialization
impl Serialize for User { }
impl Deserialize for User { }

// Cloning
impl Clone for User { }

// Debug output
impl Debug for User { }
```

Additionally:

```rust
// Response model implements From<Sea-ORM Model>
impl From<Model> for User { }

// Create model implements Into<ActiveModel>
impl From<UserCreate> for ActiveModel { }

// Update model implements merge
impl MergeIntoActiveModel<ActiveModel> for UserUpdate { }
```

## Conversions

### Entity to Response

```rust
let db_model: Model = Entity::find_by_id(1).one(db).await?;
let response: User = db_model.into();
```

### Create to ActiveModel

```rust
let create_data: UserCreate = /* from request */;
let active_model: ActiveModel = create_data.into();
let result = active_model.insert(db).await?;
```

### Update Merge

```rust
let update_data: UserUpdate = /* from request */;
let mut active_model: ActiveModel = existing.into();

// Only updates fields that were provided
update_data.merge_into(&mut active_model);

let result = active_model.update(db).await?;
```

## Customization

### Custom Model Names

```rust
#[crudcrate(api_struct = "Item")]
pub struct Model { }

// Generates: Item, ItemCreate, ItemUpdate, ItemList
```

### Additional Derives

The generated models use standard derives. For additional traits, implement them manually:

```rust
// In your code
impl Default for UserCreate {
    fn default() -> Self {
        Self {
            email: String::new(),
            password_hash: String::new(),
            display_name: None,
        }
    }
}
```

### Validation

Add validation in `CRUDOperations`:

```rust
impl CRUDOperations for UserOps {
    async fn before_create(&self, data: &mut UserCreate) -> Result<(), ApiError> {
        if !data.email.contains('@') {
            return Err(ApiError::ValidationFailed(vec![
                ValidationError::new("email", "Invalid email format")
            ]));
        }
        Ok(())
    }
}
```

## Relationships in Models

Join fields appear in response models:

```rust
// Entity definition
#[sea_orm(ignore)]
#[crudcrate(non_db_attr, join(one, all))]
pub posts: Vec<Post>,

// Generated in User response model
pub struct User {
    pub id: i32,
    pub email: String,
    pub posts: Vec<Post>,  // Loaded automatically
}
```

Control when relationships load:
- `join(one)` - Load in `get_one` only
- `join(all)` - Load in `get_all` too (can be expensive)
- `join(one, all)` - Load in both

## Next Steps

- Understand the [CRUDResource Trait](./crudresource-trait.md)
- Learn about [Field Exclusion](../features/field-exclusion.md)
- Configure [Relationships](../features/relationships.md)
