/**
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
 **/
use anyhow::Context;
use rhai::module_resolvers::FileModuleResolver;
use rhai::{exported_module, plugin::EvalAltResult, Engine, Scope, AST};
use vsmtp_common::envelop::Envelop;
use vsmtp_common::mail_context::{Body, ConnectionContext, MailContext};
use vsmtp_common::re::{anyhow, log};
use vsmtp_common::state::StateSMTP;
use vsmtp_common::status::Status;
use vsmtp_config::Config;

use crate::dsl::action_parsing::{create_action, parse_action};
use crate::dsl::object_parsing::{create_object, parse_object};
use crate::dsl::rule_parsing::{create_rule, parse_rule};
use crate::error::RuleEngineError;
use crate::obj::Object;
use crate::{log_channels, modules};

use super::server_api::ServerAPI;

const DATE_FORMAT: &[time::format_description::FormatItem<'_>] =
    time::macros::format_description!("[year]-[month]-[day]");
const TIME_FORMAT: &[time::format_description::FormatItem<'_>] =
    time::macros::format_description!("[hour]:[minute]:[second]");

///
pub struct RuleState<'a> {
    scope: Scope<'a>,
    #[allow(unused)]
    server: std::sync::Arc<ServerAPI>,
    mail_context: std::sync::Arc<std::sync::RwLock<MailContext>>,
    skip: Option<Status>,
}

impl<'a> RuleState<'a> {
    /// creates a new rule engine with an empty scope.
    #[must_use]
    pub fn new(config: &Config) -> Self {
        let mut scope = Scope::new();
        let server = std::sync::Arc::new(ServerAPI {
            config: config.clone(),
        });

        let mail_context = std::sync::Arc::new(std::sync::RwLock::new(MailContext {
            connection: ConnectionContext {
                timestamp: std::time::SystemTime::now(),
                credentials: None,
                is_authenticated: false,
                is_secured: false,
                server_name: "testserver.com".to_string(),
            },
            client_addr: std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
                0,
            ),
            envelop: Envelop::default(),
            body: Body::Empty,
            metadata: None,
        }));

        scope
            .push("date", "")
            .push("time", "")
            .push("srv", server.clone())
            .push("ctx", mail_context.clone());

        Self {
            scope,
            server,
            mail_context,
            skip: None,
        }
    }

    /// create a new rule state with connection data.
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn with_connection(config: &Config, conn: ConnectionContext) -> Self {
        let state = Self::new(config);
        state.mail_context.write().unwrap().connection = conn;
        state
    }

    /// create a RuleState from an existing mail context (f.e. when deserializing a context)
    #[must_use]
    pub fn with_context(config: &Config, mail_context: MailContext) -> Self {
        let mut scope = Scope::new();
        let server = std::sync::Arc::new(ServerAPI {
            config: config.clone(),
        });
        let mail_context = std::sync::Arc::new(std::sync::RwLock::new(mail_context));

        scope
            .push("date", "")
            .push("time", "")
            .push("srv", server.clone())
            .push("ctx", mail_context.clone());

        Self {
            scope,
            server,
            mail_context,
            skip: None,
        }
    }

    /// add data to the scope of the engine.
    pub(crate) fn add_data<T>(&mut self, name: &'a str, data: T) -> &mut Self
    where
        T: Clone + Send + Sync + 'static,
    {
        self.scope.set_or_push(name, data);
        self
    }

    /// fetch the email context (possibly) mutated by the user's rules.
    #[must_use]
    pub fn get_context(&self) -> std::sync::Arc<std::sync::RwLock<MailContext>> {
        self.mail_context.clone()
    }

    ///
    #[must_use]
    pub fn skipped(&self) -> Option<Status> {
        self.skip.clone()
    }
}

/// a sharable rhai engine.
/// contains an ast representation of the user's parsed .vsl script files
/// and objects parsed from rhai's context to rust's. This way,
/// they can be used directly into rust functions, and the engine
/// doesn't need to evaluate them each call.
/// the engine also stores a user cache that is used to fetch
/// data about system users.
pub struct RuleEngine {
    /// rhai's engine structure.
    pub(super) context: Engine,
    /// ast built from the user's .vsl files.
    pub(super) ast: AST,
    // system user cache, used for retrieving user information. (used in vsl.USER_EXISTS for example)
    // pub(super) users: Mutex<U>,
}

impl RuleEngine {
    /// runs all rules from a stage using the current transaction state.
    pub fn run_when(&self, rule_state: &mut RuleState, smtp_state: &StateSMTP) -> Status {
        if let Some(status) = &rule_state.skip {
            return status.clone();
        }

        let now = time::OffsetDateTime::now_utc();

        rule_state
            .add_data(
                "date",
                now.format(&DATE_FORMAT)
                    .unwrap_or_else(|_| String::default()),
            )
            .add_data(
                "time",
                now.format(&TIME_FORMAT)
                    .unwrap_or_else(|_| String::default()),
            );

        let rules = match self
            .context
            .eval_ast_with_scope::<rhai::Map>(&mut rule_state.scope, &self.ast)
        {
            Ok(rules) => rules,
            Err(error) => {
                log::error!(
                    target: log_channels::RE,
                    "smtp_stage '{}' skipped => rule engine failed to evaluate rules:\n\t{}",
                    smtp_state,
                    error
                );
                return Status::Next;
            }
        };

        match self.context.call_fn(
            &mut rule_state.scope,
            &self.ast,
            "run_rules",
            (rules, smtp_state.to_string()),
        ) {
            Ok(status) => {
                log::debug!(
                    target: log_channels::RE,
                    "[{}] evaluated => {:?}.",
                    smtp_state,
                    status
                );

                if let Status::Faccept | Status::Deny(_) = status {
                    log::debug!(
                        target: log_channels::RE,
                        "[{}] the rule engine will skip all rules because of the previous result.",
                        smtp_state
                    );
                    rule_state.skip = Some(status.clone());
                }
                status
            }
            Err(error) => {
                log::error!(
                    target: log_channels::RE,
                    "{}",
                    Self::parse_stage_error(error, smtp_state)
                );
                Status::Next
            }
        }
    }

    fn parse_stage_error(error: Box<EvalAltResult>, smtp_state: &StateSMTP) -> String {
        match *error {
            // NOTE: since all errors are caught and thrown in "run_rules", errors
            //       are always wrapped in ErrorInFunctionCall.
            EvalAltResult::ErrorInFunctionCall(_, _, error, _) => match *error {
                EvalAltResult::ErrorRuntime(error, _) if error.is::<rhai::Map>() => {
                    let error = error.cast::<rhai::Map>();
                    let rule = error
                        .get("rule")
                        .map_or_else(|| "unknown rule".to_string(), ToString::to_string);
                    let error = error.get("message").map_or_else(
                        || "vsl internal unexpected error".to_string(),
                        ToString::to_string,
                    );

                    format!(
                        "stage '{}' skipped => rule engine failed in '{}':\n\t{}",
                        smtp_state, rule, error
                    )
                }
                _ => {
                    format!(
                        "stage '{}' skipped => rule engine failed:\n\t{}",
                        smtp_state, error,
                    )
                }
            },
            // NOTE: all errors are caught in "run_rules", should this code be replaced
            //       with `unreachable!` ?
            _ => {
                format!(
                    "rule engine unexpected error in stage '{}':\n\t{:?}",
                    smtp_state, error
                )
            }
        }
    }

    /// creates a new instance of the rule engine, reading all files in the
    /// `script_path` parameter.
    /// if `script_path` is `None`, an warning is emitted and a deny-all script
    /// is loaded.
    ///
    /// # Errors
    /// * failed to register `script_path` as a valid module folder.
    /// * failed to compile or load any script located at `script_path`.
    pub fn new(config: &Config, script_path: &Option<std::path::PathBuf>) -> anyhow::Result<Self> {
        let mut engine = Self::new_raw(config)?;

        engine.set_module_resolver(match script_path {
            Some(script_path) => FileModuleResolver::new_with_path_and_extension(
                script_path.parent().ok_or_else(|| {
                    anyhow::anyhow!(
                        "File '{}' is not a valid root directory for rules",
                        script_path.display()
                    )
                })?,
                "vsl",
            ),
            None => FileModuleResolver::new_with_extension("vsl"),
        });

        log::debug!(target: log_channels::RE, "compiling rhai scripts ...");

        let ast = if let Some(script_path) = &script_path {
            Self::compile_executor(
                &engine,
                &std::fs::read_to_string(&script_path)
                    .context(format!("Failed to read file: '{}'", script_path.display()))?,
            )
        } else {
            log::warn!(
                target: log_channels::RE,
                "No 'main.vsl' provided in the config, the server will deny any incoming transaction by default.",
            );

            Self::compile_executor(&engine, include_str!("default_rules.rhai"))
        }?;

        log::debug!(target: log_channels::RE, "done.");

        Ok(Self {
            context: engine,
            ast,
        })
    }

    /// create a rhai engine with vsl's configuration.
    fn new_raw(config: &Config) -> anyhow::Result<rhai::Engine> {
        let mut engine = Engine::new();

        let server_config = &vsmtp_common::re::serde_json::to_string(&config.server)
            .context("failed to convert the server configuration to json")?
            .replace('{', "#{");

        let app_config = &vsmtp_common::re::serde_json::to_string(&config.app)
            .context("failed to convert the app configuration to json")?
            .replace('{', "#{");

        let mut vsl_module = rhai::Module::new();
        let mut toml_module = rhai::Module::new();

        let ast = engine
            .compile_scripts_with_scope(
                &rhai::Scope::new(),
                [include_str!("api/utils.rhai"), include_str!("api/api.rhai")],
            )
            .context("failed to compile vsl's api")?;

        let api = rhai::Module::eval_ast_as_new(rhai::Scope::new(), &ast, &engine)
            .context("failed to create vsl's api module")?;

        // setting up action, mail context & vsl's special types.
        vsl_module
            .combine(exported_module!(modules::actions::bcc::bcc))
            .combine(exported_module!(modules::actions::headers::headers))
            .combine(exported_module!(modules::actions::logging::logging))
            .combine(exported_module!(modules::actions::rule_state::rule_state))
            .combine(exported_module!(modules::actions::services::services))
            .combine(exported_module!(modules::actions::transports::transports))
            .combine(exported_module!(modules::actions::utils::utils))
            .combine(exported_module!(modules::actions::write::write))
            .combine(exported_module!(modules::types::types))
            .combine(exported_module!(modules::mail_context::mail_context));

        // setting up toml configuration injection.
        toml_module
            .set_var("server", engine.parse_json(server_config, true)?)
            .set_var("app", engine.parse_json(app_config, true)?);

        engine
            .register_static_module("vsl", vsl_module.into())
            .register_static_module("toml", toml_module.into())
            .register_global_module(api.into())
            .disable_symbol("eval")
            .on_parse_token(|token, _, _| {
                match token {
                    // remap 'is' operator to '==', it's easier than creating a new operator.
                    // NOTE: warning => "is" is a reserved keyword in rhai's tokens, maybe change to "eq" ?
                    rhai::Token::Reserved(s) if &*s == "is" => rhai::Token::EqualsTo,
                    rhai::Token::Identifier(s) if &*s == "not" => rhai::Token::NotEqualsTo,
                    // Pass through all other tokens unchanged
                    _ => token,
                }
            })
            .register_custom_syntax_raw("rule", parse_rule, true, create_rule)
            .register_custom_syntax_raw("action", parse_action, true, create_action)
            .register_custom_syntax_raw("object", parse_object, true, create_object)
            // NOTE: is their a way to defined iterators directly in modules ?
            .register_iterator::<Vec<vsmtp_common::address::Address>>()
            .register_iterator::<Vec<std::sync::Arc<Object>>>();

        Ok(engine)
    }

    /// compile the rule executor with the given script, and then checks
    /// it with a dry run.
    ///
    /// # Errors
    /// * could not compile the script.
    /// * dry run failed.
    fn compile_executor(engine: &rhai::Engine, script: &str) -> anyhow::Result<rhai::AST> {
        let mut scope = Scope::new();
        scope
            .push("date", "")
            .push("time", "")
            .push("srv", "")
            .push("ctx", "");

        let mut ast = engine
            .compile(include_str!("rule_executor.rhai"))
            .context("failed to load the rule executor")?;

        ast += engine
            .compile_with_scope(&scope, script)
            .context("failed to load script")?;

        engine
            .eval_ast_with_scope::<rhai::Map>(&mut scope, &ast)
            .with_context(|| RuleEngineError::Stage.as_str())?;

        Ok(ast)
    }

    /// create a rule engine instance from a script.
    ///
    /// # Errors
    ///
    /// * failed to compile the script.
    pub fn from_script(config: &Config, script: &str) -> anyhow::Result<Self> {
        let engine = Self::new_raw(config)?;
        let ast = Self::compile_executor(&engine, script)?;

        Ok(Self {
            context: engine,
            ast,
        })
    }
}
