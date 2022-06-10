use crate::{
    modules::{self, EngineResult},
    rule_state::RuleState,
    Service,
};
use vsmtp_common::status::Status;

/// a set of directives, filtered by smtp stage.
pub type Directives = std::collections::BTreeMap<String, Vec<Box<dyn Directive + Send + Sync>>>;

/// a directive rhai code and that can be executed, return a status.
pub trait Directive {
    fn directive_type(&self) -> &'static str;
    fn execute(&self, state: &mut RuleState, ast: &rhai::AST) -> EngineResult<Status>;
    fn name(&self) -> &str;
}

/// a rule, that returns an evaluated Status.
pub struct Rule {
    pub name: String,
    pub pointer: rhai::FnPtr,
}

impl Directive for Rule {
    fn directive_type(&self) -> &'static str {
        "rule"
    }

    fn execute(&self, state: &mut RuleState, ast: &rhai::AST) -> EngineResult<Status> {
        state
            .engine()
            .call_fn(&mut rhai::Scope::new(), ast, self.pointer.fn_name(), ())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// an action, that alway return the status Next.
pub struct Action {
    pub name: String,
    pub pointer: rhai::FnPtr,
}

impl Directive for Action {
    fn directive_type(&self) -> &'static str {
        "action"
    }

    fn execute(&self, state: &mut RuleState, ast: &rhai::AST) -> EngineResult<Status> {
        state
            .engine()
            .call_fn(&mut rhai::Scope::new(), ast, self.pointer.fn_name(), ())?;

        Ok(Status::Next)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// a delegation send the email to another service & execute
/// the underlying scope when the email is returned to the server.
pub struct Delegation {
    pub name: String,
    pub pointer: rhai::FnPtr,
    pub service: std::sync::Arc<Service>,
}

impl Directive for Delegation {
    fn directive_type(&self) -> &'static str {
        "delegate"
    }

    fn execute(&self, state: &mut RuleState, ast: &rhai::AST) -> EngineResult<Status> {
        if let Service::Smtp {
            delegator,
            receiver,
            ..
        } = &*self.service
        {
            let (from, rcpt, body) = {
                let ctx = state.context();
                let ctx = ctx
                    .read()
                    .map_err::<Box<rhai::EvalAltResult>, _>(|_| "context mutex poisoned".into())?;

                // Delegated message has been returned to the server.
                // We then just execute the rest of the directive.
                if ctx.connection.server_address == *receiver {
                    return state.engine().call_fn(
                        &mut rhai::Scope::new(),
                        ast,
                        self.pointer.fn_name(),
                        (),
                    );
                }

                let body = state
                    .message()
                    .read()
                    .map_err::<Box<rhai::EvalAltResult>, _>(|_| "context mutex poisoned".into())?
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

                crate::dsl::service::smtp::delegate(&mut *delegator, &from, &rcpt, body.as_bytes())
                    .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())?;
            }

            Ok(Status::Delegated)
        } else {
            Err(format!("cannot delegate security with '{}' service.", self.name).into())
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}
