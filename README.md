# crudcrate

[![Tests](https://github.com/evanjt/crudcrate/actions/workflows/test.yml/badge.svg)](https://github.com/evanjt/crudcrate/actions/workflows/test.yml)
[![codecov](https://codecov.io/gh/evanjt/crudcrate/branch/main/graph/badge.svg)](https://codecov.io/gh/evanjt/crudcrate)
[![Crates.io](https://img.shields.io/crates/v/crudcrate.svg)](https://crates.io/crates/crudcrate)
[![Documentation](https://docs.rs/crudcrate/badge.svg)](https://docs.rs/crudcrate)

Tired of writing boilerplate for your APIs? Frustrated that your API models look almost identical to your database models, but you have to maintain both? What if you could get a complete CRUD API running in minutes, then customize only the parts that need special handling?

**crudcrate** transforms your Sea-ORM entities into fully-featured REST APIs with one line of code.

```rust
use crudcrate::EntityToModels;

#[derive(EntityToModels)]
#[crudcrate(generate_router)]
pub struct Model {
    #[crudcrate(primary_key, exclude(create, update))]
    pub id: Uuid,
    #[crudcrate(filterable, sortable)]
    pub title: String,
    #[crudcrate(filterable)]
    pub completed: bool,
}

// That's it. You now have:
// - Complete CRUD endpoints (GET, POST, PUT, DELETE)
// - Auto-generated API models (Todo, TodoCreate, TodoUpdate, TodoList)
// - Filtering, sorting, and pagination
// - OpenAPI documentation
```

## The Problem We're Solving

You've been here before:

1. **Write your database model** - `Customer` with id, name, email, created_at
2. **Create API response model** - Basically the same as Customer, but with serde attributes
3. **Create request model for POST** - Same as Customer, but without id and created_at
4. **Create update model for PUT** - Same as POST, but all fields optional
5. **Write 6 HTTP handlers** - get_all, get_one, create, update, delete, delete_many
6. **Wire up routes** - Map each handler to an endpoint
7. **Add filtering logic** - Parse query params, build database conditions
8. **Add pagination** - Calculate offsets, limit results
9. **Add sorting** - Parse sort parameters, apply to queries
10. **Add validation** - Make sure fields are correct types
11. **Add error handling** - Return proper HTTP status codes
12. **Add OpenAPI docs** - Document all endpoints manually

And you repeat this for every single entity in your application.

## Our Solution

Let crudcrate handle the repetitive stuff:

```rust
#[derive(EntityToModels)]
#[crudcrate(generate_router)]
pub struct Customer {
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,
    #[crudcrate(filterable, sortable)]
    pub name: String,
    #[crudcrate(filterable)]
    pub email: String,
    #[crudcrete(exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
}

// Just plug it in:
let app = Router::new()
    .nest("/api/customers", Customer::router(&db));
```

**What you get instantly:**

- `GET /api/customers` - List with filtering, sorting, pagination
- `GET /api/customers/{id}` - Get single customer
- `POST /api/customers` - Create new customer
- `PUT /api/customers/{id}` - Update customer
- `DELETE /api/customers/{id}` - Delete customer
- Auto-generated `Customer`, `CustomerCreate`, `CustomerUpdate`, `CustomerList` models
- Built-in filtering: `?filter={"name_like":"John"}`
- Built-in sorting: `?sort=name&order=DESC` or `?sort=["name","DESC"]`
- Built-in pagination: `?page=1&per_page=20` or `?range=[0,19]` (React Admin)

## But What If I Need Custom Logic?

That's where crudcrate shines. You get the basics for free, but can override anything:

```rust
// Need custom validation or permissions?
#[crudcrate(fn_get_one = custom_get_one)]
pub struct Customer { /* ... */ }

async fn custom_get_one(db: &DatabaseConnection, id: Uuid) -> Result<Customer, DbErr> {
    // Add your custom logic here
    let customer = Entity::find_by_id(id)
        .filter(Column::UserId.eq(current_user_id()))  // Permission check
        .one(db)
        .await?
        .ok_or(DbErr::RecordNotFound("Customer not found"))?;

    // Add logging, caching, audit trails, etc.
    log::info!("Customer {} accessed by user {}", id, current_user_id());

    Ok(customer.into())
}
```

Override any operation: `fn_get_one`, `fn_get_all`, `fn_create`, `fn_update`, `fn_delete`, `fn_delete_many`

## Generated Models

One entity becomes four specialized models:

```rust
#[derive(EntityToModels)]
pub struct Model {
    pub id: Uuid,
    pub title: String,
    pub completed: bool,
    pub secret_data: String,  // Sensitive field
}

// Generated models:

pub struct Todo {           // API responses (get_one)
    pub id: Uuid,
    pub title: String,
    pub completed: bool,
    // secret_data excluded - sensitive info never sent to clients
}

pub struct TodoCreate {     // POST requests (excluded fields omitted)
    pub title: String,
    pub completed: bool,
    // id and secret_data excluded automatically
}

pub struct TodoUpdate {     // PUT requests (all fields optional)
    pub title: Option<String>,
    pub completed: Option<bool>,
    // id excluded, secret_data excluded unless you override
}

pub struct TodoList {       // List responses (can exclude expensive fields)
    pub id: Uuid,
    pub title: String,
    pub completed: bool,
    // secret_data excluded to avoid leaking sensitive info in lists
}
```

## Real-World Features You'll Actually Use

### Smart Filtering

```rust
#[crudcrate(filterable, sortable, fulltext)]
pub title: String,
#[crudcrate(filterable)]
pub priority: i32,
```

Your users can now:

```bash
# Exact matches
GET /api/tasks?filter={"completed":false,"priority":3}

# Numeric ranges
GET /api/tasks?filter={"priority_gte":2,"priority_lte":5}

# Text search across all searchable fields
GET /api/tasks?filter={"q":"urgent review"}

# Combine filters
GET /api/tasks?filter={"completed":false,"priority_gte":3,"q":"urgent"}
```

### Relationship Loading

Automatically load related data in API responses with full recursive support:

```rust
pub struct Customer {
    pub id: Uuid,
    pub name: String,

    // Automatically load related vehicles in API responses
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all))]
    pub vehicles: Vec<Vehicle>,
}

pub struct Vehicle {
    pub id: Uuid,
    pub make: String,

    // Each vehicle automatically loads its parts and maintenance records
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all))]
    pub parts: Vec<VehiclePart>,

    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all))]
    pub maintenance_records: Vec<MaintenanceRecord>,
}
```

**Multi-level recursive loading works out of the box:**
- Customer → Vehicles → Parts/Maintenance Records (3 levels deep)
- No complex SQL joins required - uses efficient recursive queries
- Automatic cycle detection prevents infinite recursion

**Join options:**
- `join(one)` - Load only in individual item responses
- `join(all)` - Load only in list responses
- `join(one, all)` - Load in both types of responses
- `join(one, all, depth = 2)` - Custom depth guidance (default: unlimited)

### Field Control

Sometimes certain fields shouldn't be in certain models:

```rust
// Password hash: never send to clients, never allow updates
#[crudcrate(exclude(one, create, update, list))]
pub password_hash: String,

// API keys: generate server-side, never expose in any response
#[crudcrate(exclude(one, create, update, list), on_create = generate_api_key())]
pub api_key: String,

// Internal notes: exclude from list (expensive) but show in detail view
#[crudcrate(exclude(list))]
pub internal_notes: String,

// Timestamps: manage automatically
#[crudcrate(exclude(create, update), on_create = Utc::now(), on_update = Utc::now())]
pub updated_at: DateTime<Utc>,
```

**Exclusion options:**
- `exclude(one)` - Exclude from get_one responses (main API response)
- `exclude(create)` - Exclude from POST request models
- `exclude(update)` - Exclude from PUT request models
- `exclude(list)` - Exclude from list responses
- `exclude(one, list)` - Exclude from both individual and list responses
- `exclude(create, update)` - Exclude from both request models

## Production Ready

crudcrate isn't just a toy - it's built for real applications:

### Database Optimizations

```rust
// Get performance recommendations for production
crudcrate::analyse_all_registered_models(&db, false).await;
```

Output:

```
HIGH Priority:
  customers - Fulltext search on name/email without proper index
    CREATE INDEX idx_customers_fulltext ON customers USING GIN (to_tsvector('english', name || ' ' || email));

MEDIUM Priority:
  customers - Field 'email' is filterable but not indexed
    CREATE INDEX idx_customers_email ON customers (email);
```

### Multi-Database Support

- **PostgreSQL**: Full GIN index support, tsvector optimization
- **MySQL**: FULLTEXT indexes, MATCH AGAINST queries
- **SQLite**: LIKE-based fallback (perfect for development)

### Battle-Tested Features

- SQL injection prevention via Sea-ORM parameterization
- Input validation and sanitization
- Type-safe compile-time checks
- Comprehensive test suite across all supported databases

## Quick Start

```bash
cargo add crudcrate sea-orm axum
```

```rust
use axum::Router;
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;

#[derive(EntityToModels)]
#[crudcrate(generate_router)]
pub struct Task {
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,
    #[crudcrate(sortable, filterable)]
    pub title: String,
    #[crudcrate(filterable)]
    pub completed: bool,
}

#[tokio::main]
async fn main() {
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();

    let app = Router::new()
        .nest("/api/tasks", Task::router(&db));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("🚀 API running on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}
```

That's it. You have a complete, production-ready CRUD API.

Run it:

```bash
cargo run
```

Test it:

```bash
# Create a task
curl -X POST http://localhost:3000/api/tasks \
  -H "Content-Type: application/json" \
  -d '{"title":"Build CRUD API","completed":false}'

# List all tasks
curl http://localhost:3000/api/tasks

# Get a specific task
curl http://localhost:3000/api/tasks/{id}

# Update a task
curl -X PUT http://localhost:3000/api/tasks/{id} \
  -H "Content-Type: application/json" \
  -d '{"completed":true}'
```

## When to Use crudcrate

**Perfect for:**

- Quick prototypes and MVPs
- Admin panels and internal tools
- Standard CRUD operations
- APIs that follow REST conventions
- Teams that want to move fast

**Maybe not for:**

- Highly specialized endpoints
- GraphQL APIs (though you could use the generated models)
- Complex business logic that doesn't fit CRUD patterns
- When you need full control over every detail

## Examples

```bash
# Minimal todo API
cargo run --example minimal

# Relationship loading demo
cargo run --example recursive_join
```

## License

MIT License. See [LICENSE](./LICENSE) for details.
