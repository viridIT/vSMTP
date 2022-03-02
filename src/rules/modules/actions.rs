/**
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
**/
use rhai::plugin::*;

#[allow(dead_code)]
#[export_module]
pub mod actions {

    use crate::{
        config::{
            log_channel::URULES,
            service::{Service, ServiceResult},
        },
        rules::rule_engine::Status,
        smtp::mail::MailContext,
    };

    pub fn faccept() -> Status {
        Status::Faccept
    }

    pub fn accept() -> Status {
        Status::Accept
    }

    pub fn next() -> Status {
        Status::Next
    }

    pub fn deny() -> Status {
        Status::Deny
    }

    pub fn log(level: &str, message: &str) {
        match level {
            "trace" => log::trace!(target: URULES, "{}", message),
            "debug" => log::debug!(target: URULES, "{}", message),
            "info" => log::info!(target: URULES, "{}", message),
            "warn" => log::warn!(target: URULES, "{}", message),
            "error" => log::error!(target: URULES, "{}", message),
            unknown => log::warn!(
                target: URULES,
                "'{}' is not a valid log level. Original message: '{}'",
                unknown,
                message
            ),
        }
    }

    // TODO: not yet functional, the relayer cannot connect to servers.
    /// send a mail from a template.
    #[rhai_fn(return_raw)]
    pub fn send_mail(
        from: &str,
        to: rhai::Array,
        path: &str,
        relay: &str,
    ) -> Result<(), Box<EvalAltResult>> {
        // TODO: email could be cached using an object. (obj mail "my_mail" "/path/to/mail")
        let email = std::fs::read_to_string(path).map_err::<Box<EvalAltResult>, _>(|err| {
            format!("vsl::send_mail failed, email path to send unavailable: {err:?}").into()
        })?;

        let envelop = lettre::address::Envelope::new(
            Some(from.parse().map_err::<Box<EvalAltResult>, _>(|err| {
                format!("vsl::send_mail from parsing failed: {err:?}").into()
            })?),
            to.into_iter()
                // NOTE: address that couldn't be converted will be silently dropped.
                .flat_map(|rcpt| {
                    rcpt.try_cast::<String>()
                        .and_then(|s| s.parse::<lettre::Address>().map(Some).unwrap_or(None))
                })
                .collect(),
        )
        .map_err::<Box<EvalAltResult>, _>(|err| {
            format!("vsl::send_mail envelop parsing failed {err:?}").into()
        })?;

        match lettre::Transport::send_raw(
            &lettre::SmtpTransport::relay(relay)
                .map_err::<Box<EvalAltResult>, _>(|err| {
                    format!("vsl::send_mail failed to connect to relay: {err:?}").into()
                })?
                .build(),
            &envelop,
            email.as_bytes(),
        ) {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("vsl::send_mail failed to send: {err:?}").into()),
        }
    }

    // TODO: use UsersCache to optimize user lookup.
    /// use the user cache to check if a user exists on the system.
    pub(crate) fn user_exist(name: &str) -> bool {
        users::get_user_by_name(name).is_some()
    }

    #[rhai_fn(global, return_raw)]
    pub(crate) fn run(
        services: &mut std::sync::Arc<Vec<Service>>,
        service_name: &str,
        ctx: std::sync::Arc<std::sync::RwLock<MailContext>>,
    ) -> Result<ServiceResult, Box<EvalAltResult>> {
        services
            .iter()
            .find(|s| match s {
                Service::UnixShell { name, .. } => name == service_name,
            })
            .ok_or_else::<Box<EvalAltResult>, _>(|| {
                format!("No service in config named: '{service_name}'").into()
            })?
            .run(ctx)
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())
    }
}
