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

pub(crate) mod actions;
pub(crate) mod mail_context;
pub(crate) mod message;
pub(crate) mod types;

pub use actions::*;

pub(crate) type EngineResult<T> = Result<T, Box<rhai::EvalAltResult>>;

#[doc(hidden)]
mod inner {
    use super::{actions, mail_context, message, types};

    rhai::def_package! {
        /// vsl's standard api.
        pub StandardVSLPackage(module) {
            rhai::packages::StandardPackage::init(module);

            module.combine(rhai::exported_module!(actions::bcc::bcc))
                .combine(rhai::exported_module!(actions::logging::logging))
                .combine(rhai::exported_module!(actions::rule_state::rule_state))
                .combine(rhai::exported_module!(actions::security::security))
                .combine(rhai::exported_module!(actions::services::services))
                .combine(rhai::exported_module!(actions::transports::transports))
                .combine(rhai::exported_module!(actions::utils::utils))
                .combine(rhai::exported_module!(actions::write::write))
                .combine(rhai::exported_module!(types::types))
                .combine(rhai::exported_module!(mail_context::mail_context))
                .combine(rhai::exported_module!(message::message))
                .combine(rhai::exported_module!(message::message_calling_parse));

            }
    }
}

pub use inner::StandardVSLPackage;
