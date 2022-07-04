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
use vsmtp_common::{rcpt::Rcpt, Address};

#[rhai::plugin::export_module]
pub mod message {
    /// check if a given header exists in the top level headers.
    #[rhai_fn(global, return_raw, pure)]
    pub fn has_header(this: &mut Message, header: &str) -> EngineResult<bool> {
        Ok(vsl_guard_ok!(this.read()).get_header(header).is_some())
    }

    /// return the value of a header if it exists. Otherwise, returns an empty string.
    #[rhai_fn(global, return_raw, pure)]
    pub fn get_header(this: &mut Message, header: &str) -> EngineResult<String> {
        Ok(vsl_guard_ok!(this.read())
            .get_header(header)
            .unwrap_or_default())
    }

    /// add a header to the end of the raw or parsed email contained in ctx.
    #[rhai_fn(global, return_raw, pure)]
    pub fn append_header(this: &mut Message, header: &str, value: &str) -> EngineResult<()> {
        vsl_guard_ok!(this.write()).append_header(header, value);
        Ok(())
    }

    /// prepend a header to the raw or parsed email contained in ctx.
    #[rhai_fn(global, return_raw, pure)]
    pub fn prepend_header(this: &mut Message, header: &str, value: &str) -> EngineResult<()> {
        vsl_guard_ok!(this.write()).prepend_header(header, value);
        Ok(())
    }

    /// set a header to the raw or parsed email contained in ctx.
    #[rhai_fn(global, return_raw, pure)]
    pub fn set_header(this: &mut Message, header: &str, value: &str) -> EngineResult<()> {
        vsl_guard_ok!(this.write()).set_header(header, value);
        Ok(())
    }

    /// Get the message body as a string
    #[rhai_fn(global, get = "mail", return_raw, pure)]
    pub fn mail(this: &mut Message) -> EngineResult<String> {
        Ok(vsl_guard_ok!(this.read()).inner().to_string())
    }

    /// Change the sender of the envelop
    #[rhai_fn(global, return_raw, pure)]
    pub fn rewrite_mail_from_context(this: &mut Context, new_addr: &str) -> EngineResult<()> {
        vsl_guard_ok!(this.write()).envelop.mail_from =
            vsl_conversion_ok!("address", Address::try_from(new_addr.to_string()));
        Ok(())
    }

    /// Change a recipient of the envelop.
    #[rhai_fn(global, return_raw, pure)]
    pub fn rewrite_rcpt(this: &mut Context, old_addr: &str, new_addr: &str) -> EngineResult<()> {
        let old_addr = vsl_conversion_ok!("address", Address::try_from(old_addr.to_string()));
        let new_addr = vsl_conversion_ok!("address", Address::try_from(new_addr.to_string()));

        let mut email = vsl_guard_ok!(this.write());

        email.envelop.rcpt.push(Rcpt::new(new_addr));

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

    /// add a recipient to the envelop.
    #[rhai_fn(global, return_raw, pure)]
    pub fn add_rcpt(this: &mut Context, new_addr: &str) -> EngineResult<()> {
        vsl_guard_ok!(this.write())
            .envelop
            .rcpt
            .push(Rcpt::new(vsl_conversion_ok!(
                "address",
                Address::try_from(new_addr.to_string())
            )));

        Ok(())
    }

    /// remove a recipient from the envelop.
    #[rhai_fn(global, return_raw, pure)]
    pub fn remove_rcpt(this: &mut Context, addr: &str) -> EngineResult<()> {
        let addr = vsl_conversion_ok!("address", Address::try_from(addr.to_string()));

        let mut email = vsl_guard_ok!(this.write());

        email
            .envelop
            .rcpt
            .iter()
            .position(|rcpt| rcpt.address == addr)
            .map_or_else(
                || {
                    Err(format!(
                "could not remove address '{addr}' because it does not resides in the envelop."
            )
                    .into())
                },
                |index| {
                    email.envelop.rcpt.remove(index);
                    Ok(())
                },
            )
    }

    #[rhai_fn(global, return_raw, pure)]
    pub fn headers(
        this: &mut Message,
        name: &str,
        count: rhai::INT,
    ) -> EngineResult<rhai::Dynamic> {
        let guard = vsl_guard_ok!(this.read());
        let name_lowercase = name.to_lowercase();
        Ok(guard
            .inner()
            .headers()
            .iter()
            .filter(|(key, _)| key.to_lowercase() == name_lowercase)
            .take(
                usize::try_from(count)
                    .map_err::<Box<rhai::EvalAltResult>, _>(|e| format!("{e}").into())?,
            )
            .map(|(key, value)| format!("{key}:{value}"))
            .collect::<Vec<_>>()
            .into())
    }
}

#[allow(dead_code)]
#[rhai::plugin::export_module]
pub mod message_calling_parse {

    #[rhai_fn(global, return_raw, pure)]
    pub fn rewrite_mail_from_message(this: &mut Message, new_addr: &str) -> EngineResult<()> {
        let new_addr = vsl_conversion_ok!("address", Address::try_from(new_addr.to_string()));

        vsl_parse_ok!(vsl_guard_ok!(this.write())).rewrite_mail_from(new_addr.full());
        Ok(())
    }

    #[rhai_fn(global, return_raw, pure)]
    pub fn rewrite_to_message(
        this: &mut Message,
        old_addr: &str,
        new_addr: &str,
    ) -> EngineResult<()> {
        let new_addr = vsl_conversion_ok!("address", Address::try_from(new_addr.to_string()));
        let old_addr = vsl_conversion_ok!("address", Address::try_from(old_addr.to_string()));

        let mut writer = vsl_guard_ok!(this.write());
        vsl_parse_ok!(writer).rewrite_rcpt(old_addr.full(), new_addr.full());
        Ok(())
    }

    /// add a recipient to the 'To' mail header.
    #[rhai_fn(global, return_raw, pure)]
    pub fn add_to(this: &mut Message, new_addr: &str) -> EngineResult<()> {
        let new_addr = vsl_conversion_ok!("address", Address::try_from(new_addr.to_string()));

        let mut writer = vsl_guard_ok!(this.write());
        vsl_parse_ok!(writer).add_rcpt(new_addr.full());
        Ok(())
    }

    /// remove a recipient from the mail 'To' header.
    #[rhai_fn(global, return_raw, pure)]
    pub fn remove_to(this: &mut Message, addr: &str) -> EngineResult<()> {
        let addr = vsl_conversion_ok!("address", Address::try_from(addr.to_string()));

        let mut writer = vsl_guard_ok!(this.write());
        vsl_parse_ok!(writer).remove_rcpt(addr.full());
        Ok(())
    }
}
