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
use crate::error::RuntimeError;
use crate::modules::types::types::{Context, Message, Server};
use crate::modules::EngineResult;
use rhai::plugin::{
    Dynamic, EvalAltResult, FnAccess, FnNamespace, Module, NativeCallContext, PluginFunction,
    RhaiResult, TypeId,
};
use vsmtp_common::mail_context::AuthCredentials;
use vsmtp_common::re::anyhow;
use vsmtp_common::Address;

#[doc(hidden)]
#[allow(dead_code)]
#[rhai::plugin::export_module]
pub mod mail_context {

    #[rhai_fn(global, get = "client_ip", return_raw, pure)]
    pub fn client_ip(this: &mut Context) -> EngineResult<std::net::IpAddr> {
        Ok(vsl_guard_ok!(this.read()).client_addr.ip())
    }

    #[rhai_fn(global, get = "client_port", return_raw, pure)]
    pub fn client_port(this: &mut Context) -> EngineResult<i64> {
        Ok(i64::from(vsl_guard_ok!(this.read()).client_addr.port()))
    }

    #[rhai_fn(global, get = "connection_timestamp", return_raw, pure)]
    pub fn connection_timestamp(this: &mut Context) -> EngineResult<std::time::SystemTime> {
        Ok(vsl_guard_ok!(this.read()).connection.timestamp)
    }

    #[rhai_fn(global, get = "server_name", return_raw, pure)]
    pub fn server_name(this: &mut Context) -> EngineResult<String> {
        Ok(vsl_guard_ok!(this.read()).connection.server_name.clone())
    }

    #[rhai_fn(global, get = "is_secured", return_raw, pure)]
    pub fn is_secured(this: &mut Context) -> EngineResult<bool> {
        Ok(vsl_guard_ok!(this.read()).connection.is_secured)
    }

    #[rhai_fn(global, get = "is_authenticated", return_raw, pure)]
    pub fn is_authenticated(this: &mut Context) -> EngineResult<bool> {
        Ok(vsl_guard_ok!(this.read()).connection.is_authenticated)
    }

    #[rhai_fn(global, get = "auth", return_raw, pure)]
    pub fn auth(this: &mut Context) -> EngineResult<AuthCredentials> {
        Ok(vsl_missing_ok!(vsl_guard_ok!(this.read()).connection.credentials, "auth").clone())
    }

    #[rhai_fn(global, get = "type", pure)]
    pub fn get_type(my_enum: &mut AuthCredentials) -> String {
        match my_enum {
            AuthCredentials::Verify { .. } => "Verify".to_string(),
            AuthCredentials::Query { .. } => "Query".to_string(),
        }
    }

    #[rhai_fn(global, get = "authid", pure)]
    pub fn get_authid(my_enum: &mut AuthCredentials) -> String {
        match my_enum {
            AuthCredentials::Query { authid } | AuthCredentials::Verify { authid, .. } => {
                authid.clone()
            }
        }
    }

    #[rhai_fn(global, get = "authpass", return_raw, pure)]
    pub fn get_authpass(my_enum: &mut AuthCredentials) -> EngineResult<String> {
        match my_enum {
            AuthCredentials::Verify { authpass, .. } => Ok(authpass.clone()),
            AuthCredentials::Query { .. } => {
                Err("no `authpass` available in credentials of type `Query`"
                    .to_string()
                    .into())
            }
        }
    }

    #[rhai_fn(global, get = "helo", return_raw, pure)]
    pub fn helo(this: &mut Context) -> EngineResult<String> {
        Ok(vsl_guard_ok!(this.read()).envelop.helo.clone())
    }

    #[rhai_fn(global, get = "mail_from", return_raw, pure)]
    pub fn mail_from(this: &mut Context) -> EngineResult<Address> {
        Ok(vsl_guard_ok!(this.read()).envelop.mail_from.clone())
    }

    #[rhai_fn(global, get = "rcpt", return_raw, pure)]
    pub fn rcpt(this: &mut Context) -> EngineResult<Vec<Address>> {
        Ok(vsl_guard_ok!(this.read())
            .envelop
            .rcpt
            .iter()
            .map(|rcpt| rcpt.address.clone())
            .collect())
    }

    #[rhai_fn(global, get = "mail_timestamp", return_raw, pure)]
    pub fn mail_timestamp(this: &mut Context) -> EngineResult<std::time::SystemTime> {
        Ok(vsl_missing_ok!(vsl_guard_ok!(this.read()).metadata, "mail_timestamp").timestamp)
    }

    #[rhai_fn(global, get = "message_id", return_raw, pure)]
    pub fn message_id(this: &mut Context) -> EngineResult<String> {
        Ok(
            vsl_missing_ok!(vsl_guard_ok!(this.read()).metadata, "message_id")
                .message_id
                .clone(),
        )
    }

    #[rhai_fn(global, get = "mail", return_raw, pure)]
    pub fn mail(this: &mut Message) -> EngineResult<String> {
        Ok(vsl_missing_ok!(vsl_guard_ok!(this.read()), "mail").to_string())
    }

    #[rhai_fn(global, name = "to_string", pure)]
    pub fn ctx_to_string(_: &mut Context) -> String {
        "MailContext".to_string()
    }

    #[rhai_fn(global, name = "to_debug", pure)]
    pub fn ctx_to_debug(this: &mut Context) -> String {
        ctx_to_string(this)
    }

    #[rhai_fn(global, name = "to_string", pure)]
    pub fn srv_to_string(_: &mut Server) -> String {
        "Server".to_string()
    }

    #[rhai_fn(global, name = "to_debug", pure)]
    pub fn srv_to_debug(this: &mut Server) -> String {
        srv_to_string(this)
    }
}
