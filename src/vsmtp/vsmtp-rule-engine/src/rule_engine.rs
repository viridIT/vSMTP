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
use anyhow::Context;
use rhai::module_resolvers::FileModuleResolver;
use rhai::packages::Package;
use rhai::{plugin::EvalAltResult, Engine, Scope, AST};
use vsmtp_common::re::{anyhow, log};
use vsmtp_common::state::StateSMTP;
use vsmtp_common::status::Status;
use vsmtp_config::Config;

use crate::dsl::action::parsing::{create_action, parse_action};
use crate::dsl::directives::{Action, Directive, Directives, Rule};
use crate::dsl::object::parsing::{create_object, parse_object};
use crate::dsl::object::Object;
use crate::dsl::rule::parsing::{create_rule, parse_rule};
use crate::dsl::service::parsing::{create_service, parse_service};
use crate::modules::actions::rule_state::rule_state::deny;
use crate::modules::EngineResult;
use crate::rule_state::RuleState;
use crate::{log_channels, modules};

/// a sharable rhai engine.
/// contains an ast representation of the user's parsed .vsl script files,
/// and modules / packages to create a cheap rhai runtime.
pub struct RuleEngine {
    /// ast built from the user's .vsl files.
    pub(super) ast: AST,
    /// rules & actions registered by the user.
    directives: Directives,
    /// vsl's standard rust api.
    pub(super) vsl_native_module: rhai::Shared<rhai::Module>,
    /// vsl's standard rhai api.
    pub(super) vsl_rhai_module: rhai::Shared<rhai::Module>,
    /// rhai's standard api.
    pub(super) std_module: rhai::Shared<rhai::Module>,
    /// a translation of the toml configuration as a rhai Map.
    pub(super) toml_module: rhai::Shared<rhai::Module>,
}

impl RuleEngine {
    /// creates a new instance of the rule engine, reading all files in the
    /// `script_path` parameter.
    /// if `script_path` is `None`, an warning is emitted and a deny-all script
    /// is loaded.
    ///
    /// # Errors
    /// * failed to register `script_path` as a valid module folder.
    /// * failed to compile or load any script located at `script_path`.
    pub fn new(config: &Config, script_path: &Option<std::path::PathBuf>) -> anyhow::Result<Self> {
        log::debug!(
            target: log_channels::RE,
            "building vsl compiler and modules ..."
        );

        let mut compiler = Self::new_compiler();

        let std_module = rhai::packages::StandardPackage::new().as_shared_module();
        let vsl_native_module = modules::StandardVSLPackage::new().as_shared_module();
        let toml_module = rhai::Shared::new(Self::build_toml_module(config, &compiler)?);

        compiler
            .set_module_resolver(match script_path {
                Some(script_path) => FileModuleResolver::new_with_path_and_extension(
                    script_path.parent().ok_or_else(|| {
                        anyhow::anyhow!(
                            "file '{}' does not have a valid parent directory for rules",
                            script_path.display()
                        )
                    })?,
                    "vsl",
                ),
                None => FileModuleResolver::new_with_extension("vsl"),
            })
            .register_global_module(std_module.clone())
            .register_static_module("sys", vsl_native_module.clone())
            .register_static_module("toml", toml_module.clone());

        log::debug!(target: log_channels::RE, "compiling rhai scripts ...");

        let vsl_rhai_module =
            rhai::Shared::new(Self::compile_api(&compiler).context("failed to compile vsl's api")?);
        compiler.register_global_module(vsl_rhai_module.clone());

        let ast = if let Some(script_path) = &script_path {
            compiler
                .compile_into_self_contained(
                    &rhai::Scope::new(),
                    &std::fs::read_to_string(&script_path)
                        .context(format!("failed to read file: '{}'", script_path.display()))?,
                )
                .map_err(|err| anyhow::anyhow!("failed to compile your scripts: {err}"))
        } else {
            log::warn!(
                target: log_channels::RE,
                "No 'main.vsl' provided in the config, the server will deny any incoming transaction by default.",
            );

            compiler
                .compile(include_str!("default_rules.rhai"))
                .map_err(|err| anyhow::anyhow!("failed to compile default rules: {err}"))
        }?;

        let directives = Self::extract_directives(&compiler, &ast)?;

        log::debug!(target: log_channels::RE, "done.");

        Ok(Self {
            ast,
            directives,
            vsl_native_module,
            vsl_rhai_module,
            std_module,
            toml_module,
        })
    }

