//! vSMTP rule engine

#![doc(html_no_source)]
//
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
//
#![allow(clippy::doc_markdown)]

mod log_channels {
    /// server's rule
    pub const RE: &str = "server::rule_engine";
    pub const SERVICES: &str = "server::rule_engine::services";
}

mod dsl;
mod error;
pub mod modules;
pub mod rule_engine;
pub mod rule_state;
mod server_api;

#[cfg(test)]
mod tests;
