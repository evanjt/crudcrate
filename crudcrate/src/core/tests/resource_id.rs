use super::*;
use uuid::Uuid;

#[test]
fn a_scalar_key_is_one_column_and_renders_as_itself() {
    let id = Uuid::nil();
    assert_eq!(id.render(), id.to_string());
    assert_eq!(<Uuid as ResourceId>::column_count(), 1);
    assert_eq!(id.into_values().len(), 1);
}

/// The tuple is what `SeaORM` gives for a composite key, and it implements neither `Display` nor
/// `Into<Value>`, which is why the trait exists.
#[test]
fn a_composite_key_renders_every_part_in_key_order() {
    let id = ("alice".to_string(), 2_i32);
    assert_eq!(id.render(), "alice, 2");
    assert_eq!(<(String, i32) as ResourceId>::column_count(), 2);
    assert_eq!(id.into_values().len(), 2);
}

#[test]
fn a_three_column_key_keeps_its_order() {
    let id = (Uuid::nil(), 7_i64, "x".to_string());
    assert_eq!(id.render(), format!("{}, 7, x", Uuid::nil()));
    assert_eq!(<(Uuid, i64, String) as ResourceId>::column_count(), 3);
}

/// The installed base is single-key, and its batch delete must keep emitting the `IN` list it
/// always did rather than an OR chain per id.
#[test]
fn a_single_column_key_batch_stays_an_in_list() {
    use sea_orm::{EntityTrait, QueryFilter, QueryTrait};

    let sql = crate::filtering::test_support::entity::Entity::find()
        .filter(any_key_condition(
            &[crate::filtering::test_support::entity::Column::Id],
            vec![Uuid::nil(), Uuid::max()],
        ))
        .build(sea_orm::DatabaseBackend::Postgres)
        .to_string();
    assert!(sql.contains(" IN ("), "one column is an IN list: {sql}");
}
