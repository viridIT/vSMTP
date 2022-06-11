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
use crate::dsl::object::Object;
use crate::dsl::service::cmd::CmdResult;
use crate::modules::EngineResult;
use crate::server_api::ServerAPI;
use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, Position, RhaiResult, TypeId,
};
use vsmtp_common::mail_context::MailContext;
use vsmtp_common::mail_context::MessageBody;
use vsmtp_common::rcpt::Rcpt;
use vsmtp_common::status::Status;
use vsmtp_common::Address;

#[allow(dead_code)]
#[allow(clippy::needless_pass_by_value)]
#[rhai::plugin::export_module]
pub mod types {

    // type aliases for complex struct names
    pub type Context = std::sync::Arc<std::sync::RwLock<MailContext>>;
    pub type Message = std::sync::Arc<std::sync::RwLock<Option<MessageBody>>>;
    pub type Server = std::sync::Arc<ServerAPI>;

    // Status

    #[rhai_fn(global, name = "==", pure)]
    pub fn eq_status_operator(in1: &mut Status, in2: Status) -> bool {
        *in1 == in2
    }

    #[rhai_fn(global, name = "!=", pure)]
    pub fn neq_status_operator(in1: &mut Status, in2: Status) -> bool {
        !(*in1 == in2)
    }

    #[rhai_fn(global, pure)]
    pub fn to_string(status: &mut Status) -> String {
        status.as_ref().to_string()
    }

    // note: same as to_string ?
    #[rhai_fn(global, pure)]
    pub fn to_debug(status: &mut Status) -> String {
        status.as_ref().to_string()
    }

    #[rhai_fn(global, name = "to_debug")]
    pub fn cmd_result_to_debug(this: &mut CmdResult) -> String {
        format!("{:?}", this)
    }

    #[rhai_fn(global, name = "to_string")]
    pub fn cmd_result_to_string(this: &mut CmdResult) -> String {
        format!("{}", this)
    }

    #[rhai_fn(global, get = "has_code")]
    pub fn cmd_result_has_code(this: &mut CmdResult) -> bool {
        this.has_code()
    }

    #[rhai_fn(global, get = "code", return_raw)]
    pub fn cmd_result_get_code(this: &mut CmdResult) -> EngineResult<i64> {
        this.get_code().ok_or_else(|| {
            "service result has been terminated by a signal"
                .to_string()
                .into()
        })
    }

    #[rhai_fn(global, get = "has_signal")]
    pub fn cmd_result_has_signal(this: &mut CmdResult) -> bool {
        this.has_signal()
    }

    #[rhai_fn(global, get = "signal", return_raw)]
    pub fn cmd_result_get_signal(this: &mut CmdResult) -> EngineResult<i64> {
        this.get_signal()
            .ok_or_else(|| "service result has status code".to_string().into())
    }

    // std::time::SystemTime

    #[rhai_fn(global, name = "to_string", return_raw, pure)]
    pub fn time_to_string(this: &mut std::time::SystemTime) -> EngineResult<String> {
        Ok(format!(
            "{}",
            this.duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
                .as_secs()
        ))
    }

    #[rhai_fn(global, name = "to_debug", return_raw, pure)]
    pub fn time_to_debug(this: &mut std::time::SystemTime) -> EngineResult<String> {
        Ok(format!(
            "{:?}",
            this.duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
        ))
    }

    // std::net::IpAddr

    #[rhai_fn(global, name = "to_string", pure)]
    pub fn ip_to_string(this: &mut std::net::IpAddr) -> String {
        this.to_string()
    }

    #[rhai_fn(global, name = "to_debug", pure)]
    pub fn ip_to_debug(this: &mut std::net::IpAddr) -> String {
        format!("{this:?}")
    }

    #[rhai_fn(global, name = "==", pure)]
    pub fn ip_is_string(this: &mut std::net::IpAddr, ip: &str) -> bool {
        this.to_string() == ip
    }

    #[rhai_fn(global, name = "!=", pure)]
    pub fn ip_not_string(this: &mut std::net::IpAddr, ip: &str) -> bool {
        this.to_string() != ip
    }

    // rules::address::Address

    #[rhai_fn(global, return_raw)]
    pub fn new_address(addr: &str) -> EngineResult<Address> {
        Address::try_from(addr.to_string()).map_err(|error| error.to_string().into())
    }

    #[rhai_fn(global, name = "to_string", pure)]
    pub fn address_to_string(this: &mut Address) -> String {
        this.full().to_string()
    }

    #[rhai_fn(global, name = "to_debug", pure)]
    pub fn address_to_debug(this: &mut Address) -> String {
        format!("{this:?}")
    }

    #[rhai_fn(global, get = "local_part", pure)]
    pub fn local_part(this: &mut Address) -> String {
        this.local_part().to_string()
    }

    #[rhai_fn(global, get = "domain", pure)]
    pub fn domain(this: &mut Address) -> String {
        this.domain().to_string()
    }

