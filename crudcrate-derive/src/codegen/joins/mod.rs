//! Join relationship configuration and loading code generation.

pub mod config;
pub mod loading;

pub(crate) use config::get_join_config;
pub use config::JoinConfig;
