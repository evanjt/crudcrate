# Blog with Comments Example

A blog API with posts, comments, and authors.

## Entities

### User (Author)

```rust
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub name: String,

    #[crudcrate(filterable)]
    pub email: String,

    #[crudcrate(exclude(one, list))]
    pub password_hash: String,

    pub bio: Option<String>,

    #[crudcrate(sortable, exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTimeUtc,

    // Relationship: User has many Posts
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

### Post

```rust
#[derive(Clone, Debug, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(15))")]
pub enum PostStatus {
    #[sea_orm(string_value = "draft")]
    Draft,
    #[sea_orm(string_value = "published")]
    Published,
    #[sea_orm(string_value = "archived")]
    Archived,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "posts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable, fulltext)]
    pub title: String,

    pub slug: String,

    #[crudcrate(fulltext, exclude(list))]
    pub content: String,

    pub excerpt: Option<String>,

    #[crudcrate(filterable)]
    pub status: PostStatus,

    #[crudcrate(filterable)]
    pub author_id: Uuid,

    #[crudcrate(sortable, filterable)]
    pub published_at: Option<DateTimeUtc>,

    #[crudcrate(sortable, exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTimeUtc,

    // Author (belongs_to)
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all, depth = 1))]
    pub author: Option<super::user::User>,

    // Comments (has_many) - only in detail view
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one))]
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
    fn to() -> RelationDef { Relation::Author.def() }
}

impl Related<super::comment::Entity> for Entity {
    fn to() -> RelationDef { Relation::Comments.def() }
}
```

### Comment

```rust
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "comments")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(fulltext)]
    pub content: String,

    #[crudcrate(filterable)]
    pub post_id: Uuid,

    #[crudcrate(filterable)]
    pub author_id: Uuid,

    #[crudcrate(sortable, exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTimeUtc,

    // Comment author
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all, depth = 1))]
    pub author: Option<super::user::User>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::post::Entity",
        from = "Column::PostId",
        to = "super::post::Column::Id"
    )]
    Post,

    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::AuthorId",
        to = "super::user::Column::Id"
    )]
    Author,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef { Relation::Author.def() }
}

impl Related<super::post::Entity> for Entity {
    fn to() -> RelationDef { Relation::Post.def() }
}
```

## Router Setup

```rust
let app = Router::new()
    .merge(user::user_router())
    .merge(post::post_router())
    .merge(comment::comment_router())
    .layer(Extension(db));
```

## API Examples

### Create a User

```bash
curl -X POST http://localhost:3000/users \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Alice",
    "email": "alice@example.com",
    "password_hash": "hashed_password",
    "bio": "Software developer"
  }'
```

### Create a Post

```bash
curl -X POST http://localhost:3000/posts \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Getting Started with Rust",
    "slug": "getting-started-with-rust",
    "content": "Full article content...",
    "excerpt": "Learn the basics of Rust programming",
    "status": "published",
    "author_id": "{user-id}",
    "published_at": "2024-01-15T10:00:00Z"
  }'
```

### List Published Posts with Authors

```bash
curl "http://localhost:3000/posts?filter={\"status\":\"published\"}&sort=[\"published_at\",\"DESC\"]"
```

### Get Post with Comments

```bash
curl "http://localhost:3000/posts/{post-id}"

# Response includes author and comments
{
  "id": "...",
  "title": "Getting Started with Rust",
  "author": {
    "id": "...",
    "name": "Alice"
  },
  "comments": [
    {
      "id": "...",
      "content": "Great article!",
      "author": {"name": "Bob"}
    }
  ]
}
```

### Add a Comment

```bash
curl -X POST http://localhost:3000/comments \
  -H "Content-Type: application/json" \
  -d '{
    "content": "This really helped me understand Rust!",
    "post_id": "{post-id}",
    "author_id": "{user-id}"
  }'
```

### Search Posts

```bash
curl "http://localhost:3000/posts?q=rust programming"
```
