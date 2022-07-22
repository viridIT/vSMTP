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
use crate::modules::types::types::{Context, Server};
use crate::modules::EngineResult;
use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use vsmtp_common::rcpt::Rcpt;
use vsmtp_common::{auth::Credentials, auth::Mechanism, state::StateSMTP, Address};

#[rhai::plugin::export_module]
pub mod mail_context {
    use crate::{dsl::object::Object, modules::types::types::SharedObject};

    #[rhai_fn(global, get = "client_address", return_raw, pure)]
    pub fn client_address(context: &mut Context) -> EngineResult<String> {
        Ok(vsl_guard_ok!(context.read()).client_addr.to_string())
    }

    #[rhai_fn(global, get = "client_ip", return_raw, pure)]
    pub fn client_ip(context: &mut Context) -> EngineResult<String> {
        Ok(vsl_guard_ok!(context.read()).client_addr.ip().to_string())
    }

    #[rhai_fn(global, get = "client_port", return_raw, pure)]
    pub fn client_port(context: &mut Context) -> EngineResult<i64> {
        Ok(i64::from(vsl_guard_ok!(context.read()).client_addr.port()))
    }

    #[rhai_fn(global, get = "server_address", return_raw, pure)]
    pub fn server_address(context: &mut Context) -> EngineResult<String> {
        Ok(context
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .connection
            .server_address
            .to_string())
    }

    #[rhai_fn(global, get = "server_ip", return_raw, pure)]
    pub fn server_ip(context: &mut Context) -> EngineResult<std::net::IpAddr> {
        Ok(context
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .connection
            .server_address
            .ip())
    }

    #[rhai_fn(global, get = "server_port", return_raw, pure)]
    pub fn server_port(context: &mut Context) -> EngineResult<i64> {
        Ok(i64::from(
            context
                .read()
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
                .connection
                .server_address
                .port(),
        ))
    }

    #[rhai_fn(global, get = "connection_timestamp", return_raw, pure)]
    pub fn connection_timestamp(context: &mut Context) -> EngineResult<std::time::SystemTime> {
        Ok(vsl_guard_ok!(context.read()).connection.timestamp)
    }

    #[rhai_fn(global, get = "server_name", return_raw, pure)]
    pub fn server_name(context: &mut Context) -> EngineResult<String> {
        Ok(vsl_guard_ok!(context.read()).connection.server_name.clone())
    }

    #[rhai_fn(global, get = "is_secured", return_raw, pure)]
    pub fn is_secured(context: &mut Context) -> EngineResult<bool> {
        Ok(vsl_guard_ok!(context.read()).connection.is_secured)
    }

    #[rhai_fn(global, get = "is_authenticated", return_raw, pure)]
    pub fn is_authenticated(context: &mut Context) -> EngineResult<bool> {
        Ok(vsl_guard_ok!(context.read()).connection.is_authenticated)
    }

    #[rhai_fn(global, get = "auth", return_raw, pure)]
    pub fn auth(context: &mut Context) -> EngineResult<Credentials> {
        Ok(vsl_missing_ok!(
            vsl_guard_ok!(context.read()).connection.credentials,
            "auth",
            StateSMTP::Authenticate(Mechanism::Anonymous, None)
        )
        .clone())
    }

    #[rhai_fn(global, get = "type", pure)]
    pub fn get_type(credentials: &mut Credentials) -> String {
        credentials.to_string()
    }

    #[rhai_fn(global, get = "authid", return_raw, pure)]
    pub fn get_authid(credentials: &mut Credentials) -> EngineResult<String> {
        match credentials {
            Credentials::Query { authid } | Credentials::Verify { authid, .. } => {
                Ok(authid.clone())
            }
            Credentials::AnonymousToken { .. } => {
                Err(format!("no `authid` available in credentials of type `{credentials}`").into())
            }
        }
    }

    #[rhai_fn(global, get = "authpass", return_raw, pure)]
    pub fn get_authpass(credentials: &mut Credentials) -> EngineResult<String> {
        match credentials {
            Credentials::Verify { authpass, .. } => Ok(authpass.clone()),
            _ => Err(
                format!("no `authpass` available in credentials of type `{credentials}`").into(),
            ),
        }
    }

