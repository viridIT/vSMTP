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
use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, Position, RhaiResult, TypeId,
};

use vsmtp_common::re::log;
use vsmtp_config::log_channel::APP;

///
#[rhai::plugin::export_module]
pub mod logging {

    use crate::modules::types::types::SharedObject;

    /// log a message to the file system / console with the specified level.
    #[rhai_fn(global, name = "log")]
    pub fn log_str_str(level: &str, message: &str) {
        super::log(level, message);
    }

    /// log a message to the file system / console with the specified level.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "log")]
    pub fn log_str_obj(level: &str, message: SharedObject) {
        super::log(level, &message.to_string());
    }

    /// log a message to the file system / console with the specified level.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "log")]
    pub fn log_obj_str(level: &mut SharedObject, message: &str) {
        super::log(&level.to_string(), message);
    }

    /// log a message to the file system / console with the specified level.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "log")]
    pub fn log_obj_obj(level: &mut SharedObject, message: SharedObject) {
        super::log(&level.to_string(), &message.to_string());
    }
}

fn log(level: &str, message: &str) {
    match level {
        "trace" => log::trace!(target: APP, "{}", message),
        "debug" => log::debug!(target: APP, "{}", message),
        "info" => log::info!(target: APP, "{}", message),
        "warn" => log::warn!(target: APP, "{}", message),
        "error" => log::error!(target: APP, "{}", message),
        unknown => log::warn!(
            target: APP,
            "'{}' is not a valid log level. Original message: '{}'",
            unknown,
            message
        ),
    }
}
