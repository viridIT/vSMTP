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
use crate::api::SharedObject;
use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, Position, RhaiResult, TypeId,
};
use vsmtp_common::re::log;

#[rhai::plugin::export_module]
mod logging_rhai {

    /// log a message to the file system / console with the specified level.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "log")]
    pub fn log_str_obj(level: &str, message: SharedObject) {
        log(level, &message.to_string());
    }

    /// log a message to the file system / console with the specified level.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "log")]
    pub fn log_obj_str(level: &mut SharedObject, message: &str) {
        log(&level.to_string(), message);
    }

    /// log a message to the file system / console with the specified level.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "log")]
    pub fn log_obj_obj(level: &mut SharedObject, message: SharedObject) {
        log(&level.to_string(), &message.to_string());
    }

    /// log a message to the file system / console with the specified level.
    #[rhai_fn(global, name = "log")]
    pub fn log(level: &str, message: &str) {
        const APP_TARGET: &str = "app";

        match <log::Level as std::str::FromStr>::from_str(level) {
            Ok(level) => log::log!(target: APP_TARGET, level, "{message}"),
            Err(e) => log::warn!(
                target: APP_TARGET,
                "Got an error with level `{level}`: `{e}`. Message was: '{message}'"
            ),
        }
    }
}

pub use logging_rhai::*;