    #[rhai_fn(global, get = "anonymous_token", return_raw, pure)]
    pub fn get_anonymous_token(credentials: &mut Credentials) -> EngineResult<String> {
        match credentials {
            Credentials::AnonymousToken { token } => Ok(token.clone()),
            _ => Err(format!(
                "no `anonymous_token` available in credentials of type `{credentials}`"
            )
            .into()),
        }
    }

    #[rhai_fn(global, get = "helo", return_raw, pure)]
    pub fn helo(context: &mut Context) -> EngineResult<String> {
        Ok(vsl_guard_ok!(context.read()).envelop.helo.clone())
    }

    #[rhai_fn(global, get = "mail_from", return_raw, pure)]
    pub fn mail_from(context: &mut Context) -> EngineResult<SharedObject> {
        Ok(std::sync::Arc::new(Object::Address(
            vsl_guard_ok!(context.read()).envelop.mail_from.clone(),
        )))
    }

    #[rhai_fn(global, get = "rcpt_list", return_raw, pure)]
    pub fn rcpt_list(context: &mut Context) -> EngineResult<Vec<SharedObject>> {
        Ok(vsl_guard_ok!(context.read())
            .envelop
            .rcpt
            .iter()
            .map(|rcpt| std::sync::Arc::new(Object::Address(rcpt.address.clone())))
            .collect())
    }

    #[rhai_fn(global, get = "rcpt", return_raw, pure)]
    pub fn rcpt(context: &mut Context) -> EngineResult<SharedObject> {
        Ok(std::sync::Arc::new(Object::Address(
            vsl_missing_ok!(
                vsl_guard_ok!(context.read()).envelop.rcpt.last(),
                "rcpt",
                StateSMTP::RcptTo
            )
            .address
            .clone(),
        )))
    }

    #[rhai_fn(global, get = "mail_timestamp", return_raw, pure)]
    pub fn mail_timestamp(context: &mut Context) -> EngineResult<std::time::SystemTime> {
        Ok(vsl_missing_ok!(
            vsl_guard_ok!(context.read()).metadata,
            "mail_timestamp",
            StateSMTP::PreQ
        )
        .timestamp)
    }

    #[rhai_fn(global, get = "message_id", return_raw, pure)]
    pub fn message_id(context: &mut Context) -> EngineResult<String> {
        Ok(vsl_missing_ok!(
            vsl_guard_ok!(context.read()).metadata,
            "message_id",
            StateSMTP::PreQ
        )
        .message_id
        .clone())
    }

    #[rhai_fn(global, name = "to_string", pure)]
    pub fn ctx_to_string(_: &mut Context) -> String {
        "MailContext".to_string()
    }

    #[rhai_fn(global, name = "to_debug", pure)]
    pub fn ctx_to_debug(context: &mut Context) -> String {
        ctx_to_string(context)
    }

    #[rhai_fn(global, name = "to_string", pure)]
    pub fn srv_to_string(_: &mut Server) -> String {
        "Server".to_string()
    }

    #[rhai_fn(global, name = "to_debug", pure)]
    pub fn srv_to_debug(context: &mut Server) -> String {
        srv_to_string(context)
    }

    /// Change the sender of the envelop.
    #[rhai_fn(global, name = "rewrite_mail_from_envelop", return_raw, pure)]
    pub fn rewrite_mail_from_envelop_str(
        context: &mut Context,
        new_addr: &str,
    ) -> EngineResult<()> {
        super::rewrite_mail_from_envelop(context, new_addr)
    }

