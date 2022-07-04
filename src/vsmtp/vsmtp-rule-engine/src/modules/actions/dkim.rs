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

use crate::modules::types::types::{Message, Server};
use crate::modules::EngineResult;
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use rhai::EvalAltResult;
use vsmtp_common::re::tokio;
use vsmtp_dkim::{Key, Signature};

#[doc(hidden)]
#[rhai::plugin::export_module]
pub mod dkim {

    #[rhai_fn(global, return_raw)]
    pub fn parse_signature(input: &str) -> EngineResult<Signature> {
        <Signature as std::str::FromStr>::from_str(input)
            .map_err::<Box<rhai::EvalAltResult>, _>(|e| format!("{e}").into())
    }

    #[rhai_fn(global, pure, return_raw)]
    pub fn has_expired(signature: &mut Signature, epsilon: rhai::INT) -> EngineResult<bool> {
        match epsilon.try_into() {
            Ok(epsilon) => Ok(signature.has_expired(epsilon)),
            e => Err(e.unwrap_err().to_string().into()),
        }
    }

    #[rhai_fn(global, pure, return_raw)]
    pub fn get_public_key(
        server: &mut Server,
        signature: Signature,
        on_multiple_key_records: &str,
    ) -> EngineResult<rhai::Dynamic> {
        const VALID_POLICY: [&str; 2] = ["first", "cycle"];
        if !VALID_POLICY.contains(&on_multiple_key_records) {
            return Err(format!(
                "expected values in `{}` but got `{on_multiple_key_records}`",
                VALID_POLICY.join(",")
            )
            .into());
        }

        let resolver = server.resolvers.get(&server.config.server.domain).unwrap();

        let result = tokio::task::block_in_place(move || {
            tokio::runtime::Handle::current().block_on(signature.get_public_key(resolver))
        })
        .map_err::<Box<EvalAltResult>, _>(|e| format!("{e}").into())?;

        let keys = result.into_iter().filter_map(|i| {
            match <Key as std::str::FromStr>::from_str(&format!("{i}")) {
                Ok(key) => Some(key),
                Err(e) => {
                    println!("got error with key: `{e}`");
                    None
                }
            }
        });

        Ok(if on_multiple_key_records == "first" {
            keys.take(1).collect::<Vec<_>>()
        } else {
            keys.collect::<Vec<_>>()
        }
        .into())
    }

    #[rhai_fn(global, pure, get = "has_debug_flag")]
    pub fn has_debug_flag(key: &mut Key) -> bool {
        key.has_debug_flag()
    }

    #[allow(clippy::module_name_repetitions, clippy::needless_pass_by_value)]
    #[rhai_fn(global, pure, return_raw)]
    pub fn dkim_verify(message: &mut Message, signature: Signature, key: Key) -> EngineResult<()> {
        let guard = vsl_guard_ok!(message.read());

        signature
            .verify(guard.inner(), &key)
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

        Ok(())
    }
}
