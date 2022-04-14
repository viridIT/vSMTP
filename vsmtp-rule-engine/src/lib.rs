//! vSMTP rule engine

#![doc(html_no_source)]
#![deny(missing_docs)]
#![deny(unsafe_code)]
//
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
//
#![allow(clippy::doc_markdown)]

mod dsl;
mod error;
/// vsl prelude
pub mod modules;
mod obj;
/// rust api
pub mod rule_engine;
mod server_api;
mod service;

#[cfg(test)]
mod tests;
