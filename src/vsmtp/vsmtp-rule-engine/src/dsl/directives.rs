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
    mail_context::MailContext, queue::Queue, queue_path, state::StateSMTP, status::Status,
};

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

    #[allow(clippy::too_many_lines)]
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
                    let args = vsl_guard_ok!(state.message().read())
                        .as_ref()
                        .ok_or_else::<rhai::EvalAltResult, _>(|| {
                            "tried to delegate email security but the body was empty".into()
                        })?
                        .get_header_rev("X-VSMTP-DELEGATION")
                        .map(|header| {
                            let header =
                                vsmtp_mail_parser::get_mime_header("X-VSMTP-DELEGATION", header);
                            (
                                header.args.get("id").cloned(),
                                header.args.get("directive").cloned(),
                            )
                        });

                    // FIXME: This check is made twice (once in RuleEngine::run_when).
                    //
                    // If the 'directive' field set in the header matches the name
                    // of the current directive, we pull old context from the working
                    // queue and run the block of code.
                    // Otherwise, we add the X-VSMTP-DELEGATION to the message.
                    return match args {
                        Some((Some(message_id), Some(header_directive)))
                            if header_directive == *name =>
                        {
                            let context_path = queue_path!(
                                &state.server.config.server.queues.dirpath,
                                Queue::Working,
                                &message_id
                            );

                            *state.context().write().unwrap() =
                                MailContext::from_file_path_sync(&context_path)
                                    .map_err::<rhai::EvalAltResult, _>(|_| {
                                    format!("failed to pull old metadata for message {message_id}")
                                        .into()
                                })?;

                            state.engine().call_fn(
                                &mut rhai::Scope::new(),
                                ast,
                                pointer.fn_name(),
                                (),
                            )
                        }
                        _ => {
                            vsl_guard_ok!(state.message().write())
                                .as_mut()
                                .ok_or_else::<rhai::EvalAltResult, _>(|| {
                                    "tried to delegate email security but the body was empty".into()
                                })?
                                .add_header(
                                    "X-VSMTP-DELEGATION",
                                    &format!(
                                        "sent; stage={}; directive=\"{}\"; id=\"{}\"",
                                        smtp_stage,
                                        name,
                                        vsl_guard_ok!(state.context().read())
                                            .metadata
                                            .as_ref()
                                            .unwrap()
                                            .message_id
                                    ),
                                );

                            Ok(Status::Delegated(delegator.clone()))
                        }
                    };
                }

                Err(format!(
                    "cannot delegate security using a '{}' service in {}:'{}'.",
                    service, smtp_stage, name,
                )
                .into())
            }
        }
    }
}
