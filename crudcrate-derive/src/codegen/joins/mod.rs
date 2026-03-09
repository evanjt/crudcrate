//! Join relationship configuration and loading code generation.

pub mod config;
pub mod loading;

pub use config::JoinConfig;
pub(crate) use config::get_join_config;
