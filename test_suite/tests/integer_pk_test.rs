// Test: Non-UUID primary key support
//
// These tests validate that CRUDResource works with integer (i32) primary keys.
// Currently ALL tests are #[ignore] because CRUDResource requires
// PrimaryKey::ValueType: From<Uuid> + Into<Uuid>, which i32 doesn't satisfy.
//
// Blockers (all in crudcrate/src/core/traits.rs):
//   1. Trait bounds: From<Uuid>/Into<Uuid> on PrimaryKey::ValueType (lines 33-35)
//   2. Method signatures: get_one, update, delete all take `id: Uuid` directly
//   3. UuidIdResult struct used in delete_many (line 13)
//   4. Batch loading codegen: HashMap<uuid::Uuid, ...> in loading.rs (11 occurrences)
//   5. parent_ids collection: Vec<uuid::Uuid> in loading.rs line 359
//
// See also: crudcrate-derive/tests/ui-fail/integer_pk.rs (compile-fail proof)
//
// To enable: make CRUDResource generic over PK type, update codegen to use
// the entity's PrimaryKey::ValueType instead of hardcoded uuid::Uuid.

// NOTE: The Tag model below cannot be compiled today because EntityToModels
// requires CRUDResource which requires UUID-compatible PKs. The model definition
// is included as documentation of the target API. Once non-UUID PKs are
// supported, uncomment the model import and test bodies.

/*
// Target model (would live in common/models/tag.rs):
use crudcrate::{EntityToModels, traits::CRUDResource};
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "tags")]
#[crudcrate(api_struct = "Tag", generate_router)]
pub struct Model {
    #[sea_orm(primary_key)]
    #[crudcrate(primary_key, exclude(update))]
    pub id: i32,

    #[crudcrate(filterable, sortable)]
    pub name: String,

    #[crudcrate(filterable)]
    pub color: Option<String>,
}
*/

#[tokio::test]
#[ignore = "blocked: CRUDResource requires PrimaryKey::ValueType: From<Uuid> + Into<Uuid>"]
async fn test_integer_pk_create() {
    // Should be able to create a Tag with auto-increment i32 PK.
    // POST /tags { "name": "rust", "color": "#DEA584" }
    // Expected: 201 Created with { "id": 1, "name": "rust", "color": "#DEA584" }
}

#[tokio::test]
#[ignore = "blocked: CRUDResource requires PrimaryKey::ValueType: From<Uuid> + Into<Uuid>"]
async fn test_integer_pk_get_one() {
    // Should be able to GET /tags/1 and receive the tag.
    // Path parameter is an integer, not a UUID string.
}

#[tokio::test]
#[ignore = "blocked: CRUDResource requires PrimaryKey::ValueType: From<Uuid> + Into<Uuid>"]
async fn test_integer_pk_get_all() {
    // Should be able to GET /tags and receive paginated results.
    // IDs in response should be integers.
}

#[tokio::test]
#[ignore = "blocked: CRUDResource requires PrimaryKey::ValueType: From<Uuid> + Into<Uuid>"]
async fn test_integer_pk_update() {
    // Should be able to PATCH /tags/1 { "name": "rust-lang" }
    // Path parameter is an integer.
}

#[tokio::test]
#[ignore = "blocked: CRUDResource requires PrimaryKey::ValueType: From<Uuid> + Into<Uuid>"]
async fn test_integer_pk_delete() {
    // Should be able to DELETE /tags/1
    // Returns the deleted integer ID.
}

#[tokio::test]
#[ignore = "blocked: CRUDResource requires PrimaryKey::ValueType: From<Uuid> + Into<Uuid>"]
async fn test_integer_pk_batch_delete() {
    // Should be able to DELETE /tags/batch with [1, 2, 3]
    // Batch loading uses HashMap<uuid::Uuid, ...> which must become generic.
}

#[tokio::test]
#[ignore = "blocked: HashMap<uuid::Uuid, ...> hardcoded in batch join loading"]
async fn test_integer_pk_batch_loading_joins() {
    // If a Tag model had joins, batch loading should work with i32 parent IDs.
    // Currently loading.rs hardcodes HashMap<uuid::Uuid, Vec<T>> and
    // Vec<uuid::Uuid> for parent_ids collection.
}
