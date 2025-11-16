//! Join relationship handling
//!
//! Provides utilities for loading and configuring join relationships between entities.
//!
//! - **config**: Join configuration parsing and structures
//! - **loading**: Code generation for join loading in get_one() and get_all()

pub mod config;
pub mod loading;

pub(crate) use config::get_join_config;
pub use config::JoinConfig;
