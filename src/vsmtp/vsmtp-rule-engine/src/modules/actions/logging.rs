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
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};

#[doc(hidden)]
#[rhai::plugin::export_module]
pub mod logging {
    use vsmtp_common::re::log;
    use vsmtp_config::log_channel::APP;

    ///
    pub fn log(level: &str, message: &str) {
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
}
