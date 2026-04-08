# Public & Private Endpoints

Our task manager has users, tasks, and relationships. Now let's add a public API that anyone can access without authentication — while keeping sensitive records hidden.

## The Problem

You want **one set of routes** that serves both:

- **Admins** (authenticated) — see everything, full CRUD
- **Public** (unauthenticated) — read-only, private records hidden

crudcrate's **scoping** system handles this with two features:

1. `ScopeCondition` — a middleware-injected filter that restricts which rows are returned
2. `exclude(scoped)` — hides fields from the response when a scope is active

## Adding a Privacy Field

Add an `is_private` field to your entity:

```rust
#[derive(Clone, Debug, DeriveEntityModel, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub title: String,

    pub description: Option<String>,

    #[crudcrate(filterable, exclude(scoped))]
    pub is_private: bool,
}
```

`exclude(scoped)` does two things:

1. **Hides the field** from API responses when a scope is active — public users never see `is_private` in the JSON
2. **Strips the field from filters/sorting** — public users can't probe it via `?filter={"is_private":true}`

## Writing Scope Middleware

A scope is just Axum middleware that injects a `ScopeCondition` into the request.
You decide **when** to inject it — typically when the user isn't authenticated:

```rust
use axum::{extract::Request, middleware::Next, response::Response};
use crudcrate::ScopeCondition;
use sea_orm::{ColumnTrait, Condition};

async fn scope_tasks(mut req: Request, next: Next) -> Response {
    if !is_admin(&req) {
        // Only show public tasks
        req.extensions_mut().insert(ScopeCondition::new(
            Condition::all().add(task::Column::IsPrivate.eq(false)),
        ));
    }
    next.run(req).await
}
```

When `ScopeCondition` is present, crudcrate automatically:

- Filters list queries (`GET /`) to only return matching rows
- Returns 404 for `GET /:id` if the record doesn't pass the condition
- **Blocks all writes** (POST, PUT, DELETE) with 403 Forbidden
- Uses the scoped response model (without `exclude(scoped)` fields)
- Returns correct pagination counts reflecting the filtered total

When `ScopeCondition` is **not** present (admin requests), everything works normally — full CRUD, all fields visible.

## Mounting the Routes

Apply the scope middleware to your router. Layer it **after** your auth middleware so the auth status is available:

```rust
use axum::{middleware::from_fn, Router};

let app = Router::new()
    .nest(
        "/api/tasks",
        Task::router(&db)
            .layer(from_fn(scope_tasks))      // Check scope based on auth
            .layer(keycloak_pass_layer)        // Auth (passthrough mode)
            .into(),
    );
```

## What Happens

**Public user** (no auth token):

```bash
# List — only public tasks, no is_private in response
curl http://localhost:3000/api/tasks
# [{"id": "...", "title": "Public task", "description": "..."}]

# Private task — 404
curl http://localhost:3000/api/tasks/private-uuid
# {"error": "task not found"}

# Write — blocked
curl -X POST http://localhost:3000/api/tasks -d '{"title": "hack"}'
# 403 Forbidden

# Filter on is_private — silently ignored
curl 'http://localhost:3000/api/tasks?filter={"is_private":true}'
# Returns same results as without filter
```

**Admin** (valid auth token):

```bash
# List — all tasks, is_private visible
curl -H "Authorization: Bearer TOKEN" http://localhost:3000/api/tasks
# [{"id": "...", "title": "Public task", "is_private": false},
#  {"id": "...", "title": "Secret task", "is_private": true}]

# Full CRUD works
curl -X POST -H "Authorization: Bearer TOKEN" \
  http://localhost:3000/api/tasks \
  -d '{"title": "New task", "is_private": true}'
# 201 Created
```

## Scoping with Relationships

If your entities have parent-child relationships, you probably want privacy to cascade. For example, if an **area** is private, all **sites** in that area should be hidden too.

Add `Expr::cust()` subqueries to your scope condition:

```rust
async fn scope_sites(mut req: Request, next: Next) -> Response {
    if !is_admin(&req) {
        req.extensions_mut().insert(ScopeCondition::new(
            Condition::all()
                .add(site::Column::IsPrivate.eq(false))
                .add(Expr::cust(
                    "(area_id IS NULL OR area_id NOT IN \
                     (SELECT id FROM areas WHERE is_private = true))"
                )),
        ));
    }
    next.run(req).await
}
```

> **Warning**: `Expr::cust()` passes raw SQL directly to the database. Never interpolate user input into the string — this creates SQL injection vulnerabilities. Use only static strings or Sea-ORM's typed column API for dynamic conditions.

## Scoping with Joins

If your entity has `join()` fields (nested children in the response), `exclude(scoped)` propagates automatically through joins.

```rust
// Parent: Customer
#[crudcrate(filterable, exclude(scoped))]
pub is_private: bool,

#[crudcrate(non_db_attr, join(one, all))]
pub vehicles: Vec<Vehicle>,

// Child: Vehicle
#[crudcrate(filterable, exclude(scoped))]
pub is_private: bool,
```

When a scoped request fetches a customer, the response looks like:

```json
{
  "id": "...",
  "name": "Alice",
  "vehicles": [
    {"id": "...", "make": "Toyota", "model": "Corolla", "year": 2020}
  ]
}
```

No `is_private` on the customer **or** on any nested vehicle. crudcrate generates `CustomerScopedList` with `vehicles: Vec<VehicleScopedList>` — the scoped types cascade through every join level.

### How join scoping works

Both field stripping and row filtering happen automatically for joined children:

1. **Field stripping**: The scoped types (`VehicleScopedList`) omit `exclude(scoped)` fields from the JSON
2. **Row filtering**: During the `From<ListModel> for ScopedList` conversion, children are filtered via `ScopeFilterable::is_scope_visible()` — any child with `is_private: true` is removed from the response

> **Important**: Row filtering happens at the application level (in Rust, during type conversion), not at the SQL level. The database query loads all children; private ones are stripped during serialization. For entities with very large child sets, consider using SQL-level subqueries in your `ScopeCondition` instead (see "Scoping with Relationships" above).

For this automatic filtering to work, the child entity **must** have at least one `exclude(scoped)` boolean field. If it doesn't, all children pass through regardless of their data.

## Quick Reference

| Attribute | Effect |
|-----------|--------|
| `exclude(scoped)` | Field hidden from response when scoped |
| `ScopeCondition::new(condition)` | Filter rows in list/get_one |
| Scope + write request | Automatically returns 403 Forbidden |
| Scope + filter on excluded column | Filter silently ignored |
| Scope + join fields | Child entities use scoped types too |

## Entities Without `exclude(scoped)`

If a child entity doesn't use `exclude(scoped)`, crudcrate generates a type alias (`type ChildScopedList = ChildList`) so parent joins still compile. No action needed — it just works.

---

**Next:** [Custom Logic - Hooks](./hooks.md) - add validation and side effects to your endpoints.
