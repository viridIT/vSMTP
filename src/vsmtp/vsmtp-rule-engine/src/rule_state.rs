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
use super::server_api::ServerAPI;
use crate::api::{Context, Message, Server, SharedObject};
use crate::dsl::action::parsing::{create_action, parse_action};
use crate::dsl::delegation::parsing::{create_delegation, parse_delegation};
use crate::dsl::directives::Directive;
use crate::dsl::object::parsing::{create_object, parse_object};
use crate::dsl::rule::parsing::{create_rule, parse_rule};
use crate::dsl::service::parsing::{create_service, parse_service};
use crate::dsl::service::Service;
use crate::rule_engine::RuleEngine;
use vsmtp_common::re::anyhow;
use vsmtp_common::state::StateSMTP;
use vsmtp_common::status::Status;
use vsmtp_common::{
    envelop::Envelop,
    mail_context::{ConnectionContext, MailContext},
    MessageBody,
};
use vsmtp_config::{Config, Resolvers};

/// a state container that bridges rhai's & rust contexts.
pub struct RuleState {
    /// A lightweight engine for evaluation.
    engine: rhai::Engine,
    /// A pointer to the server api.
    pub server: Server,
    /// A pointer to the mail context for the current connection.
    mail_context: Context,
    /// A pointer to the mail body for the current connection.
    message: Message,
    // NOTE: we could replace this property by a `skip` function on
    //       the `Status` enum.
    /// A state to check if the next rules need to be executed or skipped.
    skip: Option<Status>,
}

impl std::fmt::Debug for RuleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuleState")
            .field("mail_context", &self.mail_context)
            .field("message", &self.message)
            .field("skip", &self.skip)
            .finish()
    }
}

impl RuleState {
    /// creates a new rule engine with an empty scope.
    #[must_use]
    pub fn new(
        config: &Config,
        resolvers: std::sync::Arc<Resolvers>,
        rule_engine: &RuleEngine,
    ) -> Self {
        let server = std::sync::Arc::new(ServerAPI {
            config: config.clone(),
            resolvers,
        });
        let mail_context = std::sync::Arc::new(std::sync::RwLock::new(MailContext {
            connection: ConnectionContext {
                timestamp: std::time::SystemTime::now(),
                credentials: None,
                is_authenticated: false,
                is_secured: false,
                server_name: config.server.domain.clone(),
                server_address: config
                    .server
                    .interfaces
                    .addr
                    .get(0)
                    .copied()
                    .unwrap_or_else(|| {
                        "0.0.0.0:0"
                            .parse()
                            .expect("default server address should be parsable")
                    }),
            },
            client_addr: "0.0.0.0:0"
                .parse()
                .expect("default client address should be parsable"),
            envelop: Envelop::default(),
            metadata: None,
        }));
        let message = std::sync::Arc::new(std::sync::RwLock::new(MessageBody::default()));

        let engine = Self::build_rhai_engine(
            mail_context.clone(),
            message.clone(),
            server.clone(),
            rule_engine,
        );

        Self {
            engine,
            server,
            mail_context,
            skip: None,
            message,
        }
    }

    /// create a new rule state with connection data.
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn with_connection(
        config: &Config,
        resolvers: std::sync::Arc<Resolvers>,
        rule_engine: &RuleEngine,
        conn: ConnectionContext,
    ) -> Self {
        let mut state = Self::new(config, resolvers, rule_engine);

        // all rule are skipped until the designated rule
        // in case of a delegation result.
        if rule_engine
            .directives
            .iter()
            .flat_map(|(_, d)| d)
            .any(|d| match d {
                Directive::Delegation { service, .. } => match &**service {
                    Service::Smtp { receiver, .. } => *receiver == conn.server_address,
                    _ => false,
                },
                _ => false,
            })
        {
            state.skip = Some(Status::DelegationResult);
        }

        state.mail_context.write().unwrap().connection = conn;

        state
    }

    /// create a `RuleState` from an existing mail context (f.e. when deserializing a context)
    #[must_use]
    pub fn with_context(
        config: &Config,
        resolvers: std::sync::Arc<Resolvers>,
        rule_engine: &RuleEngine,
        mail_context: MailContext,
        message: MessageBody,
    ) -> Self {
        let server = std::sync::Arc::new(ServerAPI {
            config: config.clone(),
            resolvers,
        });

        // all rule are skipped until the designated rule
        // in case of a delegation result.
        let skip = mail_context
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.skipped.clone());

        let mail_context = std::sync::Arc::new(std::sync::RwLock::new(mail_context));
        let message = std::sync::Arc::new(std::sync::RwLock::new(message));
        let engine = Self::build_rhai_engine(
            mail_context.clone(),
            message.clone(),
            server.clone(),
            rule_engine,
        );

