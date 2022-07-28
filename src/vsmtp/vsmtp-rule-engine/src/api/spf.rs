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
use crate::api::{
    EngineResult, {Context, Server},
};
use rhai::{
    plugin::{
        mem, Dynamic, FnAccess, FnNamespace, Module, NativeCallContext, PluginFunction, RhaiResult,
        TypeId,
    },
    EvalAltResult,
};
use vsmtp_common::re::tokio;

pub use security::*;

#[rhai::plugin::export_module]
mod security {

    /// evaluate a sender identity.
    /// the identity parameter can be 'helo', 'mail_from' or 'both'.
    /// # Results
    /// a rhai Map with:
    //    * result (String) : the result of an SPF evaluation.
    //    * cause  (String) : the "mechanism" that matched or the "problem" error (RFC 7208-9.1).
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(return_raw, pure)]
    pub fn check_spf(ctx: &mut Context, srv: Server) -> EngineResult<rhai::Map> {
        fn query_spf(
            resolver: &impl viaspf::lookup::Lookup,
            ip: std::net::IpAddr,
            sender: &viaspf::Sender,
        ) -> rhai::Map {
            let result = tokio::task::block_in_place(move || {
                tokio::runtime::Handle::current().block_on(async move {
                    viaspf::evaluate_sender(resolver, &viaspf::Config::default(), ip, sender, None)
                        .await
                })
            });

            map_from_query_result(&result)
        }

        let (mail_from, ip) = {
            let ctx = &ctx
                .read()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?;
            (ctx.envelop.mail_from.clone(), ctx.client_addr.ip())
        };

        let resolver = srv.resolvers.get(&srv.config.server.domain).unwrap();

        Ok(match mail_from.full().parse() {
            Ok(sender) => query_spf(resolver, ip, &sender),
            _ => rhai::Map::from_iter([("result".into(), "none".into())]),
        })
    }
}

/// create a instance from viaspf query result struct.
#[must_use]
fn map_from_query_result(q_result: &viaspf::QueryResult) -> rhai::Map {
    rhai::Map::from_iter([
        (
            "result".into(),
            rhai::Dynamic::from(q_result.spf_result.to_string()),
        ),
        {
            q_result.cause.as_ref().map_or(
                ("mechanism".into(), rhai::Dynamic::from("default")),
                |cause| match cause {
                    viaspf::SpfResultCause::Match(mechanism) => (
                        "mechanism".into(),
                        rhai::Dynamic::from(mechanism.to_string()),
                    ),
                    viaspf::SpfResultCause::Error(error) => {
                        ("problem".into(), rhai::Dynamic::from(error.to_string()))
                    }
                },
            )
        },
    ])
}
