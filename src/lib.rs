//! cargo-cross: Cross-compilation tool for Rust projects
//!
//! This crate provides cross-compilation support for Rust projects across
//! multiple platforms including Linux, Windows, macOS, FreeBSD, iOS, and Android.
//!
//! Unlike other cross-compilation tools, cargo-cross does not require Docker.
//! It downloads and manages cross-compilation toolchains automatically.

pub mod cargo;
pub mod cli;
pub mod color;
pub mod config;
pub mod download;
pub mod env;
pub mod error;
pub mod platform;
pub mod runner;

pub use cli::{parse_args, Args, Command};
pub use config::{get_target_config, HostPlatform, TargetConfig};
pub use env::sanitize_cargo_env;
pub use error::{CrossError, Result};
