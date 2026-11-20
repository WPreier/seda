//! Seda static-memory backend library.

pub mod admin;
pub mod config;
pub mod content;
pub mod hardening;
pub mod proxy;

/// Crate version reported by the command-line interface.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default configuration embedded in every Seda binary.
pub const DEFAULT_CONFIG_TOML: &str = include_str!("../assets/default_config.toml");
