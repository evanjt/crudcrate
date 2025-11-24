# Field Exclusion

Control which fields appear in which models using the `exclude` attribute.

## Basic Syntax

```rust
#[crudcrate(exclude(target1, target2, ...))]
pub field_name: FieldType,
```

## Exclusion Targets

| Target | Model | HTTP Method |
|--------|-------|-------------|
| `one` | Response | GET `/items/:id` |
| `create` | Create | POST `/items` |
| `update` | Update | PUT `/items/:id` |
| `list` | List | GET `/items` |

## Common Patterns

### Auto-Generated IDs

```rust
// Don't let clients set the ID
#[crudcrate(primary_key, exclude(create, update))]
pub id: Uuid,
```

**Result:**
- ✅ Appears in responses
- ❌ Not in create request model
- ❌ Not in update request model

### Timestamps

```rust
// Managed by the system
#[crudcrate(exclude(create, update), on_create = chrono::Utc::now())]
pub created_at: DateTimeUtc,

#[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
pub updated_at: DateTimeUtc,
```

### Sensitive Data

```rust
// Never expose password
#[crudcrate(exclude(one, list))]
pub password_hash: String,

// Never expose in responses
#[crudcrate(exclude(one, list))]
pub api_secret: String,
```

**Result:**
- ❌ Not in any response
- ✅ Can be set on create
- ✅ Can be updated

### Expensive Fields

```rust
// Large content not needed in lists
#[crudcrate(exclude(list))]
pub full_content: String,

// Computed field expensive to load
#[crudcrate(exclude(list))]
pub statistics: Json,
```

**Result:**
- ✅ In single item response
- ❌ Not in list response
- ✅ Can be set/updated

### Internal Fields

```rust
// Internal tracking, never expose
#[crudcrate(exclude(one, create, update, list))]
pub internal_score: f64,
```

**Result:**
- ❌ Not in any model
- Field only used internally in database

### Read-Only Computed Fields

```rust
// Computed by database trigger
#[crudcrate(exclude(create, update))]
pub view_count: i32,

// Calculated field
#[crudcrate(exclude(create, update))]
pub full_name: String,
```

## Generated Models Comparison

Given this entity:

```rust
pub struct Model {
    #[crudcrate(primary_key, exclude(create, update))]
    pub id: i32,

    #[crudcrate(filterable)]
    pub email: String,

    #[crudcrate(exclude(one, list))]
    pub password_hash: String,

    pub display_name: Option<String>,

    #[crudcrate(exclude(list))]
    pub bio: Option<String>,

    #[crudcrate(exclude(create, update))]
    pub created_at: DateTimeUtc,
}
```

### Response Model (one)

```rust
pub struct User {
    pub id: i32,
    pub email: String,
    // password_hash: excluded
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub created_at: DateTimeUtc,
}
```

### Create Model (create)

```rust
pub struct UserCreate {
    // id: excluded
    pub email: String,
    pub password_hash: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    // created_at: excluded
}
```

### Update Model (update)

```rust
pub struct UserUpdate {
    // id: excluded
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub display_name: Option<Option<String>>,
    pub bio: Option<Option<String>>,
    // created_at: excluded
}
```

### List Model (list)

```rust
pub struct UserList {
    pub id: i32,
    pub email: String,
    // password_hash: excluded
    pub display_name: Option<String>,
    // bio: excluded
    pub created_at: DateTimeUtc,
}
```

## Visual Reference

```
Field            │ Response │ Create │ Update │ List
─────────────────┼──────────┼────────┼────────┼──────
id               │    ✅    │   ❌   │   ❌   │  ✅
email            │    ✅    │   ✅   │   ✅   │  ✅
password_hash    │    ❌    │   ✅   │   ✅   │  ❌
display_name     │    ✅    │   ✅   │   ✅   │  ✅
bio              │    ✅    │   ✅   │   ✅   │  ❌
created_at       │    ✅    │   ❌   │   ❌   │  ✅
```

## Combining with Other Attributes

```rust
// Primary key with auto-generation
#[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
pub id: Uuid,

// Filterable but not in create/update
#[crudcrate(filterable, exclude(create, update), on_create = chrono::Utc::now())]
pub created_at: DateTimeUtc,

// Sortable in lists but not in create
#[crudcrate(sortable, exclude(create))]
pub view_count: i32,
```

## Best Practices

### 1. Always Exclude Auto-Generated Fields

```rust
#[crudcrate(primary_key, exclude(create, update))]
pub id: Uuid,

#[crudcrate(exclude(create, update))]
pub created_at: DateTimeUtc,
```

### 2. Never Expose Secrets

```rust
#[crudcrate(exclude(one, list))]
pub password_hash: String,

#[crudcrate(exclude(one, list))]
pub api_key: String,

#[crudcrate(exclude(one, list))]
pub refresh_token: Option<String>,
```

### 3. Optimize List Responses

```rust
// Large text fields
#[crudcrate(exclude(list))]
pub content: String,

// Expensive computed fields
#[crudcrate(exclude(list))]
pub statistics: Json,

// Relationships (use join instead)
#[sea_orm(ignore)]
#[crudcrate(non_db_attr, join(one))]  // Not in list
pub comments: Vec<Comment>,
```

### 4. Document Your Exclusions

```rust
/// User password hash - never exposed in API responses
#[crudcrate(exclude(one, list))]
pub password_hash: String,

/// Created timestamp - managed by system
#[crudcrate(exclude(create, update), on_create = chrono::Utc::now())]
pub created_at: DateTimeUtc,
```

## Troubleshooting

### Field Not Appearing

Check that:
1. Field is not excluded from that model
2. Field has correct visibility (`pub`)
3. Entity compiles without errors

### Client Sending Excluded Fields

Excluded fields in requests are silently ignored:

```bash
# Trying to set id on create
POST /users
{"id": 999, "email": "test@example.com"}

# Result: id is ignored, auto-generated
{"id": 1, "email": "test@example.com"}
```

### Partial Updates Not Working

For optional fields, remember the double-Option pattern:

```rust
pub struct UserUpdate {
    pub bio: Option<Option<String>>,
    // None = don't change
    // Some(None) = set to null
    // Some(Some(value)) = set to value
}
```

## Next Steps

- Configure [Default Values](./default-values.md)
- Set up [Error Handling](./error-handling.md)
- Learn about [Relationships](./relationships.md)
