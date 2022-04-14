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
use vsmtp_common::re::anyhow;
use vsmtp_common::re::log;
use vsmtp_common::re::serde_json;
use vsmtp_common::status::Status;
use vsmtp_config::log_channel::URULES;

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
    pub fn send_mail(from: &str, to: rhai::Array, path: &str, relay: &str) -> EngineResult<()> {
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
        vsmtp_config::re::users::get_user_by_name(name).is_some()
    }

    /// execute the service named @service_name from the vSMTP configuration definition
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, return_raw)]
    pub fn run_service(
        srv: &mut std::sync::Arc<ServerAPI>,
        ctx: std::sync::Arc<std::sync::RwLock<MailContext>>,
        service_name: &str,
    ) -> EngineResult<ServiceResult> {
        crate::service::run(
            srv.config
                .app
                .services
                .get(service_name)
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
        let new_addr =
            Address::try_from(new_addr.to_string()).map_err::<Box<EvalAltResult>, _>(|_| {
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
        let old_addr =
            Address::try_from(old_addr.to_string()).map_err::<Box<EvalAltResult>, _>(|_| {
                format!(
                    "could not rewrite address '{}' because it is not valid address",
                    old_addr,
                )
                .into()
            })?;

        let new_addr =
            Address::try_from(new_addr.to_string()).map_err::<Box<EvalAltResult>, _>(|_| {
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
        let new_addr = Address::try_from(new_addr.to_string()).map_err(|_| {
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
        let addr = Address::try_from(addr.to_string())
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
        srv: &mut std::sync::Arc<ServerAPI>,
        mut ctx: std::sync::Arc<std::sync::RwLock<MailContext>>,
        dir: &str,
    ) -> EngineResult<()> {
        let mut dir =
            create_app_folder(&srv.config, Some(dir)).map_err::<Box<EvalAltResult>, _>(|err| {
                format!(
                    "failed to write email at {}/{dir}: {err}",
                    srv.config.app.dirpath.display()
                )
                .into()
            })?;

        dir.push(format!("{}.eml", message_id(&mut ctx)?));

        match std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&dir)
        {
            Ok(file) => {
                let mut writer = std::io::LineWriter::new(file);

                match &ctx
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
            .map_err(|err| format!("failed to write email at {dir:?}: {err}").into()),
            Err(err) => Err(format!("failed to write email at {dir:?}: {err}").into()),
        }
    }

    /// write the content of the current email with it's metadata in a json file.
    #[rhai_fn(global, return_raw)]
    pub fn dump(
        srv: &mut std::sync::Arc<ServerAPI>,
        mut ctx: std::sync::Arc<std::sync::RwLock<MailContext>>,
        dir: &str,
    ) -> EngineResult<()> {
        let mut dir =
            create_app_folder(&srv.config, Some(dir)).map_err::<Box<EvalAltResult>, _>(|err| {
                format!(
                    "failed to dump email at {}/{dir}: {err}",
                    srv.config.app.dirpath.display()
                )
                .into()
            })?;

        dir.push(format!("{}.json", message_id(&mut ctx)?));

        match std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&dir)
        {
            Ok(mut file) => std::io::Write::write_all(
                &mut file,
                serde_json::to_string_pretty(
                    &*ctx
                        .read()
                        .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?,
                )
                .map_err::<Box<EvalAltResult>, _>(|err| {
                    format!("failed to dump email at {dir:?}: {err}").into()
                })?
                .as_bytes(),
            )
            .map_err(|err| format!("failed to dump email at {dir:?}: {err}").into()),
            Err(err) => Err(format!("failed to dump email at {dir:?}: {err}").into()),
        }
    }

    /// dump the current email into a quarantine queue, skipping delivery.
    /// the email is written in the specified app directory, inside the "queue" folder.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, return_raw)]
    pub fn quarantine(
        srv: &mut std::sync::Arc<ServerAPI>,
        mut ctx: std::sync::Arc<std::sync::RwLock<MailContext>>,
        queue: &str,
    ) -> EngineResult<Status> {
        disable_delivery_all(&mut ctx)?;

        let mut path = create_app_folder(&srv.config, Some(queue))
            .map_err::<Box<EvalAltResult>, _>(|err| {
                format!(
                    "failed to dump email at {}/{queue}: {err}",
                    srv.config.app.dirpath.display()
                )
                .into()
            })?;

        path.push(format!("{}.json", message_id(&mut ctx)?));

        let ctx = ctx.read().map_err::<Box<EvalAltResult>, _>(|_| {
            "failed to quarantine email: mail context poisoned".into()
        })?;

        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            Ok(mut file) => {
                std::io::Write::write_all(
                    &mut file,
                    serde_json::to_string_pretty(&*ctx)
                        .map_err::<Box<EvalAltResult>, _>(|err| {
                            format!("failed to quarantine email: {err:?}").into()
                        })?
                        .as_bytes(),
                )
                .map_err::<Box<EvalAltResult>, _>(|err| {
                    format!("failed to quarantine email: {err:?}").into()
                })?;

                Ok(Status::Deny)
            }
            Err(err) => Err(format!("failed to quarantine email: {err:?}").into()),
        }
    }

    /// set the delivery method to "Forward" for a single recipient.
    #[rhai_fn(global, return_raw)]
    pub fn forward(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        rcpt: &str,
        forward: &str,
    ) -> EngineResult<()> {
        set_transport_for(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            rcpt,
            &vsmtp_common::transfer::Transfer::Forward(forward.to_string()),
        )
        .map_err(|err| err.to_string().into())
    }

    /// set the delivery method to "Forward" for all recipients.
    #[rhai_fn(global, return_raw)]
    pub fn forward_all(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        forward: &str,
    ) -> EngineResult<()> {
        set_transport(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            &vsmtp_common::transfer::Transfer::Forward(forward.to_string()),
        );

        Ok(())
    }

    /// set the delivery method to "Deliver" for a single recipient.
    #[rhai_fn(global, return_raw)]
    pub fn deliver(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        rcpt: &str,
    ) -> EngineResult<()> {
        set_transport_for(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            rcpt,
            &vsmtp_common::transfer::Transfer::Deliver,
        )
        .map_err(|err| err.to_string().into())
    }

    /// set the delivery method to "Deliver" for all recipients.
    #[rhai_fn(global, return_raw)]
    pub fn deliver_all(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
    ) -> EngineResult<()> {
        set_transport(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            &vsmtp_common::transfer::Transfer::Deliver,
        );

        Ok(())
    }

    /// set the delivery method to "Mbox" for a single recipient.
    #[rhai_fn(global, return_raw)]
    pub fn mbox(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        rcpt: &str,
    ) -> EngineResult<()> {
        set_transport_for(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            rcpt,
            &vsmtp_common::transfer::Transfer::Mbox,
        )
        .map_err(|err| err.to_string().into())
    }

    /// set the delivery method to "Mbox" for all recipients.
    #[rhai_fn(global, return_raw)]
    pub fn mbox_all(this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>) -> EngineResult<()> {
        set_transport(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            &vsmtp_common::transfer::Transfer::Mbox,
        );

        Ok(())
    }

    /// set the delivery method to "Maildir" for a single recipient.
    #[rhai_fn(global, return_raw)]
    pub fn maildir(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        rcpt: &str,
    ) -> EngineResult<()> {
        set_transport_for(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            rcpt,
            &vsmtp_common::transfer::Transfer::Maildir,
        )
        .map_err(|err| err.to_string().into())
    }

    /// set the delivery method to "Maildir" for all recipients.
    #[rhai_fn(global, return_raw)]
    pub fn maildir_all(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
    ) -> EngineResult<()> {
        set_transport(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            &vsmtp_common::transfer::Transfer::Maildir,
        );

        Ok(())
    }

    /// remove the delivery method for a specific recipient.
    #[rhai_fn(global, return_raw)]
    pub fn disable_delivery(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        rcpt: &str,
    ) -> EngineResult<()> {
        set_transport_for(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            rcpt,
            &vsmtp_common::transfer::Transfer::None,
        )
        .map_err(|err| err.to_string().into())
    }

    /// remove the delivery method for all recipient.
    #[rhai_fn(global, return_raw)]
    pub fn disable_delivery_all(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
    ) -> EngineResult<()> {
        set_transport(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            &vsmtp_common::transfer::Transfer::None,
        );

        Ok(())
    }

    /// check if a given header exists in the top level headers.
    #[rhai_fn(global, return_raw, pure)]
    pub fn has_header(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        header: &str,
    ) -> EngineResult<bool> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .body
            .get_header(header)
            .is_some())
    }

    /// return the value of a header if it exists. Otherwise, returns an empty string.
    #[rhai_fn(global, return_raw)]
    pub fn get_header(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        header: &str,
    ) -> EngineResult<String> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .body
            .get_header(header)
            .map(std::string::ToString::to_string)
            .unwrap_or_default())
    }

    /// add a header to the raw or parsed email contained in ctx.
    #[rhai_fn(global, return_raw)]
    pub fn add_header(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        header: &str,
        value: &str,
    ) -> EngineResult<()> {
        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .body
            .add_header(header, value);

        Ok(())
    }

    /// set a header to the raw or parsed email contained in ctx.
    #[rhai_fn(global, return_raw)]
    pub fn set_header(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        header: &str,
        value: &str,
    ) -> EngineResult<()> {
        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .body
            .set_header(header, value);
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
                Address::try_from(bcc.to_string()).map_err(|_| {
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
                    Address::try_from(string.clone()).map_err(|_| {
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

/// set the transport method of a single recipient.
fn set_transport_for(
    ctx: &mut MailContext,
    search: &str,
    method: &vsmtp_common::transfer::Transfer,
) -> anyhow::Result<()> {
    ctx.envelop
        .rcpt
        .iter_mut()
        .find(|rcpt| rcpt.address.full() == search)
        .ok_or_else(|| anyhow::anyhow!("could not find rcpt '{}'", search))
        .map(|rcpt| rcpt.transfer_method = method.clone())
}

/// set the transport method of all recipients.
fn set_transport(ctx: &mut MailContext, method: &vsmtp_common::transfer::Transfer) {
    ctx.envelop
        .rcpt
        .iter_mut()
        .for_each(|rcpt| rcpt.transfer_method = method.clone());
}

/// create a folder at `[app.dirpath]` if needed, or just create the app folder.
fn create_app_folder(
    config: &vsmtp_config::Config,
    path: Option<&str>,
) -> anyhow::Result<std::path::PathBuf> {
    let path = path.map_or_else(
        || config.app.dirpath.clone(),
        |path| config.app.dirpath.join(path),
    );

    if !path.exists() {
        std::fs::create_dir_all(&path)?;
    }

    Ok(path)
}

#[cfg(test)]
mod test {

    use super::{create_app_folder, set_transport, set_transport_for};
    use vsmtp_common::{
        address::Address, mail_context::MailContext, rcpt::Rcpt, transfer::Transfer,
    };
    use vsmtp_config::Config;

    fn get_default_context() -> MailContext {
        MailContext {
            body: vsmtp_common::mail_context::Body::Empty,
            connection_timestamp: std::time::SystemTime::now(),
            client_addr: std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
                0,
            ),
            envelop: vsmtp_common::envelop::Envelop::default(),
            metadata: Some(vsmtp_common::mail_context::MessageMetadata {
                timestamp: std::time::SystemTime::now(),
                ..vsmtp_common::mail_context::MessageMetadata::default()
            }),
        }
    }

    #[test]
    fn test_set_transport_for() {
        let mut ctx = get_default_context();

        ctx.envelop.rcpt.push(Rcpt::new(
            Address::try_from("valid@rcpt.foo".to_string()).unwrap(),
        ));

        assert!(set_transport_for(&mut ctx, "valid@rcpt.foo", &Transfer::Deliver).is_ok());
        assert!(set_transport_for(&mut ctx, "invalid@rcpt.foo", &Transfer::Deliver).is_err());

        ctx.envelop
            .rcpt
            .iter()
            .find(|rcpt| rcpt.address.full() == "valid@rcpt.foo")
            .map(|rcpt| {
                assert_eq!(rcpt.transfer_method, Transfer::Deliver);
            })
            .or_else(|| panic!("recipient transfer method is not valid"));
    }

    #[test]
    fn test_set_transport() {
        let mut ctx = get_default_context();

        set_transport(&mut ctx, &Transfer::Forward("mta.example.com".to_string()));

        assert!(ctx
            .envelop
            .rcpt
            .iter()
            .all(|rcpt| rcpt.transfer_method == Transfer::Forward("mta.example.com".to_string())));
    }

    #[test]
    fn test_create_app_folder() {
        let mut config = Config::default();
        config.app.dirpath = "./tests/generated".into();

        let app_folder = create_app_folder(&config, None).unwrap();
        let nested_folder = create_app_folder(&config, Some("folder")).unwrap();
        let deep_folder = create_app_folder(&config, Some("deep/folder")).unwrap();

        assert_eq!(app_folder, config.app.dirpath);
        assert!(app_folder.exists());
        assert_eq!(
            nested_folder,
            std::path::PathBuf::from_iter([config.app.dirpath.to_str().unwrap(), "folder"])
        );
        assert!(nested_folder.exists());
        assert_eq!(
            deep_folder,
            std::path::PathBuf::from_iter([config.app.dirpath.to_str().unwrap(), "deep", "folder"])
        );
        assert!(deep_folder.exists());
    }
}
