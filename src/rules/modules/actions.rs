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
        config::log_channel::URULES,
        config::service::{Service, ServiceResult},
        resolver::MailContext,
        rules::{
            address::Address, modules::mail_context::mail_context::message_id,
            modules::EngineResult, obj::Object, rule_engine::Status, server_api::ServerAPI,
        },
        smtp::mail::Body,
    };
    use std::io::Write;

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
    pub fn run_service(
        this: &mut std::sync::Arc<std::sync::RwLock<ServerAPI>>,
        ctx: std::sync::Arc<std::sync::RwLock<MailContext>>,
        service_name: &str,
    ) -> EngineResult<ServiceResult> {
        let server = this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

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
            })?
            .run(
                &*ctx
                    .read()
                    .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?,
            )
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())
    }

    #[rhai_fn(global, return_raw)]
    pub fn rewrite_mail_from(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        addr: String,
    ) -> EngineResult<()> {
        let addr = Address::new(&addr).map_err::<Box<EvalAltResult>, _>(|_| {
            format!(
                "could not rewrite mail_from with '{}' because it is not valid address",
                addr,
            )
            .into()
        })?;

        let email = &mut this
            .write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

        email.envelop.mail_from = addr.clone();

        match &mut email.body {
            Body::Empty => Err("failed to rewrite mail_from: the email has not been received yet. Use this method in postq or later.".into()),
            Body::Raw(_) => Err("failed to rewrite mail_from: the email has not been parsed yet. Use this method in postq or later.".into()),
            Body::Parsed(body) => {
                body.rewrite_mail_from(addr.full());
                Ok(())
            },
        }
    }

    #[rhai_fn(global, return_raw)]
    pub fn rewrite_rcpt(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        index: String,
        addr: String,
    ) -> EngineResult<()> {
        let index = Address::new(&index).map_err::<Box<EvalAltResult>, _>(|_| {
            format!(
                "could not rewrite address '{}' because it is not valid address",
                index,
            )
            .into()
        })?;

        let addr = Address::new(&addr).map_err::<Box<EvalAltResult>, _>(|_| {
            format!(
                "could not rewrite address '{}' with '{}' because it is not valid address",
                index, addr,
            )
            .into()
        })?;

        let email = &mut this
            .write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

        email.envelop.rcpt.remove(&index);
        email.envelop.rcpt.insert(addr.clone());

        match &mut email.body {
            Body::Empty => Err("failed to rewrite rcpt: the email has not been received yet. Use this method in postq or later.".into()),
            Body::Raw(_) => Err("failed to rewrite rcpt: the email has not been parsed yet. Use this method in postq or later.".into()),
            Body::Parsed(body) => {
                body.rewrite_rcpt(index.full(), addr.full());
                Ok(())
            },
        }
    }

    #[rhai_fn(global, return_raw)]
    pub fn add_rcpt(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        s: String,
    ) -> EngineResult<()> {
        let new_addr = Address::new(&s)
            .map_err(|_| format!("{} could not be converted to a valid rcpt address", s))?;

        let email = &mut this
            .write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

        email.envelop.rcpt.insert(new_addr.clone());

        match &mut email.body {
            Body::Empty => Err("failed to rewrite rcpt: the email has not been received yet. Use this method in postq or later.".into()),
            Body::Raw(_) => Err("failed to rewrite rcpt: the email has not been parsed yet. Use this method in postq or later.".into()),
            Body::Parsed(body) => {
                body.add_rcpt(new_addr.full());
                Ok(())
            },
        }
    }

    #[rhai_fn(global, return_raw)]
    pub fn remove_rcpt(
        this: std::sync::Arc<std::sync::RwLock<MailContext>>,
        s: String,
    ) -> EngineResult<()> {
        let addr = Address::new(&s)
            .map_err(|_| format!("{} could not be converted to a valid rcpt address", s))?;

        let email = &mut this
            .write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

        email.envelop.rcpt.remove(&addr);

        match &mut email.body {
            Body::Empty => Err("failed to rewrite rcpt: the email has not been received yet. Use this method in postq or later.".into()),
            Body::Raw(_) => Err("failed to rewrite rcpt: the email has not been parsed yet. Use this method in postq or later.".into()),
            Body::Parsed(body) => {
                body.remove_rcpt(addr.full());
                Ok(())
            },
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
                    Body::Raw(raw) => writer.write_all(raw.as_bytes()),
                    Body::Parsed(email) => writer.write_all(email.to_raw().as_bytes()),
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
            Ok(mut file) => file
                .write_all(
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
    #[rhai_fn(global, return_raw)]
    pub fn quarantine(
        this: &mut std::sync::Arc<std::sync::RwLock<ServerAPI>>,
        ctx: std::sync::Arc<std::sync::RwLock<MailContext>>,
        queue: &str,
    ) -> EngineResult<()> {
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(queue)
        {
            Ok(mut file) => {
                disable_delivery(this)?;

                file.write_all(
                    serde_json::to_string_pretty(&*ctx.read().map_err::<Box<EvalAltResult>, _>(
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

    #[rhai_fn(global, return_raw)]
    pub fn deliver(
        this: &mut std::sync::Arc<std::sync::RwLock<ServerAPI>>,
        resolver: String,
    ) -> EngineResult<()> {
        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .resolver = resolver;

        Ok(())
    }

    #[rhai_fn(global, return_raw)]
    pub fn disable_delivery(
        this: &mut std::sync::Arc<std::sync::RwLock<ServerAPI>>,
    ) -> EngineResult<()> {
        deliver(this, "none".to_string())
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
            .insert(Address::new(bcc).map_err(|_| {
                format!("'{}' could not be converted to a valid rcpt address", bcc)
            })?);

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
            .insert(bcc);

        Ok(())
    }

    /// add a recipient to the list recipient using an object.
    #[rhai_fn(global, name = "bcc", return_raw)]
    pub fn bcc_object(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        bcc: std::sync::Arc<Object>,
    ) -> EngineResult<()> {
        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .rcpt
            .insert(match &*bcc {
                Object::Address(addr) => addr.clone(),
                Object::Str(string) => Address::new(string.as_str()).map_err(|_| {
                    format!(
                        "'{}' could not be converted to a valid rcpt address",
                        string
                    )
                })?,
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
