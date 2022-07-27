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
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};

const DATE_FORMAT: &[time::format_description::FormatItem<'_>] =
    time::macros::format_description!("[year]-[month]-[day]");
const TIME_FORMAT: &[time::format_description::FormatItem<'_>] =
    time::macros::format_description!("[hour]:[minute]:[second]");

///
#[rhai::plugin::export_module]
pub mod utils {

    use crate::modules::{types::types::SharedObject, EngineResult};
    use vsmtp_common::re::lettre;

    // TODO: not yet functional, the relayer cannot connect to servers.
    /// send a mail from a template.
    #[rhai_fn(return_raw)]
    pub fn send_mail(from: &str, to: rhai::Array, path: &str, relay: &str) -> EngineResult<()> {
        // TODO: email could be cached using an object. (obj mail "my_mail" "/path/to/mail")
        let email =
            std::fs::read_to_string(path).map_err::<Box<rhai::EvalAltResult>, _>(|err| {
                format!("failed to load email at {path}: {err:?}").into()
            })?;

        let envelop = lettre::address::Envelope::new(
            Some(from.parse().map_err::<Box<rhai::EvalAltResult>, _>(|err| {
                format!("sys::send_mail from parsing failed: {err:?}").into()
            })?),
            to.into_iter()
                // NOTE: address that couldn't be converted will be silently dropped.
                .filter_map(|rcpt| {
                    rcpt.try_cast::<String>()
                        .and_then(|s| s.parse::<lettre::Address>().map(Some).unwrap_or(None))
                })
                .collect(),
        )
        .map_err::<Box<rhai::EvalAltResult>, _>(|err| {
            format!("sys::send_mail envelop parsing failed {err:?}").into()
        })?;

        match lettre::Transport::send_raw(
            &lettre::SmtpTransport::relay(relay)
                .map_err::<Box<rhai::EvalAltResult>, _>(|err| {
                    format!("sys::send_mail failed to connect to relay: {err:?}").into()
                })?
                .build(),
            &envelop,
            email.as_bytes(),
        ) {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("sys::send_mail failed to send: {err:?}").into()),
        }
    }

    /// use the user cache to check if a user exists on the system.
    #[must_use]
    #[rhai_fn(global, name = "user_exist")]
    pub fn user_exist_str(name: &str) -> bool {
        super::user_exist(name)
    }

    /// use the user cache to check if a user exists on the system.
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "user_exist")]
    pub fn user_exist_obj(name: SharedObject) -> bool {
        super::user_exist(&name.to_string())
    }

    /// get the hostname of the machine.
    #[rhai_fn(return_raw)]
    pub fn hostname() -> EngineResult<String> {
        hostname::get()
            .map_err::<Box<rhai::EvalAltResult>, _>(|err| {
                format!("failed to get system's hostname: {err}").into()
            })?
            .to_str()
            .map_or(
                Err("the system's hostname is not UTF-8 valide".into()),
                |host| Ok(host.to_string()),
            )
    }

    /// get the current time.
    #[must_use]
    pub fn time() -> String {
        let now = time::OffsetDateTime::now_utc();

        now.format(&TIME_FORMAT)
            .unwrap_or_else(|_| String::default())
    }

    /// get the current date.
    #[must_use]
    pub fn date() -> String {
        let now = time::OffsetDateTime::now_utc();

        now.format(&DATE_FORMAT)
            .unwrap_or_else(|_| String::default())
    }
}

// TODO: use UsersCache to optimize user lookup.
fn user_exist(name: &str) -> bool {
    vsmtp_config::re::users::get_user_by_name(name).is_some()
}
