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
use rhai::packages::Package;
use rhai::{plugin::EvalAltResult, Engine, Scope, AST};
use vsmtp_common::envelop::Envelop;
use vsmtp_common::mail_context::{Body, ConnectionContext, MailContext};
use vsmtp_common::re::{anyhow, log};
use vsmtp_common::state::StateSMTP;
use vsmtp_common::status::Status;
use vsmtp_config::Config;

use crate::dsl::action_parsing::{create_action, parse_action};
use crate::dsl::directives::{Action, Directive, Directives, Rule};
use crate::dsl::object_parsing::{create_object, parse_object};
use crate::dsl::rule_parsing::{create_rule, parse_rule};
use crate::modules::EngineResult;
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

        scope.push("date", "").push("time", "");

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

        scope.push("date", "").push("time", "");

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
/// contains an ast representation of the user's parsed .vsl script files.
pub struct RuleEngine {
    /// ast built from the user's .vsl files.
    ast: AST,
    /// rules & actions registered by the user.
    directives: Directives,
    /// vsl's standard api.
    vsl_package: modules::StandardVSLPackage,
    /// rhai's standard api.
    std_package: rhai::packages::StandardPackage,
}

impl RuleEngine {
    /// runs all rules from a stage using the current transaction state.$
    /// # Panics
    pub fn run_when(&self, rule_state: &mut RuleState, smtp_state: &StateSMTP) -> Status {
        // FIXME: the raw engine could be part of the state instead of building one every run.
        // creating a raw engine every run which is extremely cheap to create.
        let mut engine = rhai::Engine::new_raw();

        let mail_context = rule_state.mail_context.clone();
        let server = rule_state.server.clone();

        engine
            // NOTE: why do we have to clone the arc twice instead of just moving it here ?
            // injecting the state if the current connection into the engine.
            .on_var(move |name, _, _| match name {
                "CTX" => Ok(Some(rhai::Dynamic::from(mail_context.clone()))),
                "SRV" => Ok(Some(rhai::Dynamic::from(server.clone()))),
                _ => Ok(None),
            })
            .register_global_module(self.std_package.as_shared_module())
            .register_static_module("sys", self.vsl_package.as_shared_module());

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

        if let Some(directive_set) = self.directives.get(&smtp_state.to_string()) {
            match self.execute_directives(&engine, &directive_set[..], smtp_state) {
                Ok(status) => {
                    if let Status::Faccept | Status::Deny(_) = status {
                        log::debug!(
                        target: log_channels::RE,
                        "[{}] the rule engine will skip all rules because of the previous result.",
                        smtp_state
                    );
                        rule_state.skip = Some(status.clone());
                    }

                    return status;
                }
                Err(error) => {
                    log::error!(
                        target: log_channels::RE,
                        "{}",
                        Self::parse_stage_error(error, smtp_state)
                    );
                    return Status::Next;
                }
            }
        }

        Status::Next
    }

