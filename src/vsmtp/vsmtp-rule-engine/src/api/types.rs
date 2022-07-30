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
use crate::api::{EngineResult, SharedObject};
use crate::dsl::{object::Object, service::cmd::CmdResult};
use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, Position, RhaiResult, TypeId,
};
use vsmtp_common::status::Status;

#[allow(clippy::needless_pass_by_value)]
#[rhai::plugin::export_module]
mod types_rhai {

    // Status

    /// Operator `==` for `Status`
    #[rhai_fn(global, name = "==", pure)]
    pub fn eq_status_operator(in1: &mut Status, in2: Status) -> bool {
        *in1 == in2
    }

    /// Operator `!=` for `Status`
    #[rhai_fn(global, name = "!=", pure)]
    pub fn neq_status_operator(in1: &mut Status, in2: Status) -> bool {
        !(*in1 == in2)
    }

    /// Convert a `Status` to a `String`
    #[rhai_fn(global, pure)]
    pub fn to_string(status: &mut Status) -> String {
        status.as_ref().to_string()
    }

    /// Convert a `Status` to a debug string
    #[rhai_fn(global, pure)]
    pub fn to_debug(status: &mut Status) -> String {
        status.as_ref().to_string()
    }

    /// Convert a `CmdResult` to a debug string
    #[rhai_fn(global, name = "to_debug")]
    pub fn cmd_result_to_debug(this: &mut CmdResult) -> String {
        format!("{:?}", this)
    }

    /// Convert a `CmdResult` to a `String`
    #[rhai_fn(global, name = "to_string")]
    pub fn cmd_result_to_string(this: &mut CmdResult) -> String {
        format!("{}", this)
    }

    /// Has the `CmdResult` a value?
    #[rhai_fn(global, get = "has_code")]
    pub fn cmd_result_has_code(this: &mut CmdResult) -> bool {
        this.has_code()
    }

    /// Get the `CmdResult` value
    #[rhai_fn(global, get = "code", return_raw)]
    pub fn cmd_result_get_code(this: &mut CmdResult) -> EngineResult<i64> {
        this.get_code().ok_or_else(|| {
            "service result has been terminated by a signal"
                .to_string()
                .into()
        })
    }

    /// Has the `CmdResult` been stopped by a signal?
    #[rhai_fn(global, get = "has_signal")]
    pub fn cmd_result_has_signal(this: &mut CmdResult) -> bool {
        this.has_signal()
    }

    /// Get the `CmdResult` signal value
    #[rhai_fn(global, get = "signal", return_raw)]
    pub fn cmd_result_get_signal(this: &mut CmdResult) -> EngineResult<i64> {
        this.get_signal()
            .ok_or_else(|| "service result has status code".to_string().into())
    }

    // std::time::SystemTime

    /// Convert a `std::time::SystemTime` to a `String`
    #[rhai_fn(global, name = "to_string", return_raw, pure)]
    pub fn time_to_string(this: &mut std::time::SystemTime) -> EngineResult<String> {
        Ok(format!(
            "{}",
            this.duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
                .as_secs()
        ))
    }

    /// Convert a `std::time::SystemTime` to a `String`
    #[rhai_fn(global, name = "to_debug", return_raw, pure)]
    pub fn time_to_debug(this: &mut std::time::SystemTime) -> EngineResult<String> {
        Ok(format!(
            "{:?}",
            this.duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
        ))
    }

    /// Get the `local part` of an email address
    #[rhai_fn(global, get = "local_part", return_raw, pure)]
    pub fn local_part(addr: &mut SharedObject) -> EngineResult<String> {
        match &**addr {
            Object::Address(addr) => Ok(addr.local_part().to_string()),
            other => Err(format!("cannot extract local part for {} object", other.as_ref()).into()),
        }
    }

    /// Get the `domain` of an email address
    #[rhai_fn(global, get = "domain", return_raw, pure)]
    pub fn domain(addr: &mut SharedObject) -> EngineResult<String> {
        match &**addr {
            Object::Address(addr) => Ok(addr.domain().to_string()),
            other => Err(format!("cannot extract domain for {} object", other.as_ref()).into()),
        }
    }

    // vsmtp's rule engine obj syntax (SharedObject).

    /// Convert a `SharedObject` to a `String`
    #[rhai_fn(global, name = "to_string", pure)]
    pub fn object_to_string(this: &mut SharedObject) -> String {
        this.to_string()
    }

    /// Convert a `SharedObject` to a debug string
    #[rhai_fn(global, name = "to_debug", pure)]
    pub fn object_to_debug(this: &mut SharedObject) -> String {
        format!("{:#?}", **this)
    }

