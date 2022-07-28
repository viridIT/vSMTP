//! vSMTP rule engine
//!
//! vSMTP server is built with a runtime also called `rule engine`.
//! This runtime can execute code provided in our superset of the
//! <https://rhai.rs> language.
//!
//! The viridIT scripting language (vsl) is a simple language that allow
//! you to define `rules` and `object` to control the traffic on your MTA.
//!
//! Further details on the official book of vSMTP: <https://vsmtp.rs/reference/vSL/vsl.html>

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
mod rule_engine;
mod rule_state;
mod server_api;

pub use rule_engine::RuleEngine;
pub use rule_state::RuleState;

#[cfg(test)]
mod tests;

/// Module containing the backend for the vsl's Rust API.
pub mod api {
    use crate::server_api::ServerAPI;
    use vsmtp_common::{mail_context::MailContext, MessageBody};

    /// Error produced by the vsl's Rust API function calls.
    pub type EngineResult<T> = Result<T, Box<rhai::EvalAltResult>>;
    /// Alias for `ctx()`
    pub type Context = std::sync::Arc<std::sync::RwLock<MailContext>>;
    /// Alias for `msg()`
    pub type Message = std::sync::Arc<std::sync::RwLock<MessageBody>>;
    /// Alias for `srv()`
    pub type Server = std::sync::Arc<ServerAPI>;
    /// Alias for any other object defined using the `object` keyword.
    pub type SharedObject = std::sync::Arc<Object>;

    pub use super::dsl::object::Object;
    pub use super::dsl::service::Service;

    /// module containing the backend for DKIM functionality.
    pub mod dkim;
    /// this module allow you to write information to a specific file.
    pub mod logging;
    /// Extensions for the `MailContext` type.
    pub mod mail_context;
    /// Extensions for the `MessageBody` type.
    pub mod message;
    /// Extensions for the `MessageBody` type.
    pub mod message_parsed;
    /// module related to the state engine.
    pub mod rule_state;
    /// module containing the API for [`crate::api::Service`]
    pub mod services;
    /// module containing the backend for SPF functionality.
    pub mod spf;
    /// module containing the API for the delivery methods.
    pub mod transports;
    /// Getter for common types
    pub mod types;
    /// module containing utility methods.
    pub mod utils;
    /// module containing the API for the writing of the message on disk.
    pub mod write;

    rhai::def_package! {
        /// vsl's standard api.
        pub StandardVSLPackage(module) {
            rhai::packages::StandardPackage::init(module);

            module
                .combine(rhai::exported_module!(logging))
                .combine(rhai::exported_module!(dkim))
                .combine(rhai::exported_module!(rule_state))
                .combine(rhai::exported_module!(spf))
                .combine(rhai::exported_module!(services))
                .combine(rhai::exported_module!(transports))
                .combine(rhai::exported_module!(utils))
                .combine(rhai::exported_module!(write))
                .combine(rhai::exported_module!(types))
                .combine(rhai::exported_module!(mail_context))
                .combine(rhai::exported_module!(message))
                .combine(rhai::exported_module!(message_parsed));
        }
    }

    #[cfg(test)]
    mod test {
        use vsmtp_common::mail_context::{ConnectionContext, MailContext};

        pub fn get_default_context() -> MailContext {
            MailContext {
                connection: ConnectionContext {
                    timestamp: std::time::SystemTime::now(),
                    credentials: None,
                    is_authenticated: false,
                    is_secured: false,
                    server_name: "testserver.com".to_string(),
                    server_address: "127.0.0.1:25".parse().unwrap(),
                },
                client_addr: "0.0.0.0:0".parse().unwrap(),
                envelop: vsmtp_common::envelop::Envelop::default(),
                metadata: Some(vsmtp_common::mail_context::MessageMetadata {
                    timestamp: std::time::SystemTime::now(),
                    ..vsmtp_common::mail_context::MessageMetadata::default()
                }),
            }
        }
    }
}
