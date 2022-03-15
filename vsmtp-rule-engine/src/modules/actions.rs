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
use crate::modules::mail_context::mail_context::message_id;
use crate::modules::EngineResult;
use crate::obj::Object;
use crate::server_api::ServerAPI;
use crate::service::ServiceResult;
use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, Position, RhaiResult, TypeId,
};
use vsmtp_common::address::Address;
use vsmtp_common::mail_context::Body;
use vsmtp_common::mail_context::MailContext;
use vsmtp_common::status::Status;
use vsmtp_config::log_channel::URULES;
use vsmtp_config::service::Service;

#[doc(hidden)]
#[allow(dead_code)]
#[rhai::plugin::export_module]
pub mod actions {

    /// the transaction if forced accepted, skipping rules of next stages and going the pre-queue
    #[must_use]
    pub const fn faccept() -> Status {
        Status::Faccept
    }

    /// the transaction if accepted, skipping rules to the next stage
    #[must_use]
    pub const fn accept() -> Status {
        Status::Accept
    }

    /// the transaction continue to execute rule for that stage
    #[must_use]
    pub const fn next() -> Status {
        Status::Next
    }

    /// the transaction is denied, reply error to clients
    #[must_use]
    pub const fn deny() -> Status {
        Status::Deny
    }

    ///
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
                .filter_map(|rcpt| {
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
    #[must_use]
    pub fn user_exist(name: &str) -> bool {
        users::get_user_by_name(name).is_some()
    }

    /// execute the service named @service_name from the vSMTP configuration definition
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, return_raw)]
    pub fn run_service(
        this: &mut std::sync::Arc<std::sync::RwLock<ServerAPI>>,
        ctx: std::sync::Arc<std::sync::RwLock<MailContext>>,
        service_name: &str,
    ) -> EngineResult<ServiceResult> {
        let server = this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

        crate::service::run(
            server
                .config
                .rules
                .services
                .iter()
                .find(|s| match s {
                    Service::UnixShell { name, .. } => name == service_name,
                })
                .ok_or_else::<Box<EvalAltResult>, _>(|| {
                    format!("No service in config named: '{service_name}'").into()
                })?,
            &*ctx
                .read()
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?,
        )
        .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())
    }

    /// change the sender of the mail
    #[rhai_fn(global, return_raw)]
    pub fn rewrite_mail_from(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        new_addr: &str,
    ) -> EngineResult<()> {
        let new_addr = Address::try_from(new_addr).map_err::<Box<EvalAltResult>, _>(|_| {
            format!(
                "could not rewrite mail_from with '{}' because it is not valid address",
                new_addr,
            )
            .into()
        })?;

        let email = &mut this
            .write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

        email.envelop.mail_from = new_addr.clone();

        match &mut email.body {
            Body::Empty => Err("failed to rewrite mail_from: the email has not been received yet. Use this method in postq or later.".into()),
            Body::Raw(_) => Err("failed to rewrite mail_from: the email has not been parsed yet. Use this method in postq or later.".into()),
            Body::Parsed(body) => {
                body.rewrite_mail_from(new_addr.full());
                Ok(())
            },
        }
    }

    /// change a recipient of the mail
    #[rhai_fn(global, return_raw)]
    pub fn rewrite_rcpt(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        old_addr: &str,
        new_addr: &str,
    ) -> EngineResult<()> {
        let old_addr = Address::try_from(old_addr).map_err::<Box<EvalAltResult>, _>(|_| {
            format!(
                "could not rewrite address '{}' because it is not valid address",
                old_addr,
            )
            .into()
        })?;

        let new_addr = Address::try_from(new_addr).map_err::<Box<EvalAltResult>, _>(|_| {
            format!(
                "could not rewrite address '{}' with '{}' because it is not valid address",
                old_addr, new_addr,
            )
            .into()
        })?;

        let email = &mut this
            .write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

        if let Body::Empty | Body::Raw(_) = &email.body {
            return Err("failed to rewrite rcpt: the email has not been received or parsed yet. Use this method in postq or later.".into());
        }

        if let Some(index) = email
            .envelop
            .rcpt
            .iter()
            .position(|rcpt| rcpt.address == old_addr)
        {
            email
                .envelop
                .rcpt
                .push(vsmtp_common::rcpt::Rcpt::new(new_addr.clone()));
            email.envelop.rcpt.swap_remove(index);

            match &mut email.body {
                Body::Parsed(body) => {
                    body.rewrite_rcpt(old_addr.full(), new_addr.full());
                    Ok(())
                }
                _ => unreachable!(),
            }
        } else {
            Err(format!(
                "could not rewrite address '{}' because it does not resides in rcpt.",
                old_addr
            )
            .into())
        }
    }

    /// add a recipient to the mail
    #[rhai_fn(global, return_raw)]
    pub fn add_rcpt(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        new_addr: &str,
    ) -> EngineResult<()> {
        let new_addr = Address::try_from(new_addr).map_err(|_| {
            format!(
                "'{}' could not be converted to a valid rcpt address",
                new_addr
            )
        })?;

        let email = &mut this
            .write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

        email
            .envelop
            .rcpt
            .push(vsmtp_common::rcpt::Rcpt::new(new_addr.clone()));

        match &mut email.body {
            Body::Empty => Err("failed to rewrite rcpt: the email has not been received yet. Use this method in postq or later.".into()),
            Body::Raw(_) => Err("failed to rewrite rcpt: the email has not been parsed yet. Use this method in postq or later.".into()),
            Body::Parsed(body) => {
                body.add_rcpt(new_addr.full());
                Ok(())
            },
        }
    }

    /// remove a recipient to the mail
    #[rhai_fn(global, return_raw)]
    pub fn remove_rcpt(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        addr: &str,
    ) -> EngineResult<()> {
        let addr = Address::try_from(addr)
            .map_err(|_| format!("{} could not be converted to a valid rcpt address", addr))?;

        let email = &mut this
            .write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

        if let Body::Empty | Body::Raw(_) = &email.body {
            return Err("failed to remove rcpt: the email has not been received or parsed yet. Use this method in postq or later.".into());
        }

        if let Some(index) = email
            .envelop
            .rcpt
            .iter()
            .position(|rcpt| rcpt.address == addr)
        {
            email.envelop.rcpt.remove(index);
            match &mut email.body {
                Body::Parsed(body) => body.remove_rcpt(addr.full()),
                _ => unreachable!(),
            };
            Ok(())
        } else {
            Err(format!(
                "could not remove address '{}' because it does not resides in rcpt.",
                addr
            )
            .into())
        }
    }

    /// write the current email to a specified folder.
    #[rhai_fn(global, return_raw)]
    pub fn write(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        dir: &str,
    ) -> EngineResult<()> {
        match std::fs::OpenOptions::new().create(true).write(true).open(
            std::path::PathBuf::from_iter([dir, &format!("{}.eml", message_id(this)?)]),
        ) {
            Ok(file) => {
                let mut writer = std::io::LineWriter::new(file);

                match &this
                    .read()
                    .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
                    .body
                {
                    Body::Empty => {
                        return Err(
                            "failed to write email: the body has not been received yet.".into()
                        )
                    }
                    Body::Raw(raw) => std::io::Write::write_all(&mut writer, raw.as_bytes()),
                    Body::Parsed(email) => {
                        std::io::Write::write_all(&mut writer, email.to_raw().as_bytes())
                    }
                }
            }
            .map_err(|err| format!("failed to write email at '{}': {:?}", dir, err).into()),
            Err(err) => Err(format!("failed to write email at '{}': {:?}", dir, err).into()),
        }
    }

    /// write the content of the current email in a json file.
    /// NOTE: it would be great not having all those 'map_err'.
    #[rhai_fn(global, return_raw)]
    pub fn dump(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        dir: &str,
    ) -> EngineResult<()> {
        match std::fs::OpenOptions::new().create(true).write(true).open(
            std::path::PathBuf::from_iter([dir, &format!("{}.dump.json", message_id(this)?)]),
        ) {
            Ok(mut file) => std::io::Write::write_all(
                &mut file,
                serde_json::to_string_pretty(
                    &*this
                        .read()
                        .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?,
                )
                .map_err::<Box<EvalAltResult>, _>(|err| {
                    format!("failed to dump email at '{}': {:?}", dir, err).into()
                })?
                .as_bytes(),
            )
            .map_err(|err| format!("failed to dump email at '{}': {:?}", dir, err).into()),
            Err(err) => Err(format!("failed to dump email at '{}': {:?}", dir, err).into()),
        }
    }

    // TODO: unfinished, queue parameter should point to a folder specified in toml config.
    /// dump the current email into a quarantine queue, skipping delivery.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, return_raw)]
    pub fn quarantine(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        queue: &str,
    ) -> EngineResult<()> {
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(queue)
        {
            Ok(mut file) => {
                disable_delivery(this)?;

                std::io::Write::write_all(
                    &mut file,
                    serde_json::to_string_pretty(&*this.read().map_err::<Box<EvalAltResult>, _>(
                        |err| format!("failed to dump email: {err:?}").into(),
                    )?)
                    .map_err::<Box<EvalAltResult>, _>(|err| {
                        format!("failed to dump email: {err:?}").into()
                    })?
                    .as_bytes(),
                )
                .map_err(|err| format!("failed to dump email: {err:?}").into())
            }
            Err(err) => Err(format!("failed to dump email: {err:?}").into()),
        }
    }

    /// set the delivery method
    #[rhai_fn(global, return_raw)]
    pub fn deliver(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        resolver: &str,
    ) -> EngineResult<()> {
        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .metadata
            .as_mut()
            .ok_or_else::<Box<EvalAltResult>, _>(|| {
                "metadata are not available in this stage".into()
            })?
            .resolver = resolver.to_string();

        Ok(())
    }

    /// remove the delivery method
    #[rhai_fn(global, return_raw)]
    pub fn disable_delivery(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
    ) -> EngineResult<()> {
        deliver(this, "none")
    }

    /// check if a given header exists in the top level headers.
    #[rhai_fn(global, return_raw, pure)]
    pub fn has_header(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        header: &str,
    ) -> EngineResult<bool> {
        Ok(
            match &this
                .read()
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
                .body
            {
                Body::Empty => false,
                Body::Raw(raw) => {
                    let mut headers_end = 0;

                    // getting headers from the raw email.
                    for line in raw.lines() {
                        let mut split = line.splitn(2, ':');
                        match (split.next(), split.next()) {
                            // adding one to the index because `\n` is striped using the Lines iterator.
                            (Some(_), Some(_)) => headers_end += line.len() + 1,
                            _ => break,
                        }
                    }

                    raw[0..headers_end].contains(format!("{}: ", header).as_str())
                }
                Body::Parsed(email) => email.headers.iter().any(|(name, _)| header == name),
            },
        )
    }

    /// add a header to the raw or parsed email contained in ctx.
    #[rhai_fn(global, return_raw)]
    pub fn add_header(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        header: &str,
        value: &str,
    ) -> EngineResult<()> {
        match &mut this
            .write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .body
        {
            Body::Empty => {
                return Err(format!(
                    "failed to add header '{}': the body has not been received yet.",
                    header
                )
                .into())
            }
            Body::Raw(raw) => *raw = format!("{}: {}\n{}", header, value, raw),
            Body::Parsed(email) => email.headers.push((header.to_string(), value.to_string())),
        };

        Ok(())
    }

    /// add a recipient to the list recipient using a raw string.
    #[rhai_fn(global, name = "bcc", return_raw)]
    pub fn bcc_str(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        bcc: &str,
    ) -> EngineResult<()> {
        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .rcpt
            .push(vsmtp_common::rcpt::Rcpt::new(
                Address::try_from(bcc).map_err(|_| {
                    format!("'{}' could not be converted to a valid rcpt address", bcc)
                })?,
            ));

        Ok(())
    }

    /// add a recipient to the list recipient using an address.
    #[rhai_fn(global, name = "bcc", return_raw)]
    pub fn bcc_addr(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        bcc: Address,
    ) -> EngineResult<()> {
        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .rcpt
            .push(vsmtp_common::rcpt::Rcpt::new(bcc));

        Ok(())
    }

    /// add a recipient to the list recipient using an object.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "bcc", return_raw)]
    pub fn bcc_object(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        bcc: std::sync::Arc<Object>,
    ) -> EngineResult<()> {
        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .rcpt
            .push(match &*bcc {
                Object::Address(addr) => vsmtp_common::rcpt::Rcpt::new(addr.clone()),
                Object::Str(string) => vsmtp_common::rcpt::Rcpt::new(
                    Address::try_from(string.as_str()).map_err(|_| {
                        format!(
                            "'{}' could not be converted to a valid rcpt address",
                            string
                        )
                    })?,
                ),
                other => {
                    return Err(format!(
                        "'{}' could not be converted to a valid rcpt address",
                        other.to_string()
                    )
                    .into())
                }
            });

        Ok(())
    }
}
