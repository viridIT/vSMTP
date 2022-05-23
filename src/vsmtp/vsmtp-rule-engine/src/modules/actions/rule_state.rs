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
    use crate::modules::types::types::Context;
    use crate::{
        dsl::object::Object, modules::actions::transports::transports::disable_delivery_all,
        modules::EngineResult,
    };
    use vsmtp_common::{status::Status, CodeID, Reply, ReplyOrCodeID};

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

    /// the transaction is denied, reply error to clients. (includes a custom code)
    #[rhai_fn(global, name = "deny", return_raw)]
    pub fn deny_with_code(code: &mut std::sync::Arc<Object>) -> EngineResult<Status> {
        match &**code {
            Object::Code(code) => Ok(Status::Deny(ReplyOrCodeID::Reply(code.clone()))),
            object => Err(format!("deny parameter must be a code, not {}", object.as_ref()).into()),
        }
    }

    /// the transaction is denied, reply error to clients. (includes a custom code)
    #[rhai_fn(global, name = "deny", return_raw)]
    pub fn deny_with_string(reply_to_parse: &str) -> EngineResult<Status> {
        Ok(Status::Deny(ReplyOrCodeID::Reply(
            match Reply::parse_str(reply_to_parse) {
                Ok(reply) => reply,
                Err(_) => {
                    return Err(
                        format!("deny parameter must be a code, not {:?}", reply_to_parse).into(),
                    )
                }
            },
        )))
    }

    /// the transaction is denied, reply error to clients.
    #[must_use]
    #[rhai_fn(global)]
    pub const fn deny() -> Status {
        Status::Deny(ReplyOrCodeID::CodeID(CodeID::Denied))
    }

    /// send a single informative code to the client. (using a code object)
    #[rhai_fn(global, name = "info", return_raw)]
    pub fn info_with_code(code: &mut std::sync::Arc<Object>) -> EngineResult<Status> {
        match &**code {
            Object::Code(code) => Ok(Status::Info(ReplyOrCodeID::Reply(code.clone()))),
            object => Err(format!("info parameter must be a code, not {}", object.as_ref()).into()),
        }
    }

    /// send a single informative code to the client. (using a simple string)
    #[rhai_fn(global, name = "info", return_raw)]
    pub fn info(reply_to_parse: &str) -> EngineResult<Status> {
        Ok(Status::Info(ReplyOrCodeID::Reply(
            match Reply::parse_str(reply_to_parse) {
                Ok(reply) => reply,
                Err(_) => {
                    return Err(
                        format!("info parameter must be a code, not {:?}", reply_to_parse).into(),
                    )
                }
            },
        )))
    }

    /// tells the state machine to quarantine the email & skip delivery.
    /// the email will be written in the specified app directory, in the "queue" folder.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, return_raw, pure)]
    pub fn quarantine(ctx: &mut Context, queue: &str) -> EngineResult<Status> {
        disable_delivery_all(ctx)?;

        Ok(Status::Quarantine(queue.to_string()))
    }

    #[rhai_fn(global)]
    pub const fn packet(buffer: String) -> Status {
        Status::Packet(buffer)
    }
}