    fn execute_directives(
        &self,
        engine: &rhai::Engine,
        directives: &[Box<dyn Directive + Send + Sync>],
        smtp_state: &StateSMTP,
    ) -> EngineResult<Status> {
        let mut status = Status::Next;

        for directive in directives {
            status = directive.execute(engine, &self.ast)?;

            log::debug!(
                target: log_channels::RE,
                "[{}] {} '{}' evaluated => {:?}.",
                smtp_state,
                directive.directive_type(),
                directive.name(),
                status
            );

            if status != Status::Next {
                break;
            }
        }

        log::debug!(
            target: log_channels::RE,
            "[{}] evaluated => {:?}.",
            smtp_state,
            status
        );

        Ok(status)
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

        let std_package = rhai::packages::StandardPackage::new();
        let vsl_package = modules::StandardVSLPackage::new();

        engine
            .set_module_resolver(match script_path {
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
            })
            .register_static_module("sys", vsl_package.as_shared_module())
            .register_global_module(std_package.as_shared_module());

        log::debug!(target: log_channels::RE, "compiling rhai scripts ...");

        let mut ast = Self::compile_api(&mut engine).context("failed to compile vsl's api")?;

        ast += if let Some(script_path) = &script_path {
            engine
            .compile(
                &std::fs::read_to_string(&script_path)
                .context(format!("failed to read file: '{}'", script_path.display()))?
            )
        } else {
            log::warn!(
                target: log_channels::RE,
                "No 'main.vsl' provided in the config, the server will deny any incoming transaction by default.",
            );

            engine
            .compile(include_str!("default_rules.rhai"))
        }.context("failed to compile your scripts")?;

        let directives = Self::extract_directives(&engine, &ast)?;

        log::debug!(target: log_channels::RE, "done.");

        Ok(Self {
            ast,
            directives,
            vsl_package,
            std_package,
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

        let mut toml_module = rhai::Module::new();

        // setting up toml configuration injection.
        toml_module
            .set_var("server", engine.parse_json(server_config, true)?)
            .set_var("app", engine.parse_json(app_config, true)?);

        engine
            .register_static_module("toml", toml_module.into())
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
            // TODO: yes, use a package.
            .register_iterator::<Vec<vsmtp_common::address::Address>>()
            .register_iterator::<Vec<std::sync::Arc<Object>>>();

        Ok(engine)
    }

    fn compile_api(engine: &mut rhai::Engine) -> anyhow::Result<rhai::AST> {
        let ast = engine
            .compile_scripts_with_scope(
                &rhai::Scope::new(),
                [
                    include_str!("api/sys-api.rhai"),
                    include_str!("api/rhai-api.rhai"),
                    include_str!("api/utils.rhai"),
                ],
            )
            .context("failed to compile vsl's api")?;
        Ok(ast)
    }

    /// extract rules & actions from the main vsl script.
    fn extract_directives(engine: &rhai::Engine, ast: &rhai::AST) -> anyhow::Result<Directives> {
        let mut scope = Scope::new();
        scope
            .push("date", "")
            .push("time", "")
            .push_constant("CTX", "")
            .push_constant("SRV", "");

        let raw_directives = engine
            .eval_ast_with_scope::<rhai::Map>(&mut scope, ast)
            .context("failed to evaluate your rules")?;

        let mut directives = Directives::new();

        for (stage, directive_set) in raw_directives {
            let directive_set = directive_set
                .try_cast::<rhai::Array>()
                .ok_or_else(|| {
                    anyhow::anyhow!("the stage {} must be declared with an array", stage)
                })?
                .into_iter()
                .map(|rule| {
                    let map = rule.try_cast::<rhai::Map>().unwrap();
                    let directive_type = map
                        .get("type")
                        .ok_or_else(|| anyhow::anyhow!("a directive in stage {} does not have a valid type", stage))?
                        .to_string();
                    let name = map
                        .get("name")
                        .ok_or_else(|| anyhow::anyhow!("a directive in stage {} does not have a name", stage))?
                        .to_string();
                    let pointer = map
                        .get("evaluate")
                        .ok_or_else(|| anyhow::anyhow!("the directive {} in stage {} does not have an evaluation function", stage, name))?.clone().try_cast::<rhai::FnPtr>().ok_or_else(|| anyhow::anyhow!("the directive {} in stage {} evaluation field must be a function pointer", stage, name))?;

                    let directive: Box<dyn Directive + Send + Sync> =
                        match directive_type.as_str() {
                            "rule" => Box::new(Rule { name, pointer }),
                            "action" => Box::new(Action { name, pointer}),
                            unknown => anyhow::bail!("unknown directive '{}'", unknown),
                        };

                    Ok(directive)
                })
                .collect::<anyhow::Result<Vec<Box<_>>>>()?;

            directives.insert(stage.to_string(), directive_set);
        }

        Ok(directives)
    }

    /// create a rule engine instance from a script.
    ///
    /// # Errors
    ///
    /// * failed to compile the script.
    pub fn from_script(config: &Config, script: &str) -> anyhow::Result<Self> {
        let mut engine = Self::new_raw(config)?;
        let mut ast = Self::compile_api(&mut engine).context("failed to compile vsl's api")?;
        ast += engine.compile(script)?;
        let directives = Self::extract_directives(&engine, &ast)?;
        let vsl_package = modules::StandardVSLPackage::new();
        let std_package = rhai::packages::StandardPackage::new();

        engine
            .register_static_module("sys", vsl_package.as_shared_module())
            .register_global_module(std_package.as_shared_module());

        Ok(Self {
            ast,
            directives,
            vsl_package,
            std_package,
        })
    }
}
