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
use crate::api::{
    EngineResult, {Message, SharedObject},
};
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use vsmtp_common::Address;

pub use message_parsed::*;

#[rhai::plugin::export_module]
mod message_parsed {

    /// replace the value of the `From` header by another address.
    #[rhai_fn(global, name = "rewrite_mail_from_message", return_raw, pure)]
    pub fn rewrite_mail_from_message_str(
        message: &mut Message,
        new_addr: &str,
    ) -> EngineResult<()> {
        super::rewrite_mail_from_message(message, new_addr)
    }

    /// replace the value of the `From` header by another address.
    #[rhai_fn(global, name = "rewrite_mail_from_message", return_raw, pure)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn rewrite_mail_from_message_obj(
        message: &mut Message,
        new_addr: SharedObject,
    ) -> EngineResult<()> {
        super::rewrite_mail_from_message(message, &new_addr.to_string())
    }

    /// replace the value of the `To:` header by another address.
    #[rhai_fn(global, name = "rewrite_rcpt_message", return_raw, pure)]
    pub fn rewrite_rcpt_message_str_str(
        message: &mut Message,
        old_addr: &str,
        new_addr: &str,
    ) -> EngineResult<()> {
        super::rewrite_rcpt_message(message, old_addr, new_addr)
    }

    /// replace the value of the `To:` header by another address.
    #[rhai_fn(global, name = "rewrite_rcpt_message", return_raw, pure)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn rewrite_rcpt_message_obj_str(
        message: &mut Message,
        old_addr: SharedObject,
        new_addr: &str,
    ) -> EngineResult<()> {
        super::rewrite_rcpt_message(message, &old_addr.to_string(), new_addr)
    }

    /// replace the value of the `To:` header by another address.
    #[rhai_fn(global, name = "rewrite_rcpt_message", return_raw, pure)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn rewrite_rcpt_message_str_obj(
        message: &mut Message,
        old_addr: &str,
        new_addr: SharedObject,
    ) -> EngineResult<()> {
        super::rewrite_rcpt_message(message, old_addr, &new_addr.to_string())
    }

    /// replace the value of the `To:` header by another address.
    #[rhai_fn(global, name = "rewrite_rcpt_message", return_raw, pure)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn rewrite_rcpt_message_obj_obj(
        message: &mut Message,
        old_addr: SharedObject,
        new_addr: SharedObject,
    ) -> EngineResult<()> {
        super::rewrite_rcpt_message(message, &old_addr.to_string(), &new_addr.to_string())
    }

    /// add a recipient to the 'To' mail header.
    #[rhai_fn(global, name = "add_rcpt_message", return_raw, pure)]
    pub fn add_rcpt_message_str(message: &mut Message, new_addr: &str) -> EngineResult<()> {
        super::add_rcpt_message(message, new_addr)
    }

    /// add a recipient to the 'To' mail header.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "add_rcpt_message", return_raw, pure)]
    pub fn add_rcpt_message_obj(message: &mut Message, new_addr: SharedObject) -> EngineResult<()> {
        super::add_rcpt_message(message, &new_addr.to_string())
    }

    /// remove a recipient from the mail 'To' header.
    #[rhai_fn(global, name = "remove_rcpt_message", return_raw, pure)]
    pub fn remove_rcpt_message_str(message: &mut Message, addr: &str) -> EngineResult<()> {
        super::remove_rcpt_message(message, addr)
    }

    /// remove a recipient from the mail 'To' header.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "remove_rcpt_message", return_raw, pure)]
    pub fn remove_rcpt_message_obj(message: &mut Message, addr: SharedObject) -> EngineResult<()> {
        super::remove_rcpt_message(message, &addr.to_string())
    }
}

/// internal generic function to rewrite the value of the `From` header.
fn rewrite_mail_from_message(message: &mut Message, new_addr: &str) -> EngineResult<()> {
    let new_addr = vsl_conversion_ok!("address", Address::try_from(new_addr.to_string()));

    let mut writer = vsl_guard_ok!(message.write());
    vsl_parse_ok!(writer).rewrite_mail_from(new_addr.full());

    Ok(())
}

/// internal generic function to rewrite the value of the `To` header.
fn rewrite_rcpt_message(message: &mut Message, old_addr: &str, new_addr: &str) -> EngineResult<()> {
    let new_addr = vsl_conversion_ok!("address", Address::try_from(new_addr.to_string()));
    let old_addr = vsl_conversion_ok!("address", Address::try_from(old_addr.to_string()));

    let mut writer = vsl_guard_ok!(message.write());
    vsl_parse_ok!(writer).rewrite_rcpt(old_addr.full(), new_addr.full());
    Ok(())
}

/// internal generic function to add a recipient to the `To` header.
fn add_rcpt_message(message: &mut Message, new_addr: &str) -> EngineResult<()> {
    let new_addr = vsl_conversion_ok!("address", Address::try_from(new_addr.to_string()));

    let mut writer = vsl_guard_ok!(message.write());
    vsl_parse_ok!(writer).add_rcpt(new_addr.full());
    Ok(())
}

/// internal generic function to remove a recipient to the `To` header.
fn remove_rcpt_message(this: &mut Message, addr: &str) -> EngineResult<()> {
    let addr = vsl_conversion_ok!("address", Address::try_from(addr.to_string()));

    let mut writer = vsl_guard_ok!(this.write());
    vsl_parse_ok!(writer).remove_rcpt(addr.full());
    Ok(())
}
