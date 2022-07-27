//! vSMTP rule engine

/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/

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
mod modules;
mod rule_engine;
mod rule_state;
mod server_api;

pub use dsl::object::Object;
pub use dsl::service::Service;
pub use modules::{
    dkim, logging, mail_context, message, rule_state as state, security, services, transports,
    types, utils, write,
};
pub use rule_engine::RuleEngine;
pub use rule_state::RuleState;

#[cfg(test)]
mod tests;