    /// Operator `==` for `SharedObject`
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "==", pure)]
    pub fn object_is_self(this: &mut SharedObject, other: SharedObject) -> bool {
        **this == *other
    }

    /// Operator `!=` for `SharedObject`
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "!=", pure)]
    pub fn object_not_self(this: &mut SharedObject, other: SharedObject) -> bool {
        **this != *other
    }

    /// Operator `==` for `SharedObject` and `&str`
    #[rhai_fn(global, name = "==", return_raw, pure)]
    pub fn object_is_string(this: &mut SharedObject, s: &str) -> EngineResult<bool> {
        internal_string_is_object(s, this)
    }

    /// Operator `!=` for `SharedObject` and `&str`
    #[rhai_fn(global, name = "!=", return_raw, pure)]
    pub fn object_not_string(this: &mut SharedObject, s: &str) -> EngineResult<bool> {
        internal_string_is_object(s, this).map(|res| !res)
    }

    /// Operator `==` for `&str` and `SharedObject`
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "==", return_raw)]
    pub fn string_is_object(this: &str, other: SharedObject) -> EngineResult<bool> {
        internal_string_is_object(this, &other)
    }

    /// Operator `!=` for `&str` and `SharedObject`
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "!=", return_raw)]
    pub fn string_not_object(this: &str, other: SharedObject) -> EngineResult<bool> {
        internal_string_is_object(this, &other).map(|res| !res)
    }

    /// Operator `contains`
    #[rhai_fn(global, name = "contains", return_raw, pure)]
    pub fn string_in_object(this: &mut SharedObject, s: &str) -> EngineResult<bool> {
        internal_string_in_object(s, this)
    }

    /// Operator `contains`
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "contains", return_raw, pure)]
    pub fn object_in_object(this: &mut SharedObject, other: SharedObject) -> EngineResult<bool> {
        this.contains(&other)
            .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())
    }

    // vsmtp's rule engine obj syntax container (Vec<SharedObject>).

    /// Convert a `Vec<SharedObject>` to a `String`
    #[rhai_fn(global, name = "to_string", pure)]
    pub fn object_vec_to_string(this: &mut Vec<SharedObject>) -> String {
        format!("{this:?}")
    }

    /// Convert a `Vec<SharedObject>` to a debug string
    #[rhai_fn(global, name = "to_debug", pure)]
    pub fn object_vec_to_debug(this: &mut Vec<SharedObject>) -> String {
        format!("{this:#?}")
    }

    /// Operator `contains`
    #[allow(clippy::needless_pass_by_value, clippy::ptr_arg)]
    #[rhai_fn(global, name = "contains", pure)]
    pub fn string_in_object_vec(this: &mut Vec<SharedObject>, other: &str) -> bool {
        this.iter().any(|obj| obj.to_string() == other)
    }

    /// Operator `contains`
    #[allow(clippy::needless_pass_by_value, clippy::ptr_arg)]
    #[rhai_fn(global, name = "contains", pure)]
    pub fn object_in_object_vec(this: &mut Vec<SharedObject>, other: SharedObject) -> bool {
        this.iter().any(|obj| **obj == *other)
    }

    // rcpt container.

    /// Get the `local parts` of an array of email address
    #[allow(clippy::ptr_arg)]
    #[rhai_fn(global, get = "local_parts", return_raw, pure)]
    pub fn rcpt_local_parts(this: &mut Vec<SharedObject>) -> EngineResult<Vec<SharedObject>> {
        this.iter()
            .map(|rcpt| match &**rcpt {
                Object::Address(addr) => Ok(std::sync::Arc::new(Object::Identifier(
                    addr.local_part().to_string(),
                ))),
                other => Err(
                    format!("cannot extract local part from a {} object", other.as_ref()).into(),
                ),
            })
            .collect::<EngineResult<Vec<SharedObject>>>()
    }

    /// Get the `domains` of an array of email address
    #[allow(clippy::ptr_arg)]
    #[rhai_fn(global, get = "domains", return_raw, pure)]
    pub fn rcpt_domains(this: &mut Vec<SharedObject>) -> EngineResult<Vec<SharedObject>> {
        this.iter()
            .map(|rcpt| match &**rcpt {
                Object::Address(addr) => {
                    Ok(std::sync::Arc::new(Object::Fqdn(addr.domain().to_string())))
                }
                other => {
                    Err(format!("cannot extract domain from a {} object", other.as_ref()).into())
                }
            })
            .collect::<EngineResult<Vec<SharedObject>>>()
    }
}

pub use types_rhai::*;

// the following methods are used to compare recursively deep objects
// using refs instead of shared rhai objects.
// FIXME: using generics here should be a good idea.
// TODO:  all comparison function should return an error in case of mismatching types.

pub(crate) fn internal_string_is_object(this: &str, other: &Object) -> EngineResult<bool> {
    match other {
        Object::Address(addr) => Ok(this == addr.full()),
        Object::Fqdn(fqdn) => Ok(this == fqdn),
        Object::Regex(re) => Ok(re.is_match(this)),
        Object::Str(s) | Object::Identifier(s) => Ok(this == s),
        _ => Err(format!("a {} object cannot be compared to a string", other).into()),
    }
}

pub(crate) fn internal_string_in_object(this: &str, other: &Object) -> EngineResult<bool> {
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
