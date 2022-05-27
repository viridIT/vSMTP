use crate::dsl::action::parsing::{create_action, parse_action};
use crate::dsl::object::parsing::{create_object, parse_object};
use crate::dsl::object::Object;
use crate::dsl::rule::parsing::{create_rule, parse_rule};
use crate::dsl::service::parsing::{create_service, parse_service};
use crate::rule_engine::RuleEngine;

use super::server_api::ServerAPI;
use vsmtp_common::envelop::Envelop;
use vsmtp_common::mail_context::{ConnectionContext, MailContext, MessageBody};
use vsmtp_common::status::Status;
use vsmtp_config::{Config, Resolvers};

/// a state container that bridges rhai's & rust contexts.
pub struct RuleState {
    /// a lightweight engine for evaluation.
    engine: rhai::Engine,
    /// a pointer to the server api.
    #[allow(dead_code)]
    server: std::sync::Arc<ServerAPI>,
    /// a pointer to the mail context for the current connection.
    mail_context: std::sync::Arc<std::sync::RwLock<MailContext>>,
    /// does the following rules needs to be skipped ?
    skip: Option<Status>,
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
            },
            client_addr: std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
                0,
            ),
            envelop: Envelop::default(),
            body: MessageBody::Empty,
            metadata: None,
        }));
        let engine = Self::build_rhai_engine(&mail_context, &server, rule_engine);

        Self {
            engine,
            server,
            mail_context,
            skip: None,
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
        let state = Self::new(config, resolvers, rule_engine);
        state.mail_context.write().unwrap().connection = conn;
        state
    }

    /// create a RuleState from an existing mail context (f.e. when deserializing a context)
    #[must_use]
    pub fn with_context(
        config: &Config,
        resolvers: std::sync::Arc<Resolvers>,
        rule_engine: &RuleEngine,
        mail_context: MailContext,
    ) -> Self {
        let server = std::sync::Arc::new(ServerAPI {
            config: config.clone(),
            resolvers,
        });
        let mail_context = std::sync::Arc::new(std::sync::RwLock::new(mail_context));
        let engine = Self::build_rhai_engine(&mail_context, &server, rule_engine);

        Self {
            engine,
            server,
            mail_context,
            skip: None,
        }
    }

    /// build a cheap rhai engine with vsl's api.
    fn build_rhai_engine(
        mail_context: &std::sync::Arc<std::sync::RwLock<MailContext>>,
        server: &std::sync::Arc<ServerAPI>,
        rule_engine: &RuleEngine,
    ) -> rhai::Engine {
        let mut engine = rhai::Engine::new_raw();

        let mail_context = mail_context.clone();
        let server = server.clone();

        // NOTE: on_var is not deprecated, just subject to change in futur releases.
        #[allow(deprecated)]
        engine
            // NOTE: why do we have to clone the arc twice instead of just moving it here ?
            // injecting the state if the current connection into the engine.
            .on_var(move |name, _, _| match name {
                "CTX" => Ok(Some(rhai::Dynamic::from(mail_context.clone()))),
                "SRV" => Ok(Some(rhai::Dynamic::from(server.clone()))),
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
            .register_custom_syntax_raw("object", parse_object, true, create_object)
            .register_custom_syntax_raw("service", parse_service, true, create_service)
            .register_iterator::<Vec<vsmtp_common::Address>>()
            .register_iterator::<Vec<std::sync::Arc<Object>>>();

        engine
    }

    /// fetch the email context (possibly) mutated by the user's rules.
    #[must_use]
    pub fn context(&self) -> std::sync::Arc<std::sync::RwLock<MailContext>> {
        self.mail_context.clone()
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
}
