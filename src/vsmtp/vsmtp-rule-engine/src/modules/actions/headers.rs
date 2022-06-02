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
use crate::modules::types::types::{Context, Message};
use crate::modules::EngineResult;
use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use vsmtp_common::{mail_context::MessageBody, Address};

#[rhai::plugin::export_module]
pub mod headers {

    /// check if a given header exists in the top level headers.
    #[rhai_fn(global, return_raw, pure)]
    pub fn has_header(this: &mut Message, header: &str) -> EngineResult<bool> {
        Ok(vsl_missing_ok!(vsl_guard_ok!(this.read()), "message")
            .get_header(header)
            .is_some())
    }

    /// return the value of a header if it exists. Otherwise, returns an empty string.
    #[rhai_fn(global, return_raw, pure)]
    pub fn get_header(this: &mut Message, header: &str) -> EngineResult<String> {
        Ok(vsl_missing_ok!(vsl_guard_ok!(this.read()), "message")
            .get_header(header)
            .map(ToString::to_string)
            .unwrap_or_default())
    }

    /// add a header to the raw or parsed email contained in ctx.
    #[rhai_fn(global, return_raw, pure)]
    pub fn add_header(this: &mut Message, header: &str, value: &str) -> EngineResult<()> {
        vsl_missing_ok!(mut vsl_guard_ok!(this.write()), "message").add_header(header, value);
        Ok(())
    }

    /// set a header to the raw or parsed email contained in ctx.
    #[rhai_fn(global, return_raw, pure)]
    pub fn set_header(this: &mut Message, header: &str, value: &str) -> EngineResult<()> {
        vsl_missing_ok!(mut vsl_guard_ok!(this.write()), "message").set_header(header, value);
        Ok(())
    }

    /// change the sender of the mail
    #[rhai_fn(global, return_raw, pure)]
    pub fn rewrite_mail_from_context(this: &mut Context, new_addr: &str) -> EngineResult<()> {
        let new_addr =
            Address::try_from(new_addr.to_string()).map_err::<Box<EvalAltResult>, _>(|_| {
                format!(
                    "could not rewrite mail_from with '{new_addr}' because it is not valid address"
                )
                .into()
            })?;

        vsl_guard_ok!(this.write()).envelop.mail_from = new_addr;
        Ok(())
    }

    #[rhai_fn(global, return_raw, pure)]
    pub fn rewrite_mail_from_message(this: &mut Message, new_addr: &str) -> EngineResult<()> {
        let new_addr =
            Address::try_from(new_addr.to_string()).map_err::<Box<EvalAltResult>, _>(|_| {
                format!(
                    "could not rewrite mail_from with '{new_addr}' because it is not valid address"
                )
                .into()
            })?;

        let mut writer = vsl_guard_ok!(this.write());
        match vsl_parse_ok!(writer) {
            MessageBody::Parsed(body) => body.rewrite_mail_from(new_addr.full()),
            MessageBody::Raw(..) => unreachable!("the message has been parsed just above"),
        }
        Ok(())
    }

    #[rhai_fn(global, return_raw, pure)]
    pub fn rewrite_to_message(
        this: &mut Message,
        old_addr: &str,
        new_addr: &str,
    ) -> EngineResult<()> {
        let old_addr =
            Address::try_from(old_addr.to_string()).map_err::<Box<EvalAltResult>, _>(|_| {
                format!("could not rewrite address '{old_addr}' because it is not valid address",)
                    .into()
            })?;

        let new_addr =
            Address::try_from(new_addr.to_string()).map_err::<Box<EvalAltResult>, _>(|_| {
                format!(
                    "could not rewrite address '{old_addr}' with '{new_addr}' because it is not valid address"
                )
                .into()
            })?;

        let mut writer = vsl_guard_ok!(this.write());
        match vsl_parse_ok!(writer) {
            MessageBody::Parsed(body) => body.rewrite_rcpt(old_addr.full(), new_addr.full()),
            MessageBody::Raw(..) => unreachable!("the message has been parsed just above"),
        }
        Ok(())
    }

    /// change a recipient of the envelop.
    #[rhai_fn(global, return_raw, pure)]
    pub fn rewrite_rcpt(this: &mut Context, old_addr: &str, new_addr: &str) -> EngineResult<()> {
        let old_addr =
            Address::try_from(old_addr.to_string()).map_err::<Box<EvalAltResult>, _>(|_| {
                format!("could not rewrite address '{old_addr}' because it is not valid address")
                    .into()
            })?;

        let new_addr =
            Address::try_from(new_addr.to_string()).map_err::<Box<EvalAltResult>, _>(|_| {
                format!(
                    "could not rewrite address '{old_addr}' with '{new_addr}' because it is not valid address"
                )
                .into()
            })?;

        let email = &mut this
            .write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

        email
            .envelop
            .rcpt
            .push(vsmtp_common::rcpt::Rcpt::new(new_addr));

        if let Some(index) = email
            .envelop
            .rcpt
            .iter()
            .position(|rcpt| rcpt.address == old_addr)
        {
            email.envelop.rcpt.swap_remove(index);
        }
        Ok(())
    }

    /// add a recipient to the 'To' mail header.
    #[rhai_fn(global, return_raw, pure)]
    pub fn add_to(this: &mut Message, new_addr: &str) -> EngineResult<()> {
        let new_addr = Address::try_from(new_addr.to_string())
            .map_err(|_| format!("'{new_addr}' could not be converted to a valid rcpt address"))?;

        println!("here");
        let mut writer = vsl_guard_ok!(this.write());
        match vsl_parse_ok!(writer) {
            MessageBody::Parsed(body) => body.add_rcpt(new_addr.full()),
            MessageBody::Raw(..) => unreachable!("the message has been parsed just above"),
        }
        Ok(())
    }

    /// add a recipient to the envelop.
    #[rhai_fn(global, return_raw, pure)]
    pub fn add_rcpt(this: &mut Context, new_addr: &str) -> EngineResult<()> {
        let new_addr = Address::try_from(new_addr.to_string())
            .map_err(|_| format!("'{new_addr}' could not be converted to a valid rcpt address"))?;

        vsl_guard_ok!(this.write())
            .envelop
            .rcpt
            .push(vsmtp_common::rcpt::Rcpt::new(new_addr));

        Ok(())
    }

    /// remove a recipient from the mail 'To' header.
    #[rhai_fn(global, return_raw, pure)]
    pub fn remove_to(this: &mut Message, addr: &str) -> EngineResult<()> {
        let addr = Address::try_from(addr.to_string())
            .map_err(|_| format!("{addr} could not be converted to a valid rcpt address"))?;

        let mut writer = vsl_guard_ok!(this.write());
        match vsl_parse_ok!(writer) {
            MessageBody::Parsed(body) => body.remove_rcpt(addr.full()),
            MessageBody::Raw(..) => unreachable!("the message has been parsed just above"),
        }
        Ok(())
    }

    /// remove a recipient from the envelop.
    #[rhai_fn(global, return_raw, pure)]
    pub fn remove_rcpt(this: &mut Context, addr: &str) -> EngineResult<()> {
        let addr = Address::try_from(addr.to_string())
            .map_err(|_| format!("{addr} could not be converted to a valid rcpt address"))?;

        let email = &mut this
            .write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

        if let Some(index) = email
            .envelop
            .rcpt
            .iter()
            .position(|rcpt| rcpt.address == addr)
        {
            email.envelop.rcpt.remove(index);
            Ok(())
        } else {
            Err(format!(
                "could not remove address '{addr}' because it does not resides in the envelop."
            )
            .into())
        }
    }
}
