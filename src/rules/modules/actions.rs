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
        config::{log_channel::RULES, server_config::Service},
        rules::{rule_engine::Status, service::ServiceResult},
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

    pub fn log_error(message: &str) {
        log::error!(target: RULES, "{}", message);
    }

    pub fn log_warn(message: &str) {
        log::warn!(target: RULES, "{}", message);
    }

    pub fn log_info(message: &str) {
        log::info!(target: RULES, "{}", message);
    }

    pub fn log_debug(message: &str) {
        log::debug!(target: RULES, "{}", message);
    }

    pub fn log_trace(message: &str) {
        log::trace!(target: RULES, "{}", message);
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
            format!("MAIL action failed: {err:?}").into()
        })?;

        let envelop = lettre::address::Envelope::new(
            Some(from.parse().map_err::<Box<EvalAltResult>, _>(|err| {
                format!("MAIL action failed: {err:?}").into()
            })?),
            to.into_iter()
                // NOTE: address that couldn't be converted will be silently dropped.
                .flat_map(|rcpt| {
                    rcpt.try_cast::<String>()
                        .and_then(|s| s.parse::<lettre::Address>().map(Some).unwrap_or(None))
                })
                .collect(),
        )
        .map_err::<Box<EvalAltResult>, _>(|err| format!("MAIL action failed: {err:?}").into())?;

        println!("sending email");

        match lettre::Transport::send_raw(
            &lettre::SmtpTransport::relay(relay)
                .map_err::<Box<EvalAltResult>, _>(|err| {
                    format!("MAIL action failed: {err:?}").into()
                })?
                .build(),
            &envelop,
            email.as_bytes(),
        ) {
            Ok(_) => {
                println!("email has been sent");
                Ok(())
            }
            Err(err) => {
                println!("email not sent");
                Err(format!("MAIL action failed: {err:?}").into())
            }
        }
    }

    // TODO: use UsersCache to optimize user lookup.
    /// use the user cache to check if a user exists on the system.
    pub(crate) fn user_exist(name: &str) -> bool {
        users::get_user_by_name(name).is_some()
    }

    // #[rhai_fn(name = "__LOOKUP_MAIL_FROM", return_raw)]
    // /// check the client's ip matches against the hostname passed has parameter.
    // /// this can be used, for example, to check if MAIL FROM's value
    // /// is matching the connection, preventing relaying.
    // pub fn lookup_mail_from(
    //     // curried parameters.
    //     connect: std::net::IpAddr,
    //     port: u16,
    //     // exposed parameter.
    //     hostname: &str,
    // ) -> Result<bool, Box<EvalAltResult>> {
    //     if hostname.is_empty() {
    //         return Err(
    //             "the LOOKUP_MAIL_FROM action can only be called after or in the 'mail' stage."
    //                 .into(),
    //         );
    //     }

    //     let engine = acquire_engine();
    //     let objects = engine.objects.read().unwrap();

    //     let hostname = match objects.get(hostname) {
    //         Some(Object::Fqdn(fqdn)) => fqdn.as_str(),
    //         _ => hostname,
    //     };

    //     Ok(format!("{}:{}", hostname, port)
    //         .to_socket_addrs()
    //         .map_err::<Box<EvalAltResult>, _>(|error| {
    //             format!("couldn't process dns lookup: {}", error).into()
    //         })?
    //         .any(|socket| socket.ip() == connect))
    // }

    #[rhai_fn(global, return_raw)]
    pub fn run(
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
