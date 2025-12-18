# Hiding Sensitive Data

{{#test_link exclude}}

Let's add users to our task manager. Users have passwords, but we never want to expose password hashes.

## A User Entity

```rust
#[derive(Clone, Debug, DeriveEntityModel, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable)]
    pub email: String,

    pub name: String,

    #[crudcrate(exclude(one, list))]  // Never return this
    pub password_hash: String,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTime<Utc>,
}
```

## What `exclude(one, list)` Does

| Exclude Target | Effect |
|----------------|--------|
| `one` | Hidden from `GET /users/:id` responses |
| `list` | Hidden from `GET /users` responses |
| `one, list` | Hidden from all GET responses |

## Test It

```bash
# Create a user (password_hash is accepted in POST)
curl -X POST http://localhost:3000/users \
  -H "Content-Type: application/json" \
  -d '{"email": "alice@example.com", "name": "Alice", "password_hash": "hashed123"}'
```

Response - no password_hash:

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "email": "alice@example.com",
  "name": "Alice",
  "created_at": "2024-01-15T10:30:00Z"
}
```

```bash
# List users - no password_hash
curl http://localhost:3000/users

# Get one user - no password_hash
curl http://localhost:3000/users/550e8400-e29b-41d4-a716-446655440000
```

## All Exclude Options

| Attribute | Effect | Test |
|-----------|--------|------|
| `exclude(create)` | Not in POST body | {{#test_link exclude::create}} |
| `exclude(update)` | Not in PUT body | {{#test_link exclude::update}} |
| `exclude(one)` | Not in GET /:id response | {{#test_link exclude::one}} |
| `exclude(list)` | Not in GET / response | {{#test_link exclude::list}} |

You can combine them:

```rust
// Auto-generated, never returned
#[crudcrate(exclude(create, update, one, list), on_create = Uuid::new_v4())]
pub internal_id: Uuid,

// Can create, can't update, hidden from lists
#[crudcrate(exclude(update, list))]
pub secret_code: String,
```

## Excluding from Lists Only

Sometimes you want full data in detail views but not in lists:

```rust
#[crudcrate(exclude(list))]  // Show in GET /:id, hide in GET /
pub full_description: String,
```

This is useful for large fields that you don't need in list views.

---

## Summary

Our entities now have proper data protection:

**Task** - filtering, sorting, search, timestamps
**User** - hidden password_hash

But tasks should belong to users. How do we connect them?

**Next:** [Relationships](./relationships.md) - connect tasks to users.
