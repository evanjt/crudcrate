# Migrating from crudcrate 0.11.x to 0.12.0

The CRUD traits are generic over the connection and no longer use `async-trait`.
Hook functions and trait method overrides change signature. The single-row and
batch lifecycles now run in a transaction, so a hook that fails after the write
takes the write with it.

## TL;DR

| Scenario | Action |
|---|---|
| You use `#[crudcrate(generate_router)]` or `crud_handlers!` with no hooks and no overrides | Nothing to do. Generated code is updated. |
| You write hook functions (`create::one::pre`, `read::one::transform`, ...) | Make each generic: `<C: ConnectionTrait + TransactionTrait>` taking `db: &C`. |
| You `impl CRUDOperations` or override a `CRUDResource` method | Same signature change, and delete the `#[async_trait]` attribute on the impl. |
| Your `Cargo.toml` has `async-trait` only for crudcrate impls | Remove it. |
| A hook of yours writes rows it expects to keep when the operation later fails | It is now rolled back with the operation. Move that work after the call, or to a separate transaction you open yourself. |
| You relied on a failed batch leaving its earlier rows written | It no longer does. Use `?partial=true` for per-item outcomes. |
| You call a crudcrate operation from inside your own transaction | Pass the `&DatabaseTransaction` directly. It used to require a `&DatabaseConnection`. |
| A hook of yours reaches app state for a second connection instead of using the `db` it is handed | Use `db`. The lifecycle holds a pooled connection until it commits, and a hook taking another from the same pool can exhaust it under load. |
| You register rows with `upsert` on an entity with managed timestamps | Fixed. Re-sent content now reports `Unchanged` and leaves `created_at` alone. |

## The signature change

```rust
// 0.11
async fn validate_article(
    db: &DatabaseConnection,
    data: &mut ArticleCreate,
) -> Result<(), ApiError> {

// 0.12
async fn validate_article<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(
    db: &C,
    data: &mut ArticleCreate,
) -> Result<(), ApiError> {
```

The same change applies to every method of an operations struct, and the
`#[async_trait]` attribute goes:

```rust
// 0.11
#[async_trait]
impl CRUDOperations for AssetOps {
    type Resource = Asset;

    async fn before_delete(&self, db: &DatabaseConnection, id: Uuid) -> Result<(), ApiError> {

// 0.12
impl CRUDOperations for AssetOps {
    type Resource = Asset;

    async fn before_delete<C: sea_orm::ConnectionTrait + sea_orm::TransactionTrait>(
        &self,
        db: &C,
        id: Uuid,
    ) -> Result<(), ApiError> {
```

Both bounds are required because a hook may itself run a crudcrate operation,
which opens a transaction. `DatabaseConnection` and `DatabaseTransaction` both
satisfy them, so any connection you could pass in 0.11 you can still pass.

## What the transaction changes

`create`, `update` and `delete` open a transaction, run `after_begin`, the
before hook, the write, and the after hook, then commit. Anything that fails
rolls the whole thing back.

```rust
// 0.11: the row was created and the error returned. The row stayed.
// 0.12: the row is not there.
async fn after_create<C: ConnectionTrait + TransactionTrait>(
    &self,
    db: &C,
    entity: &mut Asset,
) -> Result<(), ApiError> {
    Err(ApiError::bad_request("refused"))
}
```

If a hook does work that must survive a later failure, do it after the operation
returns rather than inside the hook.

`create_many`, `update_many` and `delete_many` wrap their rows in one
transaction. A failure at any row leaves none of them, so an error no longer
leaves half a batch written. Each row's own lifecycle is a savepoint inside that
transaction. The `?partial=true` batch endpoints are unchanged: they run each row
on the connection and report `succeeded` and `failed` with `207 Multi-Status`.

## Session state for a write

`after_begin` runs immediately after every `BEGIN` a write issues, before any
other hook on that transaction. In a batch that is once for the enclosing
transaction, ahead of `before_create_many` and its siblings, and again for each
row's savepoint. It is the place for state the write itself needs, which is what
`SET LOCAL` is for. Setting it here scopes it to this transaction instead of
leaving it on a pooled connection.

```rust
async fn after_begin<C: ConnectionTrait + TransactionTrait>(&self, db: &C) -> Result<(), ApiError> {
    db.execute_unprepared("SET LOCAL app.actor = 'alice'").await?;
    Ok(())
}
```

## Running an operation inside your own transaction

```rust
let txn = db.begin().await?;
let asset = Asset::create(&txn, payload).await?;
other_work(&txn, asset.id).await?;
txn.commit().await?;
```

The operation's own `begin` becomes a savepoint within your transaction, so it
commits or rolls back with yours.

## Send bounds and generic callers

Every trait method is declared as returning `impl Future<Output = ...> + Send`,
which is what `async-trait` gave you in 0.11. Code generic over the resource
spawns or boxes an operation's future as before:

```rust
fn spawn_create<R: CRUDResource + 'static>(db: DatabaseConnection, data: R::CreateModel)
    -> JoinHandle<Result<R, ApiError>>
where
    R::CreateModel: 'static,
{
    tokio::spawn(async move { R::create(&db, data).await })
}
```

An `impl` still writes `async fn`. The compiler checks that its future is
`Send`, so a hook holding a non-`Send` value across an `.await` is refused at
the impl, the same as it was under `async-trait`.

## Hooks hold the connection

A write lifecycle holds one pooled connection from `BEGIN` to `COMMIT`, and the
hooks run inside that span. Two things follow.

Use the `db` a hook is handed. It is the transaction, and queries on it reuse
the connection the lifecycle already holds. A hook that instead reaches app
state for the pool takes a second connection for the same request; under
concurrent writes at the pool's size, every request holds one and waits for
another, and none finishes.

```rust
// Holds two connections per request. Under load this exhausts the pool.
async fn before_delete<C: ConnectionTrait + TransactionTrait>(&self, _db: &C, id: Uuid) -> Result<(), ApiError> {
    Asset::get_one(&self.pool, id).await?;
    Ok(())
}

// Holds one. `db` is the transaction the delete runs in.
async fn before_delete<C: ConnectionTrait + TransactionTrait>(&self, db: &C, id: Uuid) -> Result<(), ApiError> {
    Asset::get_one(db, id).await?;
    Ok(())
}
```

External I/O in a hook (an object store, a webhook) runs with the connection
held, so it counts against the pool for as long as it takes. Where that time
matters, do the call before the operation or after it returns, and keep the hook
to the checks and writes the transaction is for.

## Registration on an entity with managed timestamps

`upsert` compares the columns the create model can carry. A field the entity
maintains itself (`created_at`, `updated_at`) is not part of what a source sends,
so it is neither compared nor overwritten. Re-sending identical content reports
`Unchanged` and writes nothing. A pass that does change content advances any
field carrying `on_update`.

In 0.11.1 this comparison included every stored column, so an entity with
timestamps reported `Updated` on every pass and rewrote `created_at`.