    /// create a rule engine instance from a script.
    ///
    /// # Errors
    ///
    /// * failed to compile the script.
    pub fn from_script(config: &Config, script: &str) -> anyhow::Result<Self> {
        let mut compiler = Self::new_compiler();

        let vsl_native_module = modules::StandardVSLPackage::new().as_shared_module();
        let std_module = rhai::packages::StandardPackage::new().as_shared_module();
        let toml_module = rhai::Shared::new(Self::build_toml_module(config, &compiler)?);

        compiler
            .register_global_module(std_module.clone())
            .register_static_module("sys", vsl_native_module.clone())
            .register_static_module("toml", toml_module.clone());

        let vsl_rhai_module =
            rhai::Shared::new(Self::compile_api(&compiler).context("failed to compile vsl's api")?);

        compiler.register_global_module(vsl_rhai_module.clone());

        let ast = compiler.compile_into_self_contained(&rhai::Scope::new(), script)?;

        let directives = Self::extract_directives(&compiler, &ast)?;

        Ok(Self {
            ast,
            directives,
            vsl_native_module,
            vsl_rhai_module,
            std_module,
            toml_module,
        })
    }

    /// runs all rules from a stage using the current transaction state.$
    /// # Panics
    pub fn run_when(&self, rule_state: &mut RuleState, smtp_state: &StateSMTP) -> Status {
        if let Some(status) = rule_state.skipped() {
            return (*status).clone();
        }

        if let Some(directive_set) = self.directives.get(&smtp_state.to_string()) {
            match self.execute_directives(rule_state.engine(), &directive_set[..], smtp_state) {
                Ok(status) => {
                    if let Status::Faccept | Status::Deny(_) | Status::Quarantine(_) = status {
                        log::debug!(
                        target: log_channels::RE,
                        "[{}] the rule engine will skip all rules because of the previous result.",
                        smtp_state
                    );
                        rule_state.skipping(status.clone());
                    }

                    return status;
                }
                Err(error) => {
                    log::error!(
                        target: log_channels::RE,
                        "{}",
                        Self::parse_stage_error(error, smtp_state)
                    );

                    // if an error occurs, the engine denies the connection by default.
                    let state_if_error = deny();
                    rule_state.skipping(state_if_error.clone());
                    return state_if_error;
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
        }
    }

    /// create a rhai engine to compile all scripts with vsl's configuration.
    fn new_compiler() -> rhai::Engine {
        let mut engine = Engine::new();

        // NOTE: on_parse_token is not deprecated, just subject to change in futur releases.
        #[allow(deprecated)]
        engine
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
            .register_custom_syntax_raw("service", parse_service, true, create_service)
            .register_iterator::<Vec<vsmtp_common::Address>>()
            .register_iterator::<Vec<std::sync::Arc<Object>>>();

        engine
    }

    fn compile_api(engine: &rhai::Engine) -> anyhow::Result<rhai::Module> {
        let ast = engine
            .compile_scripts_with_scope(
                &rhai::Scope::new(),
                [
                    include_str!("api/codes.rhai"),
                    include_str!("api/networks.rhai"),
                    include_str!("api/auth.rhai"),
                    include_str!("api/sys-api.rhai"),
                    include_str!("api/rhai-api.rhai"),
                    include_str!("api/utils.rhai"),
                ],
            )
            .context("failed to compile vsl's api")?;

        rhai::Module::eval_ast_as_new(rhai::Scope::new(), &ast, engine)
            .context("failed to create a module from vsl's api.")
    }

    /// extract rules & actions from the main vsl script.
    fn extract_directives(engine: &rhai::Engine, ast: &rhai::AST) -> anyhow::Result<Directives> {
        let mut scope = Scope::new();
        scope
            .push("date", ())
            .push("time", ())
            .push_constant("CTX", ())
            .push_constant("SRV", ());

        let raw_directives = engine
            .eval_ast_with_scope::<rhai::Map>(&mut scope, ast)
            .context("failed to compile your rules.")?;

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

    fn build_toml_module(config: &Config, engine: &rhai::Engine) -> anyhow::Result<rhai::Module> {
        let server_config = &vsmtp_common::re::serde_json::to_string(&config.server)
            .context("failed to convert the server configuration to json")?;

        let app_config = &vsmtp_common::re::serde_json::to_string(&config.app)
            .context("failed to convert the app configuration to json")?;

        let mut toml_module = rhai::Module::new();

        toml_module
            .set_var("server", engine.parse_json(server_config, true)?)
            .set_var("app", engine.parse_json(app_config, true)?);

        Ok(toml_module)
    }
}
