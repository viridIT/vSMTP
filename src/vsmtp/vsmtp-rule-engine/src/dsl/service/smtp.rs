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

use crate::log_channels;
use crate::{dsl::service::Service, modules::EngineResult};
use lettre::AsyncTransport;
use rhai::EvalAltResult;
use vsmtp_common::rcpt::Rcpt;
use vsmtp_common::re::{anyhow, log};
use vsmtp_common::Address;
use vsmtp_config::re::users;

pub fn parse_smtp_service(
    context: &mut rhai::EvalContext,
    input: &[rhai::Expression],
    service_name: &str,
) -> EngineResult<Service> {
    let options: rhai::Map = context
        .eval_expression_tree(&input[3])?
        .try_cast()
        .ok_or_else::<Box<rhai::EvalAltResult>, _>(|| {
            "smtp service options must be a map".into()
        })?;

    for key in ["target"] {
        if !options.contains_key(key) {
            return Err(
                format!("smtp service '{service_name}' is missing the '{key}' option.").into(),
            );
        }
    }

    // TODO: add a 'unix'/'net' modifier.
    // TODO: add tls options. (is it really that useful in case of an antivirus ?)
    let target = options.get("target").unwrap().to_string();
    let timeout: std::time::Duration = options
        .get("timeout")
        .get_or_insert(&rhai::Dynamic::from("60s"))
        .to_string()
        .parse::<vsmtp_config::re::humantime::Duration>()
        .map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())?
        .into();

    Ok(Service::Smtp {
        transport: {
            lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::builder_dangerous(target)
                .timeout(Some(timeout))
                .build()
        },
    })
}

/// delegate security handling via the smtp protocol.
pub async fn delegate(
    transport: &lettre::AsyncSmtpTransport<lettre::Tokio1Executor>,
    from: &Address,
    to: &[Rcpt],
    email: &str,
) {
    log::trace!(
        target: log_channels::SERVICES,
        "delegation on: {:#?}",
        "target"
    );

    // transport.send_raw(envelope, email).await
}
