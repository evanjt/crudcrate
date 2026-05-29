# Custom Operations

The `CRUDOperations` trait is an alternative to per-attribute hooks. Implement
it on a unit struct, override only the methods you need, and wire it in with
`#[crudcrate(operations = MyOps)]`.

## When to use operations vs per-attribute hooks

Use **operations** when you want a single struct that owns all customization
for a resource — validation, authorization, side effects. It reads like a
controller.

Use **per-attribute hooks** (`#[crudcrate(create::one::pre = validate)]`)
when you only need one or two hooks and prefer keeping them close to the
model definition.

The two systems are alternatives. When `operations` is set, per-attribute
hooks on the same resource are ignored.

## Setup

### 1. Define the operations struct

```rust
use async_trait::async_trait;
use crudcrate::{CRUDOperations, ApiError};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

pub struct AssetOps;

#[async_trait]
impl CRUDOperations for AssetOps {
    type Resource = Asset;

    async fn before_create(
        &self,
        db: &DatabaseConnection,
        data: &<Asset as crudcrate::traits::CRUDResource>::CreateModel,
    ) -> Result<(), ApiError> {
        // Validate, authorize, log — whatever you need
        Ok(())
    }

    async fn before_delete(
        &self,
        _db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<(), ApiError> {
        // Clean up S3 before the row is deleted
        delete_from_s3(id).await
            .map_err(|e| ApiError::internal(format!("S3 cleanup failed: {e}"), None))?;
        Ok(())
    }

    // All other methods use default no-op implementations
}
```

### 2. Register with the entity

```rust
#[derive(EntityToModels)]
#[crudcrate(generate_router, operations = AssetOps)]
pub struct Model {
    // ...
}
```

## Three levels of customization

### Level 1: Lifecycle hooks

`before_*` and `after_*` methods run around the default core logic.

```rust
async fn before_create(&self, db: &DatabaseConnection, data: &CreateModel) -> Result<(), ApiError>;
async fn after_create(&self, db: &DatabaseConnection, entity: &mut Self::Resource) -> Result<(), ApiError>;

async fn before_get_one(&self, db: &DatabaseConnection, id: Uuid) -> Result<(), ApiError>;
async fn after_get_one(&self, db: &DatabaseConnection, entity: &mut Self::Resource) -> Result<(), ApiError>;
```

`before_create` and `before_update` take **immutable** references to the
input data. They're for validation and authorization, not transformation.
To transform input before insertion, override `perform_create` (level 2).

`after_*` hooks take `&mut` references — use them for enrichment
(computed fields, view counts, etc.).

### Level 2: Core logic overrides

Replace the default DB query or mutation while keeping lifecycle hooks
around it.

```rust
async fn fetch_one(&self, db: &DatabaseConnection, id: Uuid) -> Result<Self::Resource, ApiError> {
    // Custom query — eg. select specific columns, add joins
    // ...
}

async fn perform_create(&self, db: &DatabaseConnection, data: CreateModel) -> Result<Self::Resource, ApiError> {
    // Custom insertion logic — eg. transform data before insert
    // ...
}
```

### Level 3: Full operation overrides

Replace the entire operation including lifecycle hooks. The default
implementations orchestrate `before_* → core_logic → after_*`, so
overriding here means you take full control.

```rust
async fn delete(&self, db: &DatabaseConnection, id: Uuid) -> Result<Uuid, ApiError> {
    // Completely custom — S3 cleanup, cascade, audit, everything
    let asset = Asset::get_one(db, id).await?;
    delete_from_s3(&asset.s3_key).await?;
    Asset::delete(db, id).await
}
```

## Interaction with joins

Entities can have both `operations` and `join(...)` fields. When they do:

- **`get_one` / `get_all`** use the standard join-loading codegen (batch
  loading, scoped child queries, FK resolution). The `before_get_one` /
  `after_get_one` and `before_get_all` / `after_get_all` hooks still fire
  around the join-loaded body.

- **`fetch_one` / `fetch_all` overrides are bypassed** when joins are
  present. Join loading needs the raw SeaORM `Model` for
  `model.find_related()`, which `fetch_one` doesn't return (it returns
  the converted API struct). If you need full control over both the fetch
  query and join loading, use per-attribute hooks (`read::one::body`)
  instead.

- **`get_one_scoped` / `get_all_scoped`** are generated with scope-aware
  join loading, same as entities without operations.

## Common patterns

### Validation

```rust
async fn before_create(
    &self,
    _db: &DatabaseConnection,
    data: &ArticleCreate,
) -> Result<(), ApiError> {
    if data.title.trim().len() < 5 {
        return Err(ApiError::bad_request("Title must be at least 5 characters"));
    }
    Ok(())
}
```

### Authorization

```rust
async fn before_update(
    &self,
    db: &DatabaseConnection,
    id: Uuid,
    _data: &ArticleUpdate,
) -> Result<(), ApiError> {
    let article = Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or_else(|| ApiError::not_found("article", Some(id.to_string())))?;

    let user = get_current_user();
    if article.author_id != user.id && !user.is_admin {
        return Err(ApiError::forbidden("Not the author"));
    }
    Ok(())
}
```

### Enrichment via after hooks

```rust
async fn after_get_one(
    &self,
    db: &DatabaseConnection,
    entity: &mut Article,
) -> Result<(), ApiError> {
    entity.view_count = get_view_count(db, entity.id).await?;
    Ok(())
}
```

### Cascading deletes

```rust
async fn before_delete(
    &self,
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError> {
    comment::Entity::delete_many()
        .filter(comment::Column::ArticleId.eq(id))
        .exec(db)
        .await?;
    Ok(())
}
```

## See also

- [CRUDOperations API reference](../reference/crudoperations-api.md) — full trait definition and execution order
- [Lifecycle Hooks](./lifecycle-hooks.md) — per-attribute hook alternative
- [Security](./security.md) — SecurityProfile configuration