    #[rhai_fn(global, name = "==", pure)]
    pub fn address_is_string(this: &mut Address, other: &str) -> bool {
        this.full() == other
    }

    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "==", pure)]
    pub fn address_is_self(this: &mut Address, other: Address) -> bool {
        *this == other
    }

    // NOTE: should a mismatched object fail or just return false ?
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "==", return_raw, pure)]
    pub fn address_is_object(
        this: &mut Address,
        other: std::sync::Arc<Object>,
    ) -> EngineResult<bool> {
        internal_address_is_object(this, &other)
    }

    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "==", return_raw, pure)]
    pub fn object_is_address(
        this: &mut std::sync::Arc<Object>,
        addr: Address,
    ) -> EngineResult<bool> {
        internal_address_is_object(&addr, this)
    }

    #[rhai_fn(global, name = "!=", pure)]
    pub fn address_not_string(this: &mut Address, other: &str) -> bool {
        this.full() != other
    }

    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "!=", pure)]
    pub fn address_not_self(this: &mut Address, other: Address) -> bool {
        *this != other
    }

    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "!=", return_raw, pure)]
    pub fn address_not_object(
        this: &mut Address,
        other: std::sync::Arc<Object>,
    ) -> EngineResult<bool> {
        internal_address_is_object(this, &other).map(|r| !r)
    }

    // vsmtp's rule engine obj syntax (std::sync::Arc<Object>).

    #[rhai_fn(global, name = "to_string", pure)]
    pub fn object_to_string(this: &mut std::sync::Arc<Object>) -> String {
        this.to_string()
    }

    #[rhai_fn(global, name = "to_debug", pure)]
    pub fn object_to_debug(this: &mut std::sync::Arc<Object>) -> String {
        format!("{:#?}", **this)
    }

    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "==", pure)]
    pub fn object_is_self(
        this: &mut std::sync::Arc<Object>,
        other: std::sync::Arc<Object>,
    ) -> bool {
        **this == *other
    }

    #[rhai_fn(global, name = "==", return_raw, pure)]
    pub fn object_is_string(this: &mut std::sync::Arc<Object>, s: &str) -> EngineResult<bool> {
        internal_string_is_object(s, this)
    }

    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "==", return_raw)]
    pub fn string_is_object(this: &str, other: std::sync::Arc<Object>) -> EngineResult<bool> {
        internal_string_is_object(this, &other)
    }

    #[rhai_fn(global, name = "contains", return_raw, pure)]
    pub fn string_in_object(this: &mut std::sync::Arc<Object>, s: &str) -> EngineResult<bool> {
        internal_string_in_object(s, this)
    }

    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "contains", return_raw, pure)]
    pub fn address_in_object(
        this: &mut std::sync::Arc<Object>,
        addr: Address,
    ) -> EngineResult<bool> {
        internal_address_in_object(&addr, this)
    }

    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "contains", return_raw, pure)]
    pub fn object_in_object(
        this: &mut std::sync::Arc<Object>,
        other: std::sync::Arc<Object>,
    ) -> EngineResult<bool> {
        internal_object_in_object(&other, this.as_ref())
    }

    // vsmtp's rule engine obj syntax container (Vec<std::sync::Arc<Object>>).

    #[rhai_fn(global, name = "to_string", pure)]
    pub fn object_vec_to_string(this: &mut Vec<std::sync::Arc<Object>>) -> String {
        format!("{:?}", this)
    }

    #[rhai_fn(global, name = "to_debug", pure)]
    pub fn object_vec_to_debug(this: &mut Vec<std::sync::Arc<Object>>) -> String {
        format!("{:#?}", this)
    }

    #[allow(clippy::needless_pass_by_value, clippy::ptr_arg)]
    #[rhai_fn(global, name = "contains", pure)]
    pub fn object_in_object_vec(
        this: &mut Vec<std::sync::Arc<Object>>,
        other: std::sync::Arc<Object>,
    ) -> bool {
        this.iter().any(|obj| **obj == *other)
    }

    // rcpt container.

    #[allow(clippy::ptr_arg)]
    #[rhai_fn(global, get = "local_parts", pure)]
    pub fn rcpt_local_parts(this: &mut Vec<Address>) -> Vec<std::sync::Arc<Object>> {
        this.iter()
            .map(|rcpt| std::sync::Arc::new(Object::Identifier(rcpt.local_part().to_string())))
            .collect()
    }

    #[allow(clippy::ptr_arg)]
    #[rhai_fn(global, get = "domains", pure)]
    pub fn rcpt_domains(this: &mut Vec<Address>) -> Vec<std::sync::Arc<Object>> {
        this.iter()
            .map(|rcpt| std::sync::Arc::new(Object::Fqdn(rcpt.domain().to_string())))
            .collect()
    }

    #[rhai_fn(global, name = "to_string", pure)]
    pub fn rcpt_to_string(this: &mut Vec<Rcpt>) -> String {
        format!("{this:?}")
    }

    #[rhai_fn(global, name = "to_debug", pure)]
    pub fn rcpt_to_debug(this: &mut Vec<Rcpt>) -> String {
        format!("{this:#?}")
    }

    #[allow(clippy::ptr_arg)]
    #[rhai_fn(global, name = "contains", return_raw, pure)]
    pub fn string_in_rcpt(this: &mut Vec<Address>, s: &str) -> EngineResult<bool> {
        let addr = Address::try_from(s.to_string())
            .map_err::<Box<EvalAltResult>, _>(|_| format!("'{}' is not an address", s).into())?;
        Ok(this.iter().any(|rcpt| *rcpt == addr))
    }

    #[allow(clippy::needless_pass_by_value, clippy::ptr_arg)]
    #[rhai_fn(global, name = "contains", pure)]
    pub fn address_in_rcpt(this: &mut Vec<Address>, addr: Address) -> bool {
        this.iter().any(|rcpt| *rcpt == addr)
    }

    #[allow(clippy::needless_pass_by_value, clippy::ptr_arg)]
    #[rhai_fn(global, name = "contains", return_raw, pure)]
    pub fn object_in_rcpt(
        this: &mut Vec<Address>,
        other: std::sync::Arc<Object>,
    ) -> EngineResult<bool> {
        internal_object_in_rcpt(this, &other)
    }
}

