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

mod actions;
mod errors;

pub use actions::{dkim, logging, rule_state, security, services, transports, utils, write};
pub type EngineResult<T> = Result<T, Box<rhai::EvalAltResult>>;

/// Extensions for the `MailContext` type.
pub mod mail_context;
/// Extensions for the `MessageBody` type.
pub mod message;
/// Getter for common types
pub mod types;

rhai::def_package! {
    /// vsl's standard api.
    pub StandardVSLPackage(module) {
        rhai::packages::StandardPackage::init(module);

        module
            .combine(rhai::exported_module!(actions::logging))
            .combine(rhai::exported_module!(actions::dkim))
            .combine(rhai::exported_module!(actions::rule_state::rule_state))
            .combine(rhai::exported_module!(actions::security::security))
            .combine(rhai::exported_module!(actions::services::services))
            .combine(rhai::exported_module!(actions::transports::transports))
            .combine(rhai::exported_module!(actions::utils::utils))
            .combine(rhai::exported_module!(actions::write::write))
            .combine(rhai::exported_module!(types))
            .combine(rhai::exported_module!(mail_context))
            .combine(rhai::exported_module!(message::message))
            .combine(rhai::exported_module!(message::message_calling_parse));
    }
}
