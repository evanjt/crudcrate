# CRUDOperations API Reference

The `CRUDOperations` trait provides three levels of customization for CRUD
behavior. Wire it in with `#[crudcrate(operations = MyOps)]`.

## Trait definition

All methods have default no-op implementations. Override only what you need.

```rust
#[async_trait]
pub trait CRUDOperations: Send + Sync {
    type Resource: CRUDResource;

    // --- Level 1: Lifecycle hooks ---

    async fn before_get_one(&self, db: &DatabaseConnection, id: Uuid) -> Result<(), ApiError>;
    async fn after_get_one(&self, db: &DatabaseConnection, entity: &mut Self::Resource) -> Result<(), ApiError>;

    async fn before_get_all(
        &self, db: &DatabaseConnection, condition: &Condition,
        order_column: <Self::Resource as CRUDResource>::ColumnType,
        order_direction: &Order, offset: u64, limit: u64,
    ) -> Result<(), ApiError>;
    async fn after_get_all(
        &self, db: &DatabaseConnection,
        entities: &mut Vec<<Self::Resource as CRUDResource>::ListModel>,
    ) -> Result<(), ApiError>;

    async fn before_create(&self, db: &DatabaseConnection, data: &CreateModel) -> Result<(), ApiError>;
    async fn after_create(&self, db: &DatabaseConnection, entity: &mut Self::Resource) -> Result<(), ApiError>;

    async fn before_update(&self, db: &DatabaseConnection, id: Uuid, data: &UpdateModel) -> Result<(), ApiError>;
    async fn after_update(&self, db: &DatabaseConnection, entity: &mut Self::Resource) -> Result<(), ApiError>;

    async fn before_delete(&self, db: &DatabaseConnection, id: Uuid) -> Result<(), ApiError>;
    async fn after_delete(&self, db: &DatabaseConnection, id: Uuid) -> Result<(), ApiError>;

    async fn before_delete_many(&self, db: &DatabaseConnection, ids: &[Uuid]) -> Result<(), ApiError>;
    async fn after_delete_many(&self, db: &DatabaseConnection, ids: &[Uuid]) -> Result<(), ApiError>;

    // --- Level 2: Core logic overrides ---

    async fn fetch_one(&self, db: &DatabaseConnection, id: Uuid) -> Result<Self::Resource, ApiError>;
    async fn fetch_all(&self, db: &DatabaseConnection, condition: &Condition, ...) -> Result<Vec<ListModel>, ApiError>;
    async fn perform_create(&self, db: &DatabaseConnection, data: CreateModel) -> Result<Self::Resource, ApiError>;
    async fn perform_update(&self, db: &DatabaseConnection, id: Uuid, data: UpdateModel) -> Result<Self::Resource, ApiError>;
    async fn perform_delete(&self, db: &DatabaseConnection, id: Uuid) -> Result<Uuid, ApiError>;
    async fn perform_delete_many(&self, db: &DatabaseConnection, ids: Vec<Uuid>) -> Result<Vec<Uuid>, ApiError>;

    // --- Level 3: Full operation overrides ---

    async fn get_one(&self, db: &DatabaseConnection, id: Uuid) -> Result<Self::Resource, ApiError>;
    async fn get_all(&self, db: &DatabaseConnection, condition: &Condition, ...) -> Result<Vec<ListModel>, ApiError>;
    async fn create(&self, db: &DatabaseConnection, data: CreateModel) -> Result<Self::Resource, ApiError>;
    async fn update(&self, db: &DatabaseConnection, id: Uuid, data: UpdateModel) -> Result<Self::Resource, ApiError>;
    async fn delete(&self, db: &DatabaseConnection, id: Uuid) -> Result<Uuid, ApiError>;
    async fn delete_many(&self, db: &DatabaseConnection, ids: Vec<Uuid>) -> Result<Vec<Uuid>, ApiError>;
    async fn create_many(&self, db: &DatabaseConnection, data: Vec<CreateModel>) -> Result<Vec<Self::Resource>, ApiError>;
    async fn update_many(&self, db: &DatabaseConnection, updates: Vec<(Uuid, UpdateModel)>) -> Result<Vec<Self::Resource>, ApiError>;
}
```

Type aliases used above for brevity:

- `CreateModel` = `<Self::Resource as CRUDResource>::CreateModel`
- `UpdateModel` = `<Self::Resource as CRUDResource>::UpdateModel`
- `ListModel` = `<Self::Resource as CRUDResource>::ListModel`

## Customization levels

### Level 1: Lifecycle hooks

`before_*` and `after_*` methods. Called around the default core logic.
Use for validation, authorization, logging, enrichment.

`before_create` and `before_update` receive **immutable** references to the
input data. To transform input, use per-attribute hooks
(`create::one::pre`) or override `perform_create`/`perform_update` instead.

### Level 2: Core logic

`fetch_one`, `fetch_all`, `perform_create`, `perform_update`,
`perform_delete`, `perform_delete_many`. Replace the default DB query or
mutation while keeping the lifecycle hooks around it.

**Caveat**: `fetch_one` and `fetch_all` are bypassed when the entity has
`join(...)` fields, because join loading requires the raw SeaORM `Model`
to call `find_related()`. The `before_*`/`after_*` hooks still fire
normally. If you need full control over both the fetch and join loading,
use per-attribute hooks (`read::one::body`) instead.

### Level 3: Full operation override

`get_one`, `get_all`, `create`, `update`, `delete`, `delete_many`,
`create_many`, `update_many`. Replace the entire operation including
lifecycle hooks. The default implementations orchestrate
`before_* → core_logic → after_*`.

## Execution order

### get_one

```
before_get_one(db, id)
    ↓
fetch_one(db, id)          ← or join-loading codegen when joins exist
    ↓
after_get_one(db, &mut entity)
```

### get_all

```
before_get_all(db, condition, order_column, &order_direction, offset, limit)
    ↓
fetch_all(db, ...)         ← or batch-loading codegen when joins exist
    ↓
after_get_all(db, &mut entities)
```

### create

```
before_create(db, &data)
    ↓
perform_create(db, data)
    ↓
after_create(db, &mut entity)
```

### update

```
before_update(db, id, &data)
    ↓
perform_update(db, id, data)
    ↓
after_update(db, &mut entity)
```

### delete

```
before_delete(db, id)
    ↓
perform_delete(db, id)
    ↓
after_delete(db, id)
```

## See also

- [Custom Operations guide](../advanced/custom-operations.md)
- [CRUDResource API](./crudresource-api.md)
- [Lifecycle Hooks](../advanced/lifecycle-hooks.md)
