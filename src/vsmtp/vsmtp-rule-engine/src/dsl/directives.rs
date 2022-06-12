use crate::{modules::EngineResult, rule_state::RuleState, Service};
use vsmtp_common::status::Status;

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

    pub fn execute(&self, state: &mut RuleState, ast: &rhai::AST) -> EngineResult<Status> {
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
                if let Service::Smtp {
                    delegator,
                    receiver,
                    ..
                } = &**service
                {
                    let (from, rcpt, body) = {
                        let ctx = state.context();
                        let ctx = ctx.read().map_err::<Box<rhai::EvalAltResult>, _>(|_| {
                            "context mutex poisoned".into()
                        })?;

                        // Delegated message has been returned to the server.
                        // We then just execute the rest of the directive.
                        if ctx.connection.server_address == *receiver {
                            return state.engine().call_fn(
                                &mut rhai::Scope::new(),
                                ast,
                                pointer.fn_name(),
                                (),
                            );
                        }

                        let body = state
                            .message()
                            .read()
                            .map_err::<Box<rhai::EvalAltResult>, _>(|_| {
                                "context mutex poisoned".into()
                            })?
                            .as_ref()
                            .map(std::string::ToString::to_string)
                            .ok_or_else::<Box<rhai::EvalAltResult>, _>(|| {
                                "tried to delegate email security but the body was empty".into()
                            })?;

                        (
                            ctx.envelop.mail_from.clone(),
                            ctx.envelop.rcpt.clone(),
                            body,
                        )
                    };

                    {
                        let mut delegator = delegator.lock().unwrap();

                        crate::dsl::service::smtp::delegate(
                            &mut *delegator,
                            &from,
                            &rcpt,
                            body.as_bytes(),
                        )
                        .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())?;
                    }

                    Ok(Status::Delegated)
                } else {
                    Err(format!("cannot delegate security with '{}' service.", name).into())
                }
            }
        }
    }
}
