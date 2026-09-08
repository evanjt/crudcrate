# Registration (Upsert)

**Find or insert a row on an alternate unique key, and say what it did.**

A source system that re-sends its own content every cycle needs the write to be
idempotent, and needs the answer keyed by what it holds rather than by position
in the request. `crudcrate::upsert` does both in one transaction.

## Declaring the key

Name the columns the source identifies its rows by:

```rust
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "curves")]
#[crudcrate(api_struct = "Curve", upsert_key(source_system, source_key))]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    pub source_system: String,
    pub source_key: String,
    pub slope: f64,
}
```

The key is an alternate identity, not the primary key. The row keeps its own
`id`, which is what the rest of your API refers to.

## Registering a row

```rust
use crudcrate::{upsert, UpsertStatus};

let (curve, status) = upsert::<Curve, _>(&db, CurveCreate {
    source_system: "cnet".to_string(),
    source_key: "DOC:plate-7".to_string(),
    slope: 1.5,
}).await?;

match status {
    UpsertStatus::Created => "the key was not stored here before",
    UpsertStatus::Updated => "the key was stored and content differed",
    UpsertStatus::Unchanged => "the key was stored and content already read as sent",
};
```

The key columns are read from the create model, so you do not repeat them. On
`Unchanged` no write happens at all, which is what tells a source its content
already stands.

## What counts as content

The columns compared and written are the ones the create model can carry, minus
the primary key. A field the entity maintains itself is not part of what a
source sends, so it is neither compared nor overwritten:

- `created_at` keeps the value it was first stored with.
- A field carrying `on_update` advances when the row does change, and not on an
  `Unchanged` pass.
- A field the create model leaves unset is not compared and not written.

## Reporting a batch

`UpsertOutcome` reports one item keyed by what the sender holds:

```rust
pub struct UpsertOutcome<K, I, S = UpsertStatus> {
    pub key: K,
    pub id: Option<I>,
    pub status: S,
    pub note: Option<String>,
}
```

`status` is generic so a caller with refusals of its own (a row whose owner does
not exist here, a row it kept against what the source sent) reports them in the
same list rather than as failures. `BatchResult`, which is per-item
success-or-error, cannot express an item that was stored and still has something
to report.

## Caveats

- **Add a unique index on the key columns.** Nothing generates one. Without it,
  two registrations racing on the same key can both find nothing and both
  insert.
- A resource declaring no `upsert_key` refuses the call rather than guessing a
  key, as does a create model that leaves a key column unset.
- `upsert` is a function you call, not a generated route. It is for an ingest
  path you write, not for the public API surface.

## See also

- [Default Values](./default-values.md) for `on_create` and `on_update`
- [Struct Attributes](../reference/struct-attributes.md)
