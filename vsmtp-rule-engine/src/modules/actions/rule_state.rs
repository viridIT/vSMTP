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
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, Position, RhaiResult, TypeId,
};

#[rhai::plugin::export_module]
pub mod rule_state {
    use crate::{
        modules::actions::create_app_folder,
        modules::actions::transports::transports::disable_delivery_all,
        modules::actions::MailContext, modules::mail_context::mail_context::message_id,
        modules::EngineResult, obj::Object, server_api::ServerAPI,
    };
    use vsmtp_common::status::{InfoPacket, Status};

    /// the transaction is forced accepted, skipping all rules and going strait for delivery.
    #[must_use]
    pub const fn faccept() -> Status {
        Status::Faccept
    }

    /// the transaction is accepted. skipping rules to the next stage.
    #[must_use]
    pub const fn accept() -> Status {
        Status::Accept
    }

    /// the transaction continue to execute rule for the current stage.
    #[must_use]
    pub const fn next() -> Status {
        Status::Next
    }

    /// the transaction is denied, reply error to clients. (includes a custom code from a string)
    #[rhai_fn(global, name = "deny")]
    pub fn deny_with_string(message: &str) -> Status {
        Status::Deny(Some(InfoPacket::Str(message.to_string())))
    }

    /// the transaction is denied, reply error to clients. (includes a custom code)
    #[rhai_fn(global, name = "deny", return_raw)]
    pub fn deny_with_code(code: &mut std::sync::Arc<Object>) -> EngineResult<Status> {
        match &**code {
            Object::Str(message) => Ok(Status::Deny(Some(InfoPacket::Str(message.clone())))),
            Object::Code(code) => Ok(Status::Deny(Some(code.clone()))),
            object => {
                Err(format!("deny parameter should be a code, not {}", object.as_str()).into())
            }
        }
    }

    /// the transaction is denied, reply error to clients.
    #[must_use]
    #[rhai_fn(global)]
    pub const fn deny() -> Status {
        Status::Deny(None)
    }

    /// send a single informative code to the client. (using a code object)
    #[rhai_fn(global, name = "info", return_raw)]
    pub fn info_with_code(code: &mut std::sync::Arc<Object>) -> EngineResult<Status> {
        match &**code {
            Object::Str(message) => Ok(Status::Info(InfoPacket::Str(message.to_string()))),
            Object::Code(code) => Ok(Status::Info(code.clone())),
            object => {
                Err(format!("deny parameter should be a code, not {}", object.as_str()).into())
            }
        }
    }

    /// send a single informative code to the client. (using a simple string)
    #[rhai_fn(global)]
    pub fn info(message: &str) -> Status {
        Status::Info(InfoPacket::Str(message.to_string()))
    }

    /// dump the current email into a quarantine queue, skipping delivery.
    /// the email is written in the specified app directory, inside the "queue" folder.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, return_raw, pure)]
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
                    vsmtp_common::re::serde_json::to_string_pretty(&*ctx)
                        .map_err::<Box<EvalAltResult>, _>(|err| {
                            format!("failed to quarantine email: {err:?}").into()
                        })?
                        .as_bytes(),
                )
                .map_err::<Box<EvalAltResult>, _>(|err| {
                    format!("failed to quarantine email: {err:?}").into()
                })?;

                Ok(Status::Deny(None))
            }
            Err(err) => Err(format!("failed to quarantine email: {err:?}").into()),
        }
    }
}