        Self {
            engine,
            server,
            mail_context,
            message,
            skip,
        }
    }

    /// build a cheap rhai engine with vsl's api.
    fn build_rhai_engine(
        mail_context: Context,
        message: Message,
        server: Server,
        rule_engine: &RuleEngine,
    ) -> rhai::Engine {
        let mut engine = rhai::Engine::new_raw();

        // NOTE: on_var is not deprecated, just subject to change in future releases.
        #[allow(deprecated)]
        engine
            // NOTE: why do we have to clone the arc instead of just moving it here ?
            // injecting the state if the current connection into the engine.
            .on_var(move |name, _, _| match name {
                "CTX" => Ok(Some(rhai::Dynamic::from(mail_context.clone()))),
                "SRV" => Ok(Some(rhai::Dynamic::from(server.clone()))),
                "MSG" => Ok(Some(rhai::Dynamic::from(message.clone()))),
                _ => Ok(None),
            })
            .on_print(|msg| println!("{msg}"))
            .register_global_module(rule_engine.std_module.clone())
            .register_global_module(rule_engine.vsl_rhai_module.clone())
            .register_static_module("sys", rule_engine.vsl_native_module.clone())
            .register_static_module("toml", rule_engine.toml_module.clone())
            // FIXME: the following 4 lines should be remove for performance improvement.
            //        need to check out how to construct directives as a module.
            .register_custom_syntax_raw("rule", parse_rule, true, create_rule)
            .register_custom_syntax_raw("action", parse_action, true, create_action)
            .register_custom_syntax_raw("delegate", parse_delegation, true, create_delegation)
            .register_custom_syntax_raw("object", parse_object, true, create_object)
            .register_custom_syntax_raw("service", parse_service, true, create_service)
            .register_iterator::<Vec<vsmtp_common::Address>>()
            .register_iterator::<Vec<SharedObject>>();

        engine
    }

    /// fetch the email context (possibly) mutated by the user's rules.
    #[must_use]
    pub fn context(&self) -> Context {
        self.mail_context.clone()
    }

    /// fetch the message body (possibly) mutated by the user's rules.
    #[must_use]
    pub fn message(&self) -> Message {
        self.message.clone()
    }

    /// Instantiate a [`RuleState`] and run it for the only `state` provided
    ///
    /// # Return
    ///
    /// A tuple with the mail context, body, result status, and skip status.
    #[must_use]
    pub fn just_run_when(
        state: &StateSMTP,
        config: &Config,
        resolvers: std::sync::Arc<Resolvers>,
        rule_engine: &RuleEngine,
        mail_context: MailContext,
        mail_message: MessageBody,
    ) -> (MailContext, MessageBody, Status, Option<Status>) {
        let mut rule_state =
            Self::with_context(config, resolvers, rule_engine, mail_context, mail_message);
        let result = rule_engine.run_when(&mut rule_state, state);

        let (mail_context, mail_message, skipped) = rule_state
            .take()
            .expect("should not have strong reference here");
        (mail_context, mail_message, result, skipped)
    }

    /// Consume the instance and return the inner [`MailContext`] and [`MessageBody`]
    ///
    /// # Errors
    ///
    /// * at least one strong reference of the [`std::sync::Arc`] is living
    /// * the [`std::sync::RwLock`] is poisoned
    pub fn take(self) -> anyhow::Result<(MailContext, MessageBody, Option<Status>)> {
        // early drop of engine because a strong reference is living inside
        let skipped = self.skipped().cloned();
        drop(self.engine);
        Ok((
            std::sync::Arc::try_unwrap(self.mail_context)
                .map_err(|_| {
                    anyhow::anyhow!("strong reference of the field `mail_context` exists")
                })?
                .into_inner()?,
            std::sync::Arc::try_unwrap(self.message)
                .map_err(|_| anyhow::anyhow!("strong reference of the field `message` exists"))?
                .into_inner()?,
            skipped,
        ))
    }

    /// get the engine used to evaluate rules for this state.
    #[must_use]
    pub const fn engine(&self) -> &rhai::Engine {
        &self.engine
    }

    /// have all rules been skipped ?
    #[must_use]
    pub const fn skipped(&self) -> Option<&Status> {
        self.skip.as_ref()
    }

    /// future rule execution need to be skipped for this state.
    pub fn skipping(&mut self, status: Status) {
        self.skip = Some(status);
    }

    /// future rules can be resumed.
    pub fn resume(&mut self) {
        self.skip = None;
    }
}
