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

mod dsl {
    pub mod action;
    pub mod delegation;
    pub mod directives;
    pub mod object;
    pub mod rule;
    pub mod service;
}

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
