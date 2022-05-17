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
use rhai::{
    plugin::{
        mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
        PluginFunction, RhaiResult, TypeId,
    },
    EvalAltResult,
};

#[rhai::plugin::export_module]
pub mod security {
    use crate::modules::{
        types::types::{Context, Server},
        EngineResult,
    };

    #[rhai_fn(return_raw, pure)]
    pub fn check_spf(ctx: &mut Context, srv: Server, identity: &str) -> EngineResult<()> {
        let (helo, mail_from, ip) = {
            let ctx = &ctx
                .read()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?;

            (
                ctx.envelop.helo.clone(),
                ctx.envelop.mail_from.clone(),
                ctx.client_addr.ip(),
            )
        };
        let config = viaspf::Config::default();
        let handle = tokio::runtime::Handle::current();
        let resolver = srv
            .resolvers
            .get(mail_from.domain())
            .ok_or_else::<Box<EvalAltResult>, _>(|| {
                format!("dns configuration for {} not found.", mail_from.domain()).into()
            })?;

        match identity {
            "mail_from" => {
                let sender = viaspf::Sender::new(mail_from.full()).unwrap();
                let helo_domain = helo.parse().ok();

                let _ = handle.block_on(viaspf::evaluate_sender(
                    resolver,
                    &config,
                    ip,
                    &sender,
                    helo_domain.as_ref(),
                ));

                Ok(())
            }
            "helo" => {
                let sender = viaspf::Sender::from_domain(&helo).unwrap();
                let helo_domain = sender.domain();

                let _ = handle.block_on(viaspf::evaluate_sender(
                    resolver,
                    &config,
                    ip,
                    &sender,
                    Some(helo_domain),
                ));

                Ok(())
            }
            _ => Err("you can only perform a spf query on mail_from or helo identities".into()),
        }
    }
}
