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
use rhai::{exported_module, EvalAltResult};

pub(crate) mod actions;
pub(crate) mod mail_context;
pub(crate) mod types;

pub(crate) type EngineResult<T> = Result<T, Box<EvalAltResult>>;

rhai::def_package! {
    /// vsl's standard api.
    pub StandardVSLPackage(module) {
        rhai::packages::StandardPackage::init(module);

        module.combine(exported_module!(super::modules::actions::bcc::bcc))
            .combine(exported_module!(super::modules::actions::headers::headers))
            .combine(exported_module!(super::modules::actions::logging::logging))
            .combine(exported_module!(super::modules::actions::rule_state::rule_state))
            .combine(exported_module!(super::modules::actions::security::security))
            .combine(exported_module!(super::modules::actions::services::services))
            .combine(exported_module!(super::modules::actions::transports::transports))
            .combine(exported_module!(super::modules::actions::utils::utils))
            .combine(exported_module!(super::modules::actions::write::write))
            .combine(exported_module!(super::modules::types::types))
            .combine(exported_module!(super::modules::mail_context::mail_context));
    }
}
