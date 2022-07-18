//! vSMTP rule engine

#![doc(html_no_source)]
#![deny(missing_docs)]
#![forbid(unsafe_code)]
//
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
//
#![allow(clippy::use_self)]

mod log_channels {
    /// server's rule
    pub const RE: &str = "server::rule_engine";
    pub const SERVICES: &str = "server::rule_engine::services";
}

mod dsl;
#[macro_use]
mod error;
mod server_api;

///
pub mod modules;
///
pub mod rule_engine;
///
pub mod rule_state;

pub use dsl::service::Service;

#[cfg(test)]
mod tests;
