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
    plugin::{mem, FnAccess, FnNamespace, Module, PluginFunction, RhaiResult, TypeId},
    Dynamic, EvalAltResult, NativeCallContext,
};

#[rhai::plugin::export_module]
pub mod services {

    use crate::dsl::service::shell::run;
    use crate::dsl::service::shell::ShellResult;
    use crate::dsl::service::Service;
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
    #[rhai_fn(global, name = "run_shell", return_raw, pure)]
    pub fn run_shell(service: &mut std::sync::Arc<Service>) -> EngineResult<ShellResult> {
        if let Service::UnixShell {
            timeout,
            user,
            group,
            command,
            args,
        } = &**service
        {
            run(timeout, command, user, group, args)
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())
        } else {
            Err("{service} cannot be run as a shell script.".into())
        }
    }

    /// execute the given shell service with dynamic arguments.
    #[rhai_fn(global, name = "run_shell", return_raw, pure)]
    pub fn run_shell_with_args(
        service: &mut std::sync::Arc<Service>,
        args: rhai::Array,
    ) -> EngineResult<ShellResult> {
        if let Service::UnixShell {
            timeout,
            user,
            group,
            command,
            ..
        } = &**service
        {
            let args = args
                .into_iter()
                .map(rhai::Dynamic::try_cast)
                .collect::<Option<Vec<String>>>()
                .ok_or_else::<Box<EvalAltResult>, _>(|| {
                    "all shell arguments must be strings".into()
                })?;
            run(timeout, command, user, group, &Some(args))
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())
        } else {
            Err("{service} cannot be run as a shell script.".into())
        }
    }
}
