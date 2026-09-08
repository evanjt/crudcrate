# CRUDOperations API Reference

The `CRUDOperations` trait provides three levels of customization for CRUD
behavior. Wire it in with `#[crudcrate(operations = MyOps)]`.

## Trait definition

All methods have default no-op implementations. Override only what you need.

```rust
pub trait CRUDOperations: Send + Sync {
    type Resource: CRUDResource;

    // --- Level 1: Lifecycle hooks ---

    async fn after_begin<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C) -> Result<(), ApiError>;

    async fn before_get_one<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, id: Uuid) -> Result<(), ApiError>;
    async fn after_get_one<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, entity: &mut Self::Resource) -> Result<(), ApiError>;

    async fn before_get_all<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(
        &self, db: &C, condition: &Condition,
        order_column: <Self::Resource as CRUDResource>::ColumnType,
        order_direction: &Order, offset: u64, limit: u64,
    ) -> Result<(), ApiError>;
    async fn after_get_all<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(
        &self, db: &C,
        entities: &mut Vec<<Self::Resource as CRUDResource>::ListModel>,
    ) -> Result<(), ApiError>;

    async fn before_create<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, data: &CreateModel) -> Result<(), ApiError>;
    async fn after_create<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, entity: &mut Self::Resource) -> Result<(), ApiError>;

    async fn before_update<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, id: Uuid, data: &UpdateModel) -> Result<(), ApiError>;
    async fn after_update<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, entity: &mut Self::Resource) -> Result<(), ApiError>;

    async fn before_delete<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, id: Uuid) -> Result<(), ApiError>;
    async fn after_delete<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, id: Uuid) -> Result<(), ApiError>;

    async fn before_delete_many<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, ids: &[Uuid]) -> Result<(), ApiError>;
    async fn after_delete_many<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, ids: &[Uuid]) -> Result<(), ApiError>;

    // --- Level 2: Core logic overrides ---

    async fn fetch_one<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, id: Uuid) -> Result<Self::Resource, ApiError>;
    async fn fetch_all<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, condition: &Condition, ...) -> Result<Vec<ListModel>, ApiError>;
    async fn perform_create<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, data: CreateModel) -> Result<Self::Resource, ApiError>;
    async fn perform_update<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, id: Uuid, data: UpdateModel) -> Result<Self::Resource, ApiError>;
    async fn perform_delete<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, id: Uuid) -> Result<Uuid, ApiError>;
    async fn perform_delete_many<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, ids: Vec<Uuid>) -> Result<Vec<Uuid>, ApiError>;

    // --- Level 3: Full operation overrides ---

    async fn get_one<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, id: Uuid) -> Result<Self::Resource, ApiError>;
    async fn get_all<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, condition: &Condition, ...) -> Result<Vec<ListModel>, ApiError>;
    async fn create<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, data: CreateModel) -> Result<Self::Resource, ApiError>;
    async fn update<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, id: Uuid, data: UpdateModel) -> Result<Self::Resource, ApiError>;
    async fn delete<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, id: Uuid) -> Result<Uuid, ApiError>;
    async fn delete_many<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, ids: Vec<Uuid>) -> Result<Vec<Uuid>, ApiError>;
    async fn create_many<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, data: Vec<CreateModel>) -> Result<Vec<Self::Resource>, ApiError>;
    async fn update_many<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(&self, db: &C, updates: Vec<(Uuid, UpdateModel)>) -> Result<Vec<Self::Resource>, ApiError>;
}
```

Type aliases used above for brevity:

- `CreateModel` = `<Self::Resource as CRUDResource>::CreateModel`
- `UpdateModel` = `<Self::Resource as CRUDResource>::UpdateModel`
- `ListModel` = `<Self::Resource as CRUDResource>::ListModel`

## Customization levels

### Level 1: Lifecycle hooks

`before_*` and `after_*` methods. Called around the default core logic.
`after_begin` runs first of all on a write, on the transaction the write happens
in, which is where session state such as `SET LOCAL` belongs.
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

The write lifecycles run inside a transaction the operation opens. `db` in every
one of their hooks is that transaction, so a hook's own writes commit with the
row and roll back with it.

### create

```
BEGIN
    ↓
after_begin(txn)
    ↓
before_create(txn, &data)
    ↓
perform_create(txn, data)
    ↓
after_create(txn, &mut entity)
    ↓
COMMIT
```

### update

```
BEGIN
    ↓
after_begin(txn)
    ↓
before_update(txn, id, &data)
    ↓
perform_update(txn, id, data)
    ↓
after_update(txn, &mut entity)
    ↓
COMMIT
```

### delete

```
BEGIN
    ↓
after_begin(txn)
    ↓
before_delete(txn, id)
    ↓
perform_delete(txn, id)
    ↓
after_delete(txn, id)
    ↓
COMMIT
```

Anything that returns an error rolls the transaction back, so a refusal in
`after_create` leaves no row behind.

### create_many, update_many, delete_many

One transaction encloses the batch, and each row's own lifecycle is a savepoint
within it. A failure at any row leaves none of the batch written. The
`?partial=true` endpoints instead run each row on the connection and report
per-item outcomes.

## See also

- [Custom Operations guide](../advanced/custom-operations.md)
- [CRUDResource API](./crudresource-api.md)
- [Lifecycle Hooks](../advanced/lifecycle-hooks.md)
