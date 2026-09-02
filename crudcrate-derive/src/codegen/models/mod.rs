//! Emits the generated model structs: API struct, Create, Update, List, Response and scoped variants.

pub(crate) mod api_struct;
pub(crate) mod create;
pub(crate) mod emit;
pub(crate) mod inclusion;
pub(crate) mod list;
pub(crate) mod merge;
pub(crate) mod response;
pub(crate) mod scoped;
pub(crate) mod shared;
pub(crate) mod update;

pub(crate) use inclusion::{is_scoped_exclusion, should_include_in_model};
