# Field Attributes Reference

Complete reference for `#[crudcrate(...)]` attributes on fields.

## Syntax

```rust
#[crudcrate(attribute1, attribute2 = value, ...)]
pub field_name: FieldType,
```

## Core Attributes

### `primary_key`

Marks the field as the primary key.

```rust
#[crudcrate(primary_key)]
pub id: i32,
```

**Required:** Yes (exactly one field)
**Type:** Flag

---

### `exclude(...)`

Exclude field from specific generated models.

```rust
#[crudcrate(exclude(create, update))]
pub id: Uuid,

#[crudcrate(exclude(one, list))]
pub password_hash: String,
```

**Type:** List of targets
**Targets:**
- `one` - Response model (GET /items/:id)
- `create` - Create model (POST /items)
- `update` - Update model (PUT /items/:id)
- `list` - List model (GET /items)

---

### `filterable`

Enable filtering on this field.

```rust
#[crudcrate(filterable)]
pub status: String,
```

**Type:** Flag
**Effect:** Allows `?filter={"status":"value"}` and `?status_eq=value`

---

### `sortable`

Enable sorting on this field.

```rust
#[crudcrate(sortable)]
pub created_at: DateTimeUtc,
```

**Type:** Flag
**Effect:** Allows `?sort=["created_at","DESC"]`

---

### `fulltext`

Include field in fulltext search.

```rust
#[crudcrate(fulltext)]
pub title: String,

#[crudcrate(fulltext)]
pub content: String,
```

**Type:** Flag
**Effect:** Field included when using `?q=search+terms`

---

## Default Value Attributes

### `on_create`

Set default value when creating new records.

```rust
#[crudcrate(on_create = Uuid::new_v4())]
pub id: Uuid,

#[crudcrate(on_create = chrono::Utc::now())]
pub created_at: DateTimeUtc,

#[crudcrate(on_create = "pending".to_string())]
pub status: String,

#[crudcrate(on_create = 0)]
pub view_count: i32,
```

**Type:** Rust expression
**When:** Evaluated during `create` operation

---

### `on_update`

Set default value when updating records.

```rust
#[crudcrate(on_update = chrono::Utc::now())]
pub updated_at: DateTimeUtc,
```

**Type:** Rust expression
**When:** Evaluated during every `update` operation

---

## Relationship Attributes

### `non_db_attr`

Marks field as non-database (for computed or relationship fields).

```rust
#[sea_orm(ignore)]
#[crudcrate(non_db_attr)]
pub comments: Vec<Comment>,
```

**Type:** Flag
**Required:** Yes, when using `join(...)`

---

### `join(...)`

Configure relationship loading.

```rust
// Load in get_one only
#[crudcrate(non_db_attr, join(one))]
pub comments: Vec<Comment>,

// Load in both get_one and get_all
#[crudcrate(non_db_attr, join(one, all))]
pub author: Option<User>,

// Limit recursion depth
#[crudcrate(non_db_attr, join(one, all, depth = 2))]
pub nested: Vec<Nested>,
```

**Type:** Configuration
**Parameters:**
- `one` - Load in single-item responses
- `all` - Load in list responses
- `depth = N` - Maximum recursion depth (default: 5)

---

### `join_filterable(...)`

Enable filtering on columns from related entities using dot-notation.

```rust
#[sea_orm(ignore)]
#[crudcrate(
    non_db_attr,
    join(one, all),
    join_filterable("make", "year", "color")
)]
pub vehicles: Vec<Vehicle>,
```

**Type:** List of column names
**Effect:** Enables `?filter={"vehicles.make":"BMW","vehicles.year_gte":2020}`
**Security:** Only listed columns can be filtered - unlisted columns are silently ignored

---

### `join_sortable(...)`

Enable sorting on columns from related entities using dot-notation.

```rust
#[sea_orm(ignore)]
#[crudcrate(
    non_db_attr,
    join(one, all),
    join_sortable("year", "mileage")
)]
pub vehicles: Vec<Vehicle>,
```

