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

use crate::modules::types::types::Server;
use crate::modules::EngineResult;
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use vsmtp_common::dkim::Key;
use vsmtp_common::dkim::Signature;
use vsmtp_common::re::tokio;

#[doc(hidden)]
#[rhai::plugin::export_module]
pub mod dkim {
    use rhai::EvalAltResult;

    #[rhai_fn(global, return_raw)]
    pub fn parse_signature(input: &str) -> EngineResult<Signature> {
        <Signature as std::str::FromStr>::from_str(input)
            .map_err::<Box<rhai::EvalAltResult>, _>(|e| format!("{e}").into())
    }

    #[rhai_fn(global, pure, return_raw)]
    pub fn get_public_key(server: &mut Server, signature: Signature) -> EngineResult<()> {
        let resolver = server.resolvers.get(&server.config.server.domain).unwrap();

        let result = tokio::task::block_in_place(move || {
            tokio::runtime::Handle::current()
                .block_on(async move { resolver.txt_lookup(signature.get_dns_query()).await })
        })
        .map_err::<Box<EvalAltResult>, _>(|e| format!("{e}").into())?;

        println!("{result:#?}");

        for i in result {
            let key = <Key as std::str::FromStr>::from_str(&format!("{i}"));
            println!("{key:#?}");
        }

        Ok(())
    }
}
