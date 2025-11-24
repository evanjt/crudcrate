# CRUDResource API Reference

The `CRUDResource` trait is the core abstraction for CRUD operations.

## Trait Definition

```rust
#[async_trait]
pub trait CRUDResource: Sized + Send + Sync {
    /// The Sea-ORM entity type
    type EntityType: EntityTrait;

    /// Model for creating new records
    type CreateModel: DeserializeOwned + Send + Sync;

    /// Model for updating existing records
    type UpdateModel: DeserializeOwned + Send + Sync;

    /// Model for list responses
    type ListModel: Serialize + Send + Sync;

    /// The primary key type
    type PrimaryKey: Send + Sync;

    /// Get a single record by ID
    async fn get_one(
        db: &DatabaseConnection,
        id: Self::PrimaryKey,
    ) -> Result<Self, ApiError>;

    /// Get all records with filtering, sorting, and pagination
    async fn get_all(
        db: &DatabaseConnection,
        condition: Condition,
        order: (Self::EntityType::Column, Order),
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Self::ListModel>, ApiError>;

    /// Create a new record
    async fn create(
        db: &DatabaseConnection,
        data: Self::CreateModel,
    ) -> Result<Self, ApiError>;

    /// Update an existing record
    async fn update(
        db: &DatabaseConnection,
        id: Self::PrimaryKey,
        data: Self::UpdateModel,
    ) -> Result<Self, ApiError>;

    /// Delete a single record
    async fn delete(
        db: &DatabaseConnection,
        id: Self::PrimaryKey,
    ) -> Result<(), ApiError>;

    /// Delete multiple records
    async fn delete_many(
        db: &DatabaseConnection,
        ids: Vec<Self::PrimaryKey>,
    ) -> Result<u64, ApiError>;

    /// Get total count matching condition
    async fn total_count(
        db: &DatabaseConnection,
        condition: &Condition,
    ) -> u64;
}
```

## Associated Types

### `EntityType`

The Sea-ORM entity for database operations.

```rust
type EntityType = entity::Entity;
```

### `CreateModel`

Request model for creating records. Excludes auto-generated fields.

```rust
type CreateModel = ArticleCreate;

// Generated struct
pub struct ArticleCreate {
    pub title: String,
    pub content: String,
    // id, created_at excluded
}
```

### `UpdateModel`

Request model for updating records. All fields optional.

```rust
type UpdateModel = ArticleUpdate;

// Generated struct
pub struct ArticleUpdate {
    pub title: Option<String>,
    pub content: Option<String>,
}
```

### `ListModel`

Response model for list operations. May exclude expensive fields.

```rust
type ListModel = ArticleList;

// Generated struct
pub struct ArticleList {
    pub id: Uuid,
    pub title: String,
    // content excluded from list
}
```

### `PrimaryKey`

Type of the primary key.

```rust
type PrimaryKey = Uuid;
// or
type PrimaryKey = i32;
```

## Methods

### `get_one`

Retrieve a single record by primary key.

```rust
async fn get_one(
    db: &DatabaseConnection,
    id: Self::PrimaryKey,
) -> Result<Self, ApiError>;
```

**Parameters:**
- `db` - Database connection
- `id` - Primary key value

**Returns:**
- `Ok(Self)` - The record as response model
- `Err(ApiError::NotFound)` - Record doesn't exist
- `Err(ApiError::Database)` - Database error

**Example:**

```rust
let article = Article::get_one(&db, article_id).await?;
```

---

### `get_all`

Retrieve multiple records with filtering, sorting, and pagination.

```rust
async fn get_all(
    db: &DatabaseConnection,
    condition: Condition,
    order: (Self::EntityType::Column, Order),
    offset: u64,
    limit: u64,
) -> Result<Vec<Self::ListModel>, ApiError>;
```

**Parameters:**
- `db` - Database connection
- `condition` - Sea-ORM condition for filtering
- `order` - Tuple of (Column, Order) for sorting
- `offset` - Number of records to skip
- `limit` - Maximum records to return

**Returns:**
- `Ok(Vec<ListModel>)` - List of records
- `Err(ApiError::Database)` - Database error

**Example:**

