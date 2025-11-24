# Relationships & Joins

CRUDCrate automatically loads related entities based on your configuration.

## Defining Relationships

### Step 1: Sea-ORM Relation Definition

```rust
// In your entity file
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    // Has Many: Post has many Comments
    #[sea_orm(has_many = "super::comment::Entity")]
    Comments,

    // Belongs To: Post belongs to User (author)
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::AuthorId",
        to = "super::user::Column::Id"
    )]
    Author,
}

// Implement Related trait for each relationship
impl Related<super::comment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Comments.def()
    }
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Author.def()
    }
}
```

### Step 2: Add Join Fields to Model

```rust
#[derive(EntityToModels)]
pub struct Model {
    #[sea_orm(primary_key)]
    #[crudcrate(primary_key)]
    pub id: i32,

    pub title: String,

    #[crudcrate(filterable)]
    pub author_id: i32,

    // Many relationship (Vec)
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all))]
    pub comments: Vec<super::comment::Comment>,

    // One relationship (Option)
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one))]
    pub author: Option<super::user::User>,
}
```

## Join Configuration

### `join(one)`

Load relationship only in `get_one` (single item) responses:

```rust
#[crudcrate(non_db_attr, join(one))]
pub comments: Vec<Comment>,
```

```bash
# GET /posts/1 - Comments loaded
{
  "id": 1,
  "title": "Hello World",
  "comments": [
    {"id": 1, "content": "Great post!"},
    {"id": 2, "content": "Thanks!"}
  ]
}

# GET /posts - Comments NOT loaded
[
  {"id": 1, "title": "Hello World"},
  {"id": 2, "title": "Another Post"}
]
```

### `join(all)`

Load relationship in both `get_one` and `get_all` responses:

```rust
#[crudcrate(non_db_attr, join(one, all))]
pub author: Option<User>,
```

```bash
# GET /posts - Authors loaded in list
[
  {
    "id": 1,
    "title": "Hello World",
    "author": {"id": 5, "name": "Alice"}
  }
]
```

**Caution**: Loading relationships in lists can be expensive. Use selectively.

### `depth` Parameter

Limit recursive loading depth:

```rust
// Load Author, but don't load Author's Posts (which would load Posts' Authors, etc.)
#[crudcrate(non_db_attr, join(one, depth = 1))]
pub author: Option<User>,

// Allow 2 levels: Post → Comments → Author
#[crudcrate(non_db_attr, join(one, depth = 2))]
pub comments: Vec<Comment>,
```

**Default**: When not specified, recursion is limited to 5 levels for safety.

## Relationship Types

### Has Many (One-to-Many)

```rust
// Post has many Comments
#[sea_orm(ignore)]
#[crudcrate(non_db_attr, join(one))]
pub comments: Vec<Comment>,
```

```json
{
  "id": 1,
  "comments": [
    {"id": 1, "content": "First"},
    {"id": 2, "content": "Second"}
  ]
}
```

### Belongs To (Many-to-One)

```rust
// Comment belongs to User
#[sea_orm(ignore)]
#[crudcrate(non_db_attr, join(one, all))]
pub user: Option<User>,
```

```json
{
  "id": 1,
  "content": "Great post!",
  "user": {"id": 5, "name": "Alice"}
}
```

### Has One (One-to-One)

```rust
// User has one Profile
#[sea_orm(ignore)]
#[crudcrate(non_db_attr, join(one))]
pub profile: Option<Profile>,
```

```json
{
  "id": 5,
  "name": "Alice",
  "profile": {"bio": "Developer", "avatar": "..."}
}
```

## Recursive Relationships

For self-referencing entities:

```rust
// Category can have child categories
#[derive(EntityToModels)]
#[sea_orm(table_name = "categories")]
pub struct Model {
    #[sea_orm(primary_key)]
    #[crudcrate(primary_key)]
    pub id: i32,

    pub name: String,

    pub parent_id: Option<i32>,

    // Children categories (with depth limit!)
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, depth = 3))]
    pub children: Vec<Category>,
}
```

```json
{
  "id": 1,
  "name": "Electronics",
  "children": [
    {
      "id": 2,
      "name": "Phones",
      "children": [
        {"id": 5, "name": "Smartphones", "children": []},
        {"id": 6, "name": "Feature Phones", "children": []}
      ]
    }
  ]
}
```

**Important**: Always use `depth` limit for self-referencing relationships!

## Complete Example

```rust
// user.rs
#[derive(Clone, Debug, DeriveEntityModel, Serialize, Deserialize, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    #[crudcrate(primary_key, exclude(create, update))]
    pub id: i32,

    #[crudcrate(filterable, sortable)]
    pub name: String,

    pub email: String,

    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one))]
    pub posts: Vec<super::post::Post>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::post::Entity")]
    Posts,
}

impl Related<super::post::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Posts.def()
    }
}
```

```rust
// post.rs
#[derive(Clone, Debug, DeriveEntityModel, Serialize, Deserialize, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "posts")]
pub struct Model {
    #[sea_orm(primary_key)]
    #[crudcrate(primary_key, exclude(create, update))]
    pub id: i32,

    #[crudcrate(filterable, sortable, fulltext)]
    pub title: String,

    pub content: String,

    #[crudcrate(filterable)]
    pub author_id: i32,

    // Load author in detail and list views
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all, depth = 1))]
    pub author: Option<super::user::User>,

    // Load comments only in detail view
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, depth = 2))]
    pub comments: Vec<super::comment::Comment>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::AuthorId",
        to = "super::user::Column::Id"
    )]
    Author,

    #[sea_orm(has_many = "super::comment::Entity")]
    Comments,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Author.def()
    }
}

impl Related<super::comment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Comments.def()
    }
}
```

## Performance Considerations

### N+1 Query Problem

CRUDCrate loads relationships using additional queries:

```sql
-- Main query
SELECT * FROM posts WHERE id = 1;

-- Relationship queries (N+1)
SELECT * FROM comments WHERE post_id = 1;
SELECT * FROM users WHERE id = 5;
```

For lists with `join(all)`, this becomes:

```sql
-- For each post
SELECT * FROM users WHERE id = ?;
```

### Optimization Strategies

1. **Use `join(one)` by default**: Only load in detail views
2. **Limit depth**: Prevent deep recursive loading
3. **Add indexes**: On foreign key columns
4. **Consider eager loading** for critical paths

```sql
-- Index foreign keys
CREATE INDEX idx_posts_author_id ON posts(author_id);
CREATE INDEX idx_comments_post_id ON comments(post_id);
```

### When to Use `join(all)`

✅ Use for:
- Small reference data (categories, tags)
- Essential context (author names in list)
- Data needed for display

❌ Avoid for:
- Large collections (comments, history)
- Deep hierarchies
- Optional/rare data

## Circular Reference Prevention

CRUDCrate detects and warns about potential infinite loops:

```
Warning: Potential circular reference detected in Post.author -> User.posts -> Post
Consider using depth limit: join(one, depth = 1)
```

Always use `depth` limits when entities reference each other.

## Next Steps

- Learn about [Field Exclusion](./field-exclusion.md)
- Configure [Default Values](./default-values.md)
- Set up [Error Handling](./error-handling.md)
