# Multi-Tenant Scoping

The [scoping tutorial](../tutorial/scoping.md) showed a **public vs. private** split: an
unauthenticated tier that is read-only and hides private rows. Multi-tenancy is a different shape
of the same primitive:

- Every caller is **authenticated**, and each one is confined to **their own tenant's** rows.
- Scoped callers usually still need to **create and update** within their tenant.

You build it from the same `ScopeCondition`, with two differences from the public/private case:
the condition is **per-tenant** (a value, not an `is_private` boolean), and you have to be
deliberate about **writes**.

## Apply the condition to every method

A `ScopeCondition` filters reads and confines writes. Inject it on every request from a tenant:

```rust
use axum::{extract::Request, middleware::Next, response::Response};
use crudcrate::ScopeCondition;
use sea_orm::{ColumnTrait, Condition};

async fn scope_tenant(mut request: Request, next: Next) -> Response {
    if let Some(tenant_id) = current_tenant(&request) {
        request.extensions_mut().insert(ScopeCondition::new(
            Condition::all().add(widget::Column::TenantId.eq(tenant_id)),
        ));
    }
    next.run(request).await
}
```

A caller without an extension can read and write every row. Authenticate callers and decide
which methods they may use before injecting the condition. Use `read_only_router()` for a
public mount that must not accept writes.

## A per-tenant condition across a join (typed, injection-safe)

Often the `tenant_id` lives on a parent table and the entity you're serving only has a foreign key
to it. A `Condition` works on the entity's own columns, but it can hold a **subquery**, so you
scope the child by the set of parent ids the tenant owns in one statement, with no extra round-trip.

The tutorial used `Expr::cust()` (raw SQL, with a SQL-injection warning). Prefer the **typed**
builder when you can. It can't be injected into and it survives column renames:

```rust
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QuerySelect, QueryTrait};

// `gadget` only has `widget_id`; the tenant lives on `widget`.
fn gadget_scope(tenant_id: Uuid) -> Condition {
    let widgets_in_tenant = widget::Entity::find()
        .select_only()
        .column(widget::Column::Id)
        .filter(widget::Column::TenantId.eq(tenant_id))
        .into_query(); // -> sea_query::SelectStatement

    Condition::all().add(gadget::Column::WidgetId.in_subquery(widgets_in_tenant))
}
```

Rows whose foreign key is `NULL` (an unassigned child) drop out of an `IN (subquery)` by
construction, usually exactly what you want for a scoped caller.

## Confining writes

An update or delete selects the existing row under the condition and locks it for the
transaction. An excluded row returns 404. Creates and updates check their resulting rows
before commit; a row outside the condition returns 403 and rolls the transaction back.
This also catches an update that changes the row's tenant.

Batches are atomic by default. With `?partial=true`, each item has its own transaction and
an excluded item is reported as a failure. Custom resource hooks run inside the same
transaction as the scope checks. Hooks remain responsible for any additional rows they
write and for external side effects that a database rollback cannot undo.

The derive implements `CRUDResource::resource_id()` to identify a created row for the check.
A manual resource implementation must provide it for scoped creates and updates; the default
fails closed. Direct `CRUDResource` calls remain unscoped; use `crudcrate::scope` write helpers
when composing a scoped operation outside the generated handlers.

## Fail closed with `REQUIRE_SCOPE`

For an entity that must **never** be served without a scope (a misrouted handler would otherwise
leak every tenant's rows), set `REQUIRE_SCOPE`. The generated read handlers then return `500` when
no `ScopeCondition` is present, instead of silently returning everything:

```rust
#[crudcrate(require_scope, generate_router)]
pub struct Widget { /* ... */ }
```

Leave it **off** for entities that have a legitimate unscoped (admin) caller; there, the absence
of the extension is the intended "see everything" path.

`REQUIRE_SCOPE` governs reads only. Writes with an extension are confined; writes without
one remain unrestricted. It does not replace authentication or method permissions.

## Quick reference

| Concern | Where it lives |
|---------|----------------|
| Confine **reads** to a tenant | `ScopeCondition` injected by middleware |
| Scope across a join | `Column::Fk.in_subquery(parent_ids_for_tenant)` (typed, not raw SQL) |
| Let tenants **write** their own data | Inject the scope on writes too |
| Confine **writes** to a tenant | One condition, checked inside the write transaction |
| Never serve unscoped by accident | `#[crudcrate(require_scope)]` |
| Hide a column from scoped reads | `exclude(scoped)` on the field (see the tutorial) |

---

**See also:** [Public & Private Endpoints](../tutorial/scoping.md) for the read-only/public tier
and `exclude(scoped)`, and [Security Best Practices](./security.md).
