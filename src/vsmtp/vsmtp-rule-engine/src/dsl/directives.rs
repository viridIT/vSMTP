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

use crate::{modules::EngineResult, rule_state::RuleState, vsl_guard_ok, Service};
use vsmtp_common::{
    rcpt::Rcpt,
    re::{
        anyhow::{self, Context},
        lettre::{self, Transport},
    },
    state::StateSMTP,
    status::Status,
    Address,
};

use super::service::SmtpConnection;

/// a set of directives, filtered by smtp stage.
pub type Directives = std::collections::BTreeMap<String, Vec<Directive>>;

/// a type of rule that can be executed from a function pointer.
pub enum Directive {
    /// execute code that return a status.
    Rule { name: String, pointer: rhai::FnPtr },
    /// execute code that does not need a return value.
    Action { name: String, pointer: rhai::FnPtr },
    /// delegate a message to a service, and execute the
    /// inner rhai function when the message is forwared
    /// to the service receive endpoint.
    Delegation {
        name: String,
        pointer: rhai::FnPtr,
        service: std::sync::Arc<Service>,
    },
}

impl Directive {
    pub const fn directive_type(&self) -> &str {
        match self {
            Directive::Rule { .. } => "rule",
            Directive::Action { .. } => "action",
            Directive::Delegation { .. } => "delegate",
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Directive::Rule { name, .. }
            | Directive::Action { name, .. }
            | Directive::Delegation { name, .. } => name,
        }
    }

    pub fn execute(
        &self,
        state: &mut RuleState,
        ast: &rhai::AST,
        smtp_stage: &StateSMTP,
    ) -> EngineResult<Status> {
        match self {
            Directive::Rule { pointer, .. } => {
                state
                    .engine()
                    .call_fn(&mut rhai::Scope::new(), ast, pointer.fn_name(), ())
            }
            Directive::Action { pointer, .. } => {
                state
                    .engine()
                    .call_fn(&mut rhai::Scope::new(), ast, pointer.fn_name(), ())?;

                Ok(Status::Next)
            }
            Directive::Delegation {
                pointer,
                service,
                name,
            } => {
                if let Service::Smtp { delegator, .. } = &**service {
                    let (from, rcpt, body) = {
                        let ctx = state.context();
                        let ctx = vsl_guard_ok!(ctx.read());

                        let msg = state.message();
                        let mut msg = vsl_guard_ok!(msg.write());
                        let msg = msg.as_mut().ok_or_else::<rhai::EvalAltResult, _>(|| {
                            "tried to delegate email security but the body was empty".into()
                        })?;

                        let body = if msg.get_header("X-VSMTP-DELEGATION").is_some() {
                            // we received delegation results, we do not delegate & execute the body
                            // of the directive.
                            return state.engine().call_fn(
                                &mut rhai::Scope::new(),
                                ast,
                                pointer.fn_name(),
                                (),
                            );
                        } else {
                            msg.add_header(
                                "X-VSMTP-DELEGATION",
                                &format!(
                                    "sent; stage={}; directive={}; id={}",
                                    smtp_stage,
                                    pointer.fn_name(),
                                    ctx.metadata.as_ref().unwrap().message_id
                                ),
                            );

                            msg.to_string()
                        };

                        (
                            ctx.envelop.mail_from.clone(),
                            ctx.envelop.rcpt.clone(),
                            body,
                        )
                    };

                    let delegator =
                        delegator
                            .lock()
                            .map_err::<Box<rhai::EvalAltResult>, _>(|err| {
                                format!(
                                "delegation connector for the '{}' smtp service is poisoned: {}",
                                name, err
                            )
                                .into()
                            })?;

                    delegate(&*delegator, &from, &rcpt[..], body.as_bytes()).map_err::<Box<
                        rhai::EvalAltResult,
                    >, _>(
                        |err| {
                            format!(
                                "failed to delegate message using {} in {}:'{}' : {}",
                                name,
                                smtp_stage,
                                pointer.fn_name(),
                                err
                            )
                            .into()
                        },
                    )?;

                    Ok(Status::Delegated)
                } else {
                    Err(format!(
                        "cannot delegate security using the '{}' service in {}:'{}'.",
                        name,
                        smtp_stage,
                        pointer.fn_name()
                    )
                    .into())
                }
            }
        }
    }
}

fn delegate(
    delegator: &SmtpConnection,
    from: &Address,
    rcpt: &[Rcpt],
    body: &[u8],
) -> anyhow::Result<lettre::transport::smtp::response::Response> {
    let envelope = lettre::address::Envelope::new(
        Some(from.full().parse()?),
        rcpt.iter()
            .map(|rcpt| {
                rcpt.address
                    .full()
                    .parse::<lettre::Address>()
                    .with_context(|| format!("failed to parse address {}", rcpt.address.full()))
            })
            .collect::<anyhow::Result<Vec<_>>>()?,
    )?;

    Ok(delegator.0.send_raw(&envelope, body)?)
}
