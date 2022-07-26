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

///
#[rhai::plugin::export_module]
pub mod logging {
    use vsmtp_common::re::log;

    ///
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
