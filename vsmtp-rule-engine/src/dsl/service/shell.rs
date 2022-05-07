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

use crate::{dsl::service::Service, modules::EngineResult};

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

    let timeout = options.get("timeout").unwrap().to_string();
    let command = options.get("command").unwrap().to_string();
    let args = options.get("args").unwrap().to_string();

    Ok(Service::UnixShell {
        timeout: std::time::Duration::from_secs(1),
        user: None,
        group: None,
        command,
        args: Some(args),
    })
}
