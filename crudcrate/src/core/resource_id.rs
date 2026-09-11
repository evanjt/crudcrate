//! The primary-key value of a resource, whether it is one column or several.
//!
//! A single-column key is whatever the entity declares (`Uuid`, `i32`, `String`); `SeaORM` spells a
//! composite key as a tuple of those. The two are handled through this trait rather than through
//! `Display` and `Into<sea_orm::Value>` directly, because a tuple implements neither and the orphan
//! rule puts an impl for one out of a consumer's reach.

use sea_orm::{ColumnTrait, Condition, Value};

/// A key value, rendered for a message and decomposed for a query.
pub trait ResourceId: Clone + Eq + std::hash::Hash + Send + Sync + 'static {
    /// How the key reads in a not-found message. A single-column key is its own `Display`; a
    /// composite key is its parts joined, in key order.
    fn render(&self) -> String;

    /// The key's columns' values, in the order [`crate::CRUDResource::ID_COLUMNS`] names them.
    fn into_values(self) -> Vec<Value>;

    /// How many columns the key spans. `1` for every scalar.
    fn column_count() -> usize;
}

/// `column = value` for each part of the key, which is one row.
pub fn key_condition<C: ColumnTrait, I: ResourceId>(columns: &[C], id: I) -> Condition {
    columns
        .iter()
        .zip(id.into_values())
        .fold(Condition::all(), |condition, (column, value)| {
            condition.add(column.eq(value))
        })
}

/// Any of these keys, as one condition. This is what a batch operation filters on.
///
/// A single-column key stays an `IN` list, which is the statement every existing consumer already
/// emits and the one the planner handles as a single scalar-array operation. Only a composite key
/// becomes an OR of per-key equality, because a tuple cannot be an `IN` list.
pub fn any_key_condition<C: ColumnTrait, I: ResourceId>(
    columns: &[C],
    ids: impl IntoIterator<Item = I>,
) -> Condition {
    let ids = ids.into_iter();
    match columns {
        [column] => {
            let values: Vec<Value> = ids.flat_map(ResourceId::into_values).collect();
            Condition::all().add(column.is_in(values))
        }
        _ => ids.fold(Condition::any(), |condition, id| {
            condition.add(key_condition(columns, id))
        }),
    }
}

macro_rules! scalar_resource_id {
    ($($type:ty),* $(,)?) => {
        $(
            impl ResourceId for $type {
                fn render(&self) -> String {
                    ::std::string::ToString::to_string(self)
                }
                fn into_values(self) -> Vec<Value> {
                    vec![self.into()]
                }
                fn column_count() -> usize {
                    1
                }
            }
        )*
    };
}

scalar_resource_id!(
    uuid::Uuid,
    String,
    i8,
    i16,
    i32,
    i64,
    u8,
    u16,
    u32,
    u64,
    chrono::NaiveDate,
    chrono::NaiveDateTime,
    chrono::DateTime<chrono::Utc>,
    chrono::DateTime<chrono::FixedOffset>,
);

macro_rules! tuple_resource_id {
    ($($name:ident),+) => {
        impl<$($name),+> ResourceId for ($($name,)+)
        where
            $($name: ResourceId + Into<Value>,)+
        {
            fn render(&self) -> String {
                #[allow(non_snake_case)]
                let ($($name,)+) = self;
                [$(ResourceId::render($name)),+].join(", ")
            }
            fn into_values(self) -> Vec<Value> {
                #[allow(non_snake_case)]
                let ($($name,)+) = self;
                vec![$($name.into()),+]
            }
            fn column_count() -> usize {
                [$(stringify!($name)),+].len()
            }
        }
    };
}

tuple_resource_id!(A, B);
tuple_resource_id!(A, B, C);
tuple_resource_id!(A, B, C, D);

#[cfg(test)]
#[path = "tests/resource_id.rs"]
mod tests;
