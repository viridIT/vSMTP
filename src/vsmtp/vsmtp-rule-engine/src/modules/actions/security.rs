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
        PluginFunction, Position, RhaiResult, TypeId,
    },
    EvalAltResult,
};

#[rhai::plugin::export_module]
pub mod security {
    use crate::modules::{
        types::types::{Context, Server},
        EngineResult,
    };

    #[derive(Default, Clone)]
    pub struct SpfResult {
        pub result: String,
        pub cause: String,
    }

    impl SpfResult {
        /// create a instance from viaspf query result struct.
        pub fn from_query_result(q_result: viaspf::QueryResult) -> Self {
            Self {
                result: q_result.spf_result.to_string(),
                cause: q_result
                    .cause
                    .map_or("default".to_string(), |cause| match cause {
                        viaspf::SpfResultCause::Match(mechanism) => mechanism.to_string(),
                        viaspf::SpfResultCause::Error(error) => error.to_string(),
                    }),
            }
        }
    }

    /// evaluate a sender identity.
    /// the identity parameter can ether be 'mail_from' or 'helo'.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(return_raw, pure)]
    pub fn check_spf(ctx: &mut Context, srv: Server, identity: &str) -> EngineResult<SpfResult> {
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
                format!(
                    "no dns configuration found for {} while checking spf.",
                    mail_from.domain()
                )
                .into()
            })?;

        match identity {
            "mail_from" => {
                let sender = viaspf::Sender::new(mail_from.full()).unwrap();
                let helo_domain = helo.parse().ok();

                Ok(SpfResult::from_query_result(handle.block_on(
                    viaspf::evaluate_sender(resolver, &config, ip, &sender, helo_domain.as_ref()),
                )))
            }
            "helo" => {
                let sender = viaspf::Sender::from_domain(&helo).unwrap();
                let helo_domain = sender.domain();

                Ok(SpfResult::from_query_result(handle.block_on(
                    viaspf::evaluate_sender(resolver, &config, ip, &sender, Some(helo_domain)),
                )))
            }
            _ => Err("you can only perform a spf query on mail_from or helo identities".into()),
        }
    }

    #[rhai_fn(get = "result")]
    pub fn get_spf_result(spf: &mut SpfResult) -> String {
        spf.result.clone()
    }

    #[rhai_fn(get = "cause")]
    pub fn get_spf_cause(spf: &mut SpfResult) -> String {
        spf.cause.clone()
    }
}
