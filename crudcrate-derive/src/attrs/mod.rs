//! Parsing of `#[crudcrate(...)]` attributes into the derive IR.

pub(crate) mod field;
pub(crate) mod hooks;
pub(crate) mod join;
pub(crate) mod resource;

pub(crate) use field::{field_has_crudcrate_flag, get_crudcrate_bool, get_crudcrate_expr};
pub(crate) use join::{JoinConfig, get_join_config};
pub(crate) use resource::{extract_table_name, get_string_from_attr, parse_crud_resource_meta};
