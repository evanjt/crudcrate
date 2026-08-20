# Multi-Tenant Scoping

The [scoping tutorial](../tutorial/scoping.md) showed a **public vs. private** split: an
unauthenticated tier that is read-only and hides private rows. Multi-tenancy is a different shape
of the same primitive:

- Every caller is **authenticated**, and each one is confined to **their own tenant's** rows.
- Scoped callers usually still need to **create and update** within their tenant.

You build it from the same `ScopeCondition`, with two differences from the public/private case:
the condition is **per-tenant** (a value, not an `is_private` boolean), and you have to be
deliberate about **writes**.

## The catch: a present scope blocks all writes

When a `ScopeCondition` extension is on the request, the generated `POST`/`PUT`/`DELETE` handlers
return `403 Forbidden` ("Write access denied in scoped context"). That's the behaviour you want for
a read-only public tier, but in a multi-tenant app it would stop a tenant from writing their *own* data.

The fix: **inject the scope only on read requests.** Reads get confined by the
`ScopeCondition`; writes are confined by your own guard (next section), so a tenant can still
create and update within their tenant.

```rust
use axum::{extract::Request, http::Method, middleware::Next, response::Response};
use crudcrate::ScopeCondition;
use sea_orm::{ColumnTrait, Condition};

async fn scope_reads(request: Request, next: Next) -> Response {
    // Your auth middleware has already put the caller's tenant into the request extensions.
    let tenant = current_tenant(&request);

    // Only GET/HEAD get a ScopeCondition. If we injected it on writes too, every tenant write
    // would 403, so writes flow through unscoped here and are confined by `confine_writes`.
    if let (Some(tenant_id), true) =
        (tenant, matches!(*request.method(), Method::GET | Method::HEAD))
    {
        let mut request = request;
        request.extensions_mut().insert(ScopeCondition::new(
            Condition::all().add(widget::Column::TenantId.eq(tenant_id)),
        ));
        return next.run(request).await;
    }
    next.run(request).await
}
```

An **unscoped** caller (e.g. a platform admin with `current_tenant == None`) gets no extension and
therefore sees and edits everything, the same "absence of scope = full access" rule as the
tutorial.

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

Reads are handled. For writes, add a small guard that resolves the **target's** tenant and rejects
a mismatch. There's nothing CrudCrate-specific here; it's your ownership policy:

```rust
async fn confine_writes(
    State(db): State<DatabaseConnection>,
    request: Request,
    next: Next,
) -> Response {
    let Some(tenant_id) = current_tenant(&request) else {
        return next.run(request).await; // unscoped admin, no restriction
    };
    if matches!(*request.method(), Method::GET | Method::HEAD | Method::OPTIONS) {
        return next.run(request).await; // reads are confined by `scope_reads`
    }

    // For PUT/DELETE: look up the existing row's owning tenant by id.
    // For POST: read the owning tenant from the create body's foreign key.
    // Reject anything that doesn't resolve to `tenant_id` with 403. Fail closed:
    // if you can't resolve an owner (e.g. a global/shared table), deny.
    match resolve_owner_tenant(&db, &request).await {
        Some(owner) if owner == tenant_id => next.run(request).await,
        _ => forbidden("token is scoped to a different tenant"),
    }
}
```

Keeping reads and writes in separate guards is deliberate: read scoping stays a pure query filter
(`ScopeCondition`), while the write guard owns the mutation policy. They never fight, because the
read scope is never present on a write.

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

`REQUIRE_SCOPE` governs **reads only**. Write handlers ignore it: they are 403 whenever a
`ScopeCondition` is present and allowed when it is absent, so the safe-methods-only mounting
above keeps working. Confining writes to a tenant is the write guard's job, not this flag's.

## Quick reference

| Concern | Where it lives |
|---------|----------------|
| Confine **reads** to a tenant | `ScopeCondition` injected **on GET/HEAD only** |
| Scope across a join | `Column::Fk.in_subquery(parent_ids_for_tenant)` (typed, not raw SQL) |
| Let tenants **write** their own data | Don't inject the scope on writes; add a write guard |
| Confine **writes** to a tenant | Your own middleware: resolve the target's tenant, 403 on mismatch |
| Never serve unscoped by accident | `#[crudcrate(require_scope)]` |
| Hide a column from scoped reads | `exclude(scoped)` on the field (see the tutorial) |

---

**See also:** [Public & Private Endpoints](../tutorial/scoping.md) for the read-only/public tier
and `exclude(scoped)`, and [Security Best Practices](./security.md).
