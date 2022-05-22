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

    /// evaluate a sender identity.
    /// the identity parameter can ether be 'mail_from' or 'helo'.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(return_raw, pure)]
    pub fn check_spf(ctx: &mut Context, srv: Server, identity: &str) -> EngineResult<rhai::Map> {
        fn query_spf(
            resolver: &impl viaspf::lookup::Lookup,
            ip: std::net::IpAddr,
            sender: &viaspf::Sender,
            helo_domain: Option<&viaspf::DomainName>,
        ) -> rhai::Map {
            let result = tokio::task::block_in_place(move || {
                tokio::runtime::Handle::current().block_on(async move {
                    viaspf::evaluate_sender(
                        resolver,
                        &viaspf::Config::default(),
                        ip,
                        sender,
                        helo_domain,
                    )
                    .await
                })
            });

            map_from_query_result(&result)
        }

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
                dbg!("mail from: ");
                let sender = viaspf::Sender::new(mail_from.full())
                    .map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())?;
                let helo_domain = helo.parse().ok();
                Ok(query_spf(resolver, ip, &sender, helo_domain.as_ref()))
            }
            "helo" => {
                dbg!("helo: ", &helo);
                let sender = viaspf::Sender::from_domain(&helo)
                    .map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())?;
                Ok(query_spf(resolver, ip, &sender, Some(sender.domain())))
            }
            _ => Err("you can only perform a spf query on mail_from or helo identities".into()),
        }
    }
}

/// create a instance from viaspf query result struct.
pub fn map_from_query_result(q_result: &viaspf::QueryResult) -> rhai::Map {
    rhai::Map::from_iter([
        (
            "result".into(),
            rhai::Dynamic::from(q_result.spf_result.to_string()),
        ),
        (
            "cause".into(),
            q_result
                .cause
                .as_ref()
                .map_or(rhai::Dynamic::from("default"), |cause| match cause {
                    viaspf::SpfResultCause::Match(mechanism) => {
                        rhai::Dynamic::from(mechanism.to_string())
                    }
                    viaspf::SpfResultCause::Error(error) => rhai::Dynamic::from(error.to_string()),
                }),
        ),
    ])
}