```rust
use sea_orm::{Condition, Order};

let condition = Condition::all()
    .add(Column::Status.eq("published"));

let articles = Article::get_all(
    &db,
    condition,
    (Column::CreatedAt, Order::Desc),
    0,   // offset
    20,  // limit
).await?;
```

---

### `create`

Create a new record.

```rust
async fn create(
    db: &DatabaseConnection,
    data: Self::CreateModel,
) -> Result<Self, ApiError>;
```

**Parameters:**
- `db` - Database connection
- `data` - Create model with field values

**Returns:**
- `Ok(Self)` - Created record as response model
- `Err(ApiError::Database)` - Database error (e.g., constraint violation)

**Example:**

```rust
let new_article = Article::create(&db, ArticleCreate {
    title: "Hello World".into(),
    content: "My first post".into(),
}).await?;
```

---

### `update`

Update an existing record.

```rust
async fn update(
    db: &DatabaseConnection,
    id: Self::PrimaryKey,
    data: Self::UpdateModel,
) -> Result<Self, ApiError>;
```

**Parameters:**
- `db` - Database connection
- `id` - Primary key of record to update
- `data` - Update model (only Some fields are updated)

**Returns:**
- `Ok(Self)` - Updated record as response model
- `Err(ApiError::NotFound)` - Record doesn't exist
- `Err(ApiError::Database)` - Database error

**Example:**

```rust
let updated = Article::update(&db, article_id, ArticleUpdate {
    title: Some("New Title".into()),
    content: None,  // Don't change
}).await?;
```

---

### `delete`

Delete a single record.

```rust
async fn delete(
    db: &DatabaseConnection,
    id: Self::PrimaryKey,
) -> Result<(), ApiError>;
```

**Parameters:**
- `db` - Database connection
- `id` - Primary key of record to delete

**Returns:**
- `Ok(())` - Record deleted
- `Err(ApiError::NotFound)` - Record doesn't exist
- `Err(ApiError::Database)` - Database error

**Example:**

```rust
Article::delete(&db, article_id).await?;
```

---

### `delete_many`

Delete multiple records by IDs.

```rust
async fn delete_many(
    db: &DatabaseConnection,
    ids: Vec<Self::PrimaryKey>,
) -> Result<u64, ApiError>;
```

**Parameters:**
- `db` - Database connection
- `ids` - List of primary keys (max 100)

**Returns:**
- `Ok(u64)` - Number of records deleted
- `Err(ApiError::BadRequest)` - More than 100 IDs provided
- `Err(ApiError::Database)` - Database error

**Example:**

```rust
let deleted_count = Article::delete_many(&db, vec![id1, id2, id3]).await?;
```

---

### `total_count`

Get count of records matching a condition.

```rust
async fn total_count(
    db: &DatabaseConnection,
    condition: &Condition,
) -> u64;
```

**Parameters:**
- `db` - Database connection
- `condition` - Sea-ORM condition for filtering

**Returns:**
- Count of matching records (0 on error)

**Example:**

```rust
let condition = Condition::all()
    .add(Column::Status.eq("published"));

let count = Article::total_count(&db, &condition).await;
```

## Usage in Custom Handlers

```rust
use crudcrate::CRUDResource;

async fn custom_list_handler(
    Query(params): Query<FilterOptions>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Vec<ArticleList>>, ApiError> {
    let condition = apply_filters::<Entity>(&params)?;
    let (offset, limit) = parse_pagination(&params);
    let order = parse_sorting::<Entity>(&params);

    let articles = Article::get_all(&db, condition, order, offset, limit).await?;

    Ok(Json(articles))
}

async fn custom_create_handler(
    Extension(db): Extension<DatabaseConnection>,
    Json(data): Json<ArticleCreate>,
) -> Result<Json<Article>, ApiError> {
    // Additional validation
    if data.title.is_empty() {
        return Err(ApiError::BadRequest("Title required".into()));
    }

    let article = Article::create(&db, data).await?;

    Ok(Json(article))
}
```

## See Also

- [CRUDOperations API](./crudoperations-api.md)
- [Error Types](./error-types.md)
- [Custom Operations](../advanced/custom-operations.md)