    /// Change the sender of the envelop using an object.
    #[rhai_fn(global, name = "rewrite_mail_from_envelop", return_raw, pure)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn rewrite_mail_from_envelop_obj(
        context: &mut Context,
        new_addr: SharedObject,
    ) -> EngineResult<()> {
        super::rewrite_mail_from_envelop(context, &new_addr.to_string())
    }

    /// Replace a recipient of the envelop.
    #[rhai_fn(global, name = "rewrite_rcpt_envelop", return_raw, pure)]
    pub fn rewrite_rcpt_str_str(
        context: &mut Context,
        old_addr: &str,
        new_addr: &str,
    ) -> EngineResult<()> {
        super::rewrite_rcpt(context, old_addr, new_addr)
    }

    /// Replace a recipient of the envelop.
    #[rhai_fn(global, name = "rewrite_rcpt_envelop", return_raw, pure)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn rewrite_rcpt_obj_str(
        context: &mut Context,
        old_addr: SharedObject,
        new_addr: &str,
    ) -> EngineResult<()> {
        super::rewrite_rcpt(context, &old_addr.to_string(), new_addr)
    }

    /// Replace a recipient of the envelop.
    #[rhai_fn(global, name = "rewrite_rcpt_envelop", return_raw, pure)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn rewrite_rcpt_str_obj(
        context: &mut Context,
        old_addr: &str,
        new_addr: SharedObject,
    ) -> EngineResult<()> {
        super::rewrite_rcpt(context, old_addr, &new_addr.to_string())
    }

    /// Replace a recipient of the envelop.
    #[rhai_fn(global, name = "rewrite_rcpt_envelop", return_raw, pure)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn rewrite_rcpt_obj_obj(
        context: &mut Context,
        old_addr: SharedObject,
        new_addr: SharedObject,
    ) -> EngineResult<()> {
        super::rewrite_rcpt(context, &old_addr.to_string(), &new_addr.to_string())
    }

    /// add a recipient to the envelop.
    #[rhai_fn(global, name = "add_rcpt_envelop", return_raw, pure)]
    pub fn add_rcpt_str(context: &mut Context, new_addr: &str) -> EngineResult<()> {
        super::add_rcpt(context, new_addr)
    }

    /// add a recipient to the envelop.
    #[rhai_fn(global, name = "add_rcpt_envelop", return_raw, pure)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn add_rcpt_obj(context: &mut Context, new_addr: SharedObject) -> EngineResult<()> {
        super::add_rcpt(context, &new_addr.to_string())
    }

    /// remove a recipient from the envelop.
    #[rhai_fn(global, name = "remove_rcpt_envelop", return_raw, pure)]
    pub fn remove_rcpt_envelop_str(context: &mut Context, addr: &str) -> EngineResult<()> {
        super::remove_rcpt_envelop(context, addr)
    }

    /// remove a recipient from the envelop.
    #[rhai_fn(global, name = "remove_rcpt_envelop", return_raw, pure)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn remove_rcpt_envelop_obj(context: &mut Context, addr: SharedObject) -> EngineResult<()> {
        super::remove_rcpt_envelop(context, &addr.to_string())
    }
}

/// internal generic function to rewrite the `mail_from` value of the envelop.
fn rewrite_mail_from_envelop(context: &mut Context, new_addr: &str) -> EngineResult<()> {
    vsl_guard_ok!(context.write()).envelop.mail_from =
        vsl_conversion_ok!("address", Address::try_from(new_addr.to_string()));
    Ok(())
}

/// internal generic function to rewrite a recipient of the envelop.
fn rewrite_rcpt(context: &mut Context, old_addr: &str, new_addr: &str) -> EngineResult<()> {
    let old_addr = vsl_conversion_ok!("address", Address::try_from(old_addr.to_string()));
    let new_addr = vsl_conversion_ok!("address", Address::try_from(new_addr.to_string()));

    let mut context = vsl_guard_ok!(context.write());

    context.envelop.rcpt.push(Rcpt::new(new_addr));

    if let Some(index) = context
        .envelop
        .rcpt
        .iter()
        .position(|rcpt| rcpt.address == old_addr)
    {
        context.envelop.rcpt.swap_remove(index);
    }
    Ok(())
}

/// internal generic function to add a recipient to the envelop.
fn add_rcpt(context: &mut Context, new_addr: &str) -> EngineResult<()> {
    vsl_guard_ok!(context.write())
        .envelop
        .rcpt
        .push(Rcpt::new(vsl_conversion_ok!(
            "address",
            Address::try_from(new_addr.to_string())
        )));

    Ok(())
}

/// internal generic function to remove a recipient to the envelop.
fn remove_rcpt_envelop(context: &mut Context, addr: &str) -> EngineResult<()> {
    let addr = vsl_conversion_ok!("address", Address::try_from(addr.to_string()));

    let mut email = vsl_guard_ok!(context.write());

    email
        .envelop
        .rcpt
        .iter()
        .position(|rcpt| rcpt.address == addr)
        .map_or_else(
            || Ok(()),
            |index| {
                email.envelop.rcpt.remove(index);
                Ok(())
            },
        )
}