**Type:** List of column names
**Effect:** Enables `?sort=["vehicles.year","DESC"]`
**Security:** Only listed columns can be sorted - unlisted columns fall back to default sort

---

## Common Patterns

### Auto-Generated ID

```rust
#[sea_orm(primary_key, auto_increment = false)]
#[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
pub id: Uuid,
```

### Managed Timestamps

```rust
#[crudcrate(sortable, exclude(create, update), on_create = chrono::Utc::now())]
pub created_at: DateTimeUtc,

#[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
pub updated_at: DateTimeUtc,
```

### Sensitive Data

```rust
#[crudcrate(exclude(one, list))]
pub password_hash: String,

#[crudcrate(exclude(one, list))]
pub api_secret: String,
```

### Searchable Content

```rust
#[crudcrate(filterable, sortable, fulltext)]
pub title: String,

#[crudcrate(fulltext, exclude(list))]
pub content: String,
```

### Foreign Key with Relation

```rust
#[crudcrate(filterable)]
pub author_id: Uuid,

#[sea_orm(ignore)]
#[crudcrate(non_db_attr, join(one, all, depth = 1))]
pub author: Option<User>,
```

### Relationship with Join Filtering/Sorting

```rust
// Enable filtering and sorting on related entity columns
#[sea_orm(ignore)]
#[crudcrate(
    non_db_attr,
    join(one, all, depth = 1),
    join_filterable("make", "year", "color"),
    join_sortable("year", "mileage")
)]
pub vehicles: Vec<Vehicle>,
```

Enables queries like:
- `?filter={"vehicles.make":"BMW"}`
- `?sort=["vehicles.year","DESC"]`

### Computed Field (Read-Only)

```rust
#[crudcrate(sortable, exclude(create, update))]
pub view_count: i32,
```

---

## Complete Example

```rust
#[derive(EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "articles")]
pub struct Model {
    // Primary key with auto-generation
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    // Searchable, filterable, sortable title
    #[crudcrate(filterable, sortable, fulltext)]
    pub title: String,

    // Searchable content, excluded from lists
    #[crudcrate(fulltext, exclude(list))]
    pub content: String,

    // Optional summary for lists
    pub summary: Option<String>,

    // Filterable status
    #[crudcrate(filterable)]
    pub status: ArticleStatus,

    // Foreign key with relationship
    #[crudcrate(filterable)]
    pub author_id: Uuid,

    // Read-only view counter
    #[crudcrate(sortable, exclude(create, update), on_create = 0)]
    pub view_count: i32,

    // Auto-managed timestamps
    #[crudcrate(sortable, exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTimeUtc,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now(), on_update = chrono::Utc::now())]
    pub updated_at: DateTimeUtc,

    // Relationships
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all, depth = 1))]
    pub author: Option<User>,

    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one))]
    pub comments: Vec<Comment>,

    // Relationship with join filtering/sorting
    #[sea_orm(ignore)]
    #[crudcrate(
        non_db_attr,
        join(one, all, depth = 1),
        join_filterable("tag_name"),
        join_sortable("tag_name")
    )]
    pub tags: Vec<Tag>,
}
```

## Attribute Compatibility

| Attribute | Combinable With |
|-----------|----------------|
| `primary_key` | `exclude`, `on_create` |
| `exclude` | All except `join` targets conflict |
| `filterable` | `sortable`, `fulltext`, `exclude` |
| `sortable` | `filterable`, `fulltext`, `exclude` |
| `fulltext` | `filterable`, `sortable`, `exclude` |
| `on_create` | `on_update`, `exclude(create)` |
| `on_update` | `on_create`, `exclude(update)` |
| `non_db_attr` | `join`, `join_filterable`, `join_sortable` (required) |
| `join` | `non_db_attr` (required), `join_filterable`, `join_sortable` |
| `join_filterable` | `non_db_attr`, `join`, `join_sortable` |
| `join_sortable` | `non_db_attr`, `join`, `join_filterable` |

## See Also

- [Struct Attributes Reference](./struct-attributes.md)
- [Field Exclusion](../features/field-exclusion.md)
- [Relationships](../features/relationships.md)
