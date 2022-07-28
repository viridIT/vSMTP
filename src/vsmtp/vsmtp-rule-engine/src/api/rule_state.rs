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
    api::{
        transports::disable_delivery_all,
        EngineResult, {Context, SharedObject},
    },
    dsl::object::Object,
};
use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, Position, RhaiResult, TypeId,
};
use vsmtp_common::{status::Status, CodeID, Reply, ReplyOrCodeID};

fn reply_or_code_id_from_object(code: &SharedObject) -> EngineResult<ReplyOrCodeID> {
    match &**code {
        Object::Code(code) => Ok(ReplyOrCodeID::Right(code.clone())),
        object => Err(format!("parameter must be a code, not {}", object.as_ref()).into()),
    }
}

fn reply_or_code_id_from_string(code: &str) -> EngineResult<ReplyOrCodeID> {
    Ok(ReplyOrCodeID::Right(Reply::parse_str(code).map_err::<Box<
        EvalAltResult,
    >, _>(
        |_| format!("parameter must be a code, not {:?}", code).into(),
    )?))
}

pub use rule_state::*;

#[rhai::plugin::export_module]
mod rule_state {

    /// Return a [`Status::Faccept`] with the default code associated
    #[must_use]
    pub const fn faccept() -> Status {
        Status::Faccept(ReplyOrCodeID::Left(CodeID::Ok))
    }

    /// Return a [`Status::Faccept`] with `code`
    ///
    /// # Errors
    ///
    /// * `code` is not a valid code
    #[rhai_fn(global, name = "faccept", return_raw)]
    pub fn faccept_with_code(code: &mut SharedObject) -> EngineResult<Status> {
        reply_or_code_id_from_object(code).map(Status::Faccept)
    }

    /// Return a [`Status::Faccept`] with `code`
    ///
    /// # Errors
    ///
    /// * `code` is not a valid code
    #[rhai_fn(global, name = "faccept", return_raw)]
    pub fn faccept_with_string(code: &str) -> EngineResult<Status> {
        reply_or_code_id_from_string(code).map(Status::Faccept)
    }

    /// Return a [`Status::Accept`] with the default code associated
    #[must_use]
    pub const fn accept() -> Status {
        Status::Accept(ReplyOrCodeID::Left(CodeID::Ok))
    }

    /// Return a [`Status::Accept`] with `code`
    ///
    /// # Errors
    ///
    /// * `code` is not a valid code
    #[rhai_fn(global, name = "accept", return_raw)]
    pub fn accept_with_code(code: &mut SharedObject) -> EngineResult<Status> {
        reply_or_code_id_from_object(code).map(Status::Accept)
    }

    /// Return a [`Status::Accept`] with `code`
    ///
    /// # Errors
    ///
    /// * `code` is not a valid code
    #[rhai_fn(global, name = "accept", return_raw)]
    pub fn accept_with_string(code: &str) -> EngineResult<Status> {
        reply_or_code_id_from_string(code).map(Status::Accept)
    }

    /// Return a [`Status::Next`]
    #[must_use]
    pub const fn next() -> Status {
        Status::Next
    }

    /// Return a [`Status::Deny`] with the default code associated
    #[must_use]
    #[rhai_fn(global)]
    pub const fn deny() -> Status {
        Status::Deny(ReplyOrCodeID::Left(CodeID::Denied))
    }

    /// Return a [`Status::Deny`] with `code`
    ///
    /// # Errors
    ///
    /// * `code` is not a valid code
    #[rhai_fn(global, name = "deny", return_raw)]
    pub fn deny_with_code(code: &mut SharedObject) -> EngineResult<Status> {
        reply_or_code_id_from_object(code).map(Status::Deny)
    }

    /// Return a [`Status::Deny`] with `code`
    ///
    /// # Errors
    ///
    /// * `code` is not a valid code
    #[rhai_fn(global, name = "deny", return_raw)]
    pub fn deny_with_string(code: &str) -> EngineResult<Status> {
        reply_or_code_id_from_string(code).map(Status::Deny)
    }

    /// Return a [`Status::Info`] with `code`
    ///
    /// # Errors
    ///
    /// * `code` is not a valid code
    #[rhai_fn(global, name = "info", return_raw)]
    pub fn info_with_code(code: &mut SharedObject) -> EngineResult<Status> {
        reply_or_code_id_from_object(code).map(Status::Info)
    }

    /// Return a [`Status::Info`] with `code`
    ///
    /// # Errors
    ///
    /// * `code` is not a valid code
    #[rhai_fn(global, name = "info", return_raw)]
    pub fn info_with_string(code: &str) -> EngineResult<Status> {
        reply_or_code_id_from_string(code).map(Status::Info)
    }

    /// Return a [`Status::Quarantine`] with `queue`
    ///
    /// # Errors
    ///
    /// * a mutex is poisoned
    #[rhai_fn(global, name = "quarantine", return_raw, pure)]
    pub fn quarantine_str(ctx: &mut Context, queue: &str) -> EngineResult<Status> {
        disable_delivery_all(ctx)?;

        Ok(Status::Quarantine(queue.to_string()))
    }

    /// Return a [`Status::Quarantine`] with `queue`
    ///
    /// # Errors
    ///
    /// * a mutex is poisoned
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "quarantine", return_raw, pure)]
    pub fn quarantine_obj(ctx: &mut Context, queue: SharedObject) -> EngineResult<Status> {
        disable_delivery_all(ctx)?;

        Ok(Status::Quarantine(queue.to_string()))
    }

    /// Return a [`Status::Packet`] with `buffer`
    #[rhai_fn(global)]
    #[must_use]
    pub const fn packet(buffer: String) -> Status {
        Status::Packet(buffer)
    }
}
