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

use crate::dsl::service::ServiceResult;
use crate::log_channels;
use crate::{dsl::service::Service, modules::EngineResult};
use rhai::EvalAltResult;
use vsmtp_common::re::{anyhow, log};
use vsmtp_config::re::users;

pub fn parse_shell_service(
    context: &mut rhai::EvalContext,
    input: &[rhai::Expression],
    service_name: &str,
) -> EngineResult<Service> {
    let options: rhai::Map = context
        .eval_expression_tree(&input[3])?
        .try_cast()
        .ok_or_else::<Box<rhai::EvalAltResult>, _>(|| {
            "shell service options must be a map".into()
        })?;

    for key in ["timeout", "command", "args"] {
        if !options.contains_key(key) {
            return Err(
                format!("shell service {service_name} is missing the '{key}' option.").into(),
            );
        }
    }

    let timeout: std::time::Duration = options
        .get("timeout")
        .unwrap()
        .to_string()
        .parse::<vsmtp_config::re::humantime::Duration>()
        .map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())?
        .into();
    let command = options.get("command").unwrap().to_string();
    let args = options.get("args").unwrap().to_string();

    Ok(Service::UnixShell {
        timeout,
        user: None,
        group: None,
        command,
        args: Some(args),
    })
}

/// run a shell service.
/// # Errors
///
/// * if the user used to launch commands is not found.
/// * if the group used to launch commands is not found.
/// * if the shell service failed to spawn.
/// * if the shell returned an error.
pub fn run(service: &Service) -> anyhow::Result<ServiceResult> {
    if let Service::UnixShell {
        timeout,
        command,
        user,
        group,
        ..
    } = service
    {
        let mut child = std::process::Command::new(command);

        if let Some(user_name) = user {
            if let Some(user) = users::get_user_by_name(&user_name) {
                std::os::unix::prelude::CommandExt::uid(&mut child, user.uid());
            } else {
                anyhow::bail!("user not found: '{user_name}'")
            }
        }
        if let Some(group_name) = group {
            if let Some(group) = users::get_group_by_name(group_name) {
                std::os::unix::prelude::CommandExt::gid(&mut child, group.gid());
            } else {
                anyhow::bail!("group not found: '{group_name}'")
            }
        }

        log::trace!(
            target: log_channels::SERVICES,
            "shell running command: {:#?}",
            child
        );

        let mut child = match child.spawn() {
            Ok(child) => child,
            Err(err) => anyhow::bail!("shell process failed to spawn: {err:?}"),
        };

        let status = match wait_timeout::ChildExt::wait_timeout(&mut child, *timeout) {
            Ok(status) => status.unwrap_or_else(|| {
                child.kill().expect("child has already exited");
                child.wait().expect("command wasn't running")
            }),

            Err(err) => anyhow::bail!("shell unexpected error: {err:?}"),
        };

        Ok(ServiceResult::new(status))
    } else {
        anyhow::bail!("only a shell service can use the 'run' function")
    }
}
