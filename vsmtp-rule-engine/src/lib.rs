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
/// vsl prelude
pub mod modules;
mod obj;
/// rust api
pub mod rule_engine;
pub mod rule_state;
mod server_api;
mod service;

#[cfg(test)]
mod tests;