// the following methods are used to compare recursively deep objects
// using refs instead of shared rhai objects.

pub fn internal_string_is_object(this: &str, other: &Object) -> EngineResult<bool> {
    match other {
        Object::Address(addr) => Ok(this == addr.full()),
        Object::Fqdn(fqdn) => Ok(this == fqdn),
        Object::Regex(re) => Ok(re.is_match(this)),
        Object::Str(s) | Object::Identifier(s) => Ok(this == s),
        _ => Err(format!("a {} object cannot be compared to a string", other).into()),
    }
}

pub fn internal_string_in_object(this: &str, other: &Object) -> EngineResult<bool> {
    match other {
        Object::Group(group) => Ok(group.iter().any(|obj| internal_string_is_object(this, obj).unwrap_or(false))),
        Object::File(file) => Ok(file.iter().any(|obj| internal_string_is_object(this, obj).unwrap_or(false))),
        _ => {
             Err(format!(
                "the 'in' operator can only be used with 'group' and 'file' object types, you used the string {} with the object {}",
                this,
                other
            )
            .into())
        }
    }
}

pub fn internal_address_is_object(this: &Address, other: &Object) -> EngineResult<bool> {
    Ok(match &*other {
        Object::Address(addr) => this == addr,
        Object::Fqdn(fqdn) => this.domain() == fqdn,
        Object::Regex(re) => re.is_match(this.full()),
        Object::Identifier(s) => this.local_part() == s,
        Object::Str(s) => this.full() == s,
        _ => return Err(format!("a {} object cannot be compared to an address", other).into()),
    })
}

pub fn internal_address_in_object(this: &Address, other: &Object) -> EngineResult<bool> {
    Ok(match &*other {
        Object::Group(group) => group.iter().any(|obj| internal_address_is_object(this, obj).unwrap_or(false)),
        Object::File(file) => file.iter().any(|obj| internal_address_is_object(this, obj).unwrap_or(false)),
        _ => {
            return Err(format!(
                "the 'in' operator can only be used with 'group' and 'file' object types, you used the address {} with the object {}",
                this.full(),
                other
            )
            .into())
        }
    })
}

pub fn internal_object_in_object(this: &Object, other: &Object) -> EngineResult<bool> {
    Ok(match &*other {
        Object::Group(group) => group.iter().any(|obj| **obj == *this),
        Object::File(file) => file.iter().any(|obj| obj == this),
        _ => {
            return Err(format!(
                "the 'in' operator can only be used with 'group' and 'file' object types, you used the object {} to search in {}",
                this,
                other
            )
            .into())
        }
    })
}

pub fn internal_object_in_rcpt(this: &[Address], other: &Object) -> EngineResult<bool> {
    Ok(match &*other {
        Object::Address(addr) => this.iter().any(|rcpt| rcpt == addr),
        Object::Fqdn(fqdn) => this.iter().any(|rcpt| rcpt.domain() == fqdn),
        Object::Regex(re) => this.iter().any(|rcpt| !re.is_match(rcpt.full())),
        Object::File(file) => file
            .iter()
            .any(|obj| internal_object_in_rcpt(this, obj).unwrap_or(false)),
        Object::Group(group) => group
            .iter()
            .any(|obj| internal_object_in_rcpt(this, obj).unwrap_or(false)),
        Object::Identifier(s) => this.iter().any(|rcpt| rcpt.local_part() == s),
        Object::Str(s) => this.iter().any(|rcpt| rcpt.full() == s),
        _ => {
            return Err(format!(
                "a {} object cannot be compared to the rcpt container",
                other
            )
            .into())
        }
    })
}
