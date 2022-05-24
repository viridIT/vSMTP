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
use crate::{
    dsl::object::Object, modules::actions::transports::transports::disable_delivery_all,
    modules::types::types::Context, modules::EngineResult,
};
use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, Position, RhaiResult, TypeId,
};
use vsmtp_common::{status::Status, CodeID, Reply, ReplyOrCodeID};

fn reply_or_code_id_from_object(code: &std::sync::Arc<Object>) -> EngineResult<ReplyOrCodeID> {
    match &**code {
        Object::Code(code) => Ok(ReplyOrCodeID::Reply(code.clone())),
        object => Err(format!("parameter must be a code, not {}", object.as_ref()).into()),
    }
}

fn reply_or_code_id_from_string(code: &str) -> EngineResult<ReplyOrCodeID> {
    Ok(ReplyOrCodeID::Reply(Reply::parse_str(code).map_err::<Box<
        EvalAltResult,
    >, _>(
        |_| format!("parameter must be a code, not {:?}", code).into(),
    )?))
}

#[rhai::plugin::export_module]
pub mod rule_state {

    /// the transaction is forced accepted, skipping all rules and going strait for delivery.
    #[must_use]
    pub const fn faccept() -> Status {
        Status::Faccept(ReplyOrCodeID::CodeID(CodeID::Ok))
    }

    #[rhai_fn(global, name = "faccept", return_raw)]
    pub fn faccept_with_code(code: &mut std::sync::Arc<Object>) -> EngineResult<Status> {
        reply_or_code_id_from_object(code).map(Status::Faccept)
    }

    /// the transaction is denied, reply error to clients. (includes a custom code)
    #[rhai_fn(global, name = "faccept", return_raw)]
    pub fn faccept_with_string(reply_to_parse: &str) -> EngineResult<Status> {
        reply_or_code_id_from_string(reply_to_parse).map(Status::Faccept)
    }

    /// the transaction is accepted. skipping rules to the next stage.
    #[must_use]
    pub const fn accept() -> Status {
        Status::Accept(ReplyOrCodeID::CodeID(CodeID::Ok))
    }

    #[rhai_fn(global, name = "accept", return_raw)]
    pub fn accept_with_code(code: &mut std::sync::Arc<Object>) -> EngineResult<Status> {
        reply_or_code_id_from_object(code).map(Status::Accept)
    }

    /// the transaction is denied, reply error to clients. (includes a custom code)
    #[rhai_fn(global, name = "accept", return_raw)]
    pub fn accept_with_string(reply_to_parse: &str) -> EngineResult<Status> {
        reply_or_code_id_from_string(reply_to_parse).map(Status::Accept)
    }

    /// the transaction continue to execute rule for the current stage.
    #[must_use]
    pub const fn next() -> Status {
        Status::Next
    }

    /// the transaction is denied, reply error to clients.
    #[must_use]
    #[rhai_fn(global)]
    pub const fn deny() -> Status {
        Status::Deny(ReplyOrCodeID::CodeID(CodeID::Denied))
    }

    /// the transaction is denied, reply error to clients. (includes a custom code)
    #[rhai_fn(global, name = "deny", return_raw)]
    pub fn deny_with_code(code: &mut std::sync::Arc<Object>) -> EngineResult<Status> {
        reply_or_code_id_from_object(code).map(Status::Deny)
    }

    /// the transaction is denied, reply error to clients. (includes a custom code)
    #[rhai_fn(global, name = "deny", return_raw)]
    pub fn deny_with_string(reply_to_parse: &str) -> EngineResult<Status> {
        reply_or_code_id_from_string(reply_to_parse).map(Status::Deny)
    }

    /// send a single informative code to the client. (using a code object)
    #[rhai_fn(global, name = "info", return_raw)]
    pub fn info_with_code(code: &mut std::sync::Arc<Object>) -> EngineResult<Status> {
        reply_or_code_id_from_object(code).map(Status::Info)
    }

    /// send a single informative code to the client. (using a simple string)
    #[rhai_fn(global, name = "info", return_raw)]
    pub fn info_with_string(reply_to_parse: &str) -> EngineResult<Status> {
        reply_or_code_id_from_string(reply_to_parse).map(Status::Info)
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
