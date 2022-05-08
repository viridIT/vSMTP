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
use rhai::{
    plugin::{FnAccess, FnNamespace, Module, PluginFunction, RhaiResult, TypeId},
    Dynamic, EvalAltResult, NativeCallContext,
};

#[rhai::plugin::export_module]
pub mod services {

    use crate::dsl::service::shell::run;
    use crate::dsl::service::Service;
    use crate::dsl::service::ServiceResult;
    use crate::modules::EngineResult;

    #[rhai_fn(global, pure)]
    pub fn to_string(service: &mut std::sync::Arc<Service>) -> String {
        service.to_string()
    }

    #[rhai_fn(global, pure)]
    pub fn to_debug(service: &mut std::sync::Arc<Service>) -> String {
        format!("{service:#?}")
    }

    /// execute the given shell service.
    #[rhai_fn(global, return_raw, pure)]
    pub fn run_shell(service: &mut std::sync::Arc<Service>) -> EngineResult<ServiceResult> {
        run(service).map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())
    }
}
