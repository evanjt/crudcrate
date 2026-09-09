# Migrating from crudcrate 0.12.x to 0.13.0

Two changes need action: `upsert` takes an active model, and a `ScopeCondition`
on a write request now confines the write instead of refusing it.

## TL;DR

| Scenario | Action |
|---|---|
| You call `crudcrate::upsert` | Pass an active model: `upsert::<Curve, _>(&db, create.into())`. |
| A registration key is `exclude(create, update)` | Convert the create model, set the key columns on the active model, then call `upsert`. |
| You inject a `ScopeCondition` on reads only, and guard writes yourself | Inject it on writes too and drop the guard, or keep the guard. Both work. |
| You rely on a present `ScopeCondition` returning 403 for every write | It no longer does. Mount `read_only_router()`, or authorize methods before the handler. |
| You `impl CRUDResource` by hand and serve scoped writes | Implement `resource_id()`. The default fails closed. |
| You generate clients from the OpenAPI document | Regenerate. A nullable field on a response model is now `required`. |

## `upsert` takes an active model

```rust
// 0.12
let (curve, status) = upsert::<Curve, _>(&db, CurveCreate {
    source_system: "cnet".to_string(),
    source_key: "DOC:plate-7".to_string(),
    slope: 1.5,
}).await?;

// 0.13
let (curve, status) = upsert::<Curve, _>(&db, CurveCreate {
    source_system: "cnet".to_string(),
    source_key: "DOC:plate-7".to_string(),
    slope: 1.5,
}.into()).await?;
```

The key columns are read from the active model. A key the API must not accept
from a request stays excluded from the create and update models, and the ingest
path sets it from the authenticated caller:

```rust
let mut active: curve::ActiveModel = create.into();
active.source_system = Set(caller.system.clone());
active.source_key = Set(sent_key);
let (curve, status) = upsert::<Curve, _>(&db, active).await?;
```

Comparison is unchanged in spirit: a column the model leaves unset is neither
compared nor written, and identical content still reports `Unchanged` without
writing.

## Scoped writes are confined, not refused

In 0.12 a `ScopeCondition` on the request made every `POST`, `PUT` and `DELETE`
return `403 Forbidden`. In 0.13 the condition confines the write inside its
transaction:

- An update or delete selects the row under the condition and locks it. An
  excluded row is a 404.
- A create or update checks the resulting row before commit. A row that lands
  outside the condition rolls back with a 403, which also catches an update that
  moves a row to another tenant.
- A batch shares the write transaction, custom hooks included. With
  `?partial=true` each item is confined on its own.
- Write responses use the scoped field exclusions.

A multi-tenant app can now inject one condition on every request and delete the
read-only/write-guard split described in the 0.12 multi-tenant guide.

A scope is not a permission. Where a caller must not write at all, mount
`Resource::read_only_router(&db)` or authorize the method before the handler
runs.

Confining a created or updated row needs the row's id, which the derive supplies
through `CRUDResource::resource_id()`. A hand-written `CRUDResource` impl that
serves scoped writes must implement it; the provided default returns an error.
Direct `CRUDResource` calls are unscoped as before. The `crudcrate::scope`
functions (`create`, `update`, `delete` and their `_many` forms) run the same
checks outside the generated handlers.

## A nullable response field is `required`

utoipa reads every `Option<T>` as not-required and nullable, so a field the API
always sends as null and one a client may omit were described identically. On
List, Response, scoped and API models the field is now `required` and nullable.
A field carrying `skip_serializing_if`, or declaring its own
`#[schema(required)]`, keeps what it declares. Create and update models are
requests and are untouched.

Generated clients change shape: the field stops being optional in the client's
constructor and is a nullable value instead. Regenerate and fix the call sites.
