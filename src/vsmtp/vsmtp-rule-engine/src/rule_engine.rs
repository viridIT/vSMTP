use crate::api::StandardVSLPackage;
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
use crate::api::rule_state::deny;
use crate::api::EngineResult;
use crate::api::SharedObject;
use crate::dsl::action::parsing::{create_action, parse_action};
use crate::dsl::delegation::parsing::{create_delegation, parse_delegation};
use crate::dsl::directives::{Directive, Directives};
use crate::dsl::object::parsing::{create_object, parse_object};
use crate::dsl::rule::parsing::{create_rule, parse_rule};
use crate::dsl::service::parsing::{create_service, parse_service};
use crate::dsl::service::Service;
use crate::rule_state::RuleState;
use anyhow::Context;
use rhai::module_resolvers::FileModuleResolver;
use rhai::packages::Package;
use rhai::{plugin::EvalAltResult, Engine, Scope, AST};
use vsmtp_common::mail_context::MailContext;
use vsmtp_common::queue::Queue;
use vsmtp_common::queue_path;
use vsmtp_common::re::{anyhow, log};
use vsmtp_common::state::StateSMTP;
use vsmtp_common::status::Status;
use vsmtp_config::Config;

/// a sharable rhai engine.
/// contains an ast representation of the user's parsed .vsl script files,
/// and modules / packages to create a cheap rhai runtime.
pub struct RuleEngine {
    /// ast built from the user's .vsl files.
    pub(super) ast: AST,
    /// rules & actions registered by the user.
    pub(super) directives: Directives,
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
        log::debug!("building vsl compiler and modules ...");

        let mut compiler = Self::new_compiler();

        let std_module = rhai::packages::StandardPackage::new().as_shared_module();
        let vsl_native_module = StandardVSLPackage::new().as_shared_module();
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

        log::debug!("compiling rhai scripts ...");

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
                "No 'main.vsl' provided in the config, the server will deny any incoming transaction by default."
            );

            compiler
                .compile(include_str!("../api/default_rules.rhai"))
                .map_err(|err| anyhow::anyhow!("failed to compile default rules: {err}"))
        }?;

        let directives = Self::extract_directives(&compiler, &ast)?;

        log::debug!("done.");

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

        let vsl_native_module = StandardVSLPackage::new().as_shared_module();
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

    // FIXME: delegation handling to refactor.
    /// runs all rules from a stage using the current transaction state.
    ///
    /// the `server_address` parameter is used to distinguish logs from each other,
    /// printing the address & port associated with this run session, not the current
    /// context. (because the context could have been pulled from the filesystem when
    /// receiving delegation results)
    /// # Panics
    pub fn run_when(&self, rule_state: &mut RuleState, smtp_state: &StateSMTP) -> Status {
        if let Some(directive_set) = self.directives.get(&smtp_state.to_string()) {
            // check if we need to skip directive execution or resume because of a delegation.
            let directive_set = match rule_state.skipped() {
                Some(Status::DelegationResult) if smtp_state.email_received() => {
                    if let Some(header) = rule_state
                        .message()
                        .read()
                        .unwrap()
                        .get_header("X-VSMTP-DELEGATION")
                    {
                        let header =
                            vsmtp_mail_parser::get_mime_header("X-VSMTP-DELEGATION", &header);

                        let (stage, directive_name, message_id) = match (
                            header.args.get("stage"),
                            header.args.get("directive"),
                            header.args.get("id"),
                        ) {
                            (Some(stage), Some(directive_name), Some(message_id)) => {
                                (stage, directive_name, message_id)
                            }
                            _ => return Status::DelegationResult,
                        };

                        if *stage == smtp_state.to_string() {
                            if let Some(d) = directive_set
                                .iter()
                                .position(|directive| directive.name() == directive_name)
                            {
                                // If delegation results are coming in and that this is the correct
                                // directive that has been delegated, we need to pull
                                // the old context because its state has been lost
                                // when the delegation happened.
                                //
                                // There is however no need to discard the old email because it
                                // will be overridden by the results once it's time to write
                                // in the 'mail' queue.
                                let path_to_context = queue_path!(
                                    &rule_state.server.config.server.queues.dirpath,
                                    Queue::Delegated,
                                    message_id
                                );

                                // FIXME: this is only useful for preq, the other processes
                                //        already fetch the old context.
                                match MailContext::from_file_path_sync(&path_to_context) {
                                    Ok(mut context) => {
                                        context.metadata.as_mut().unwrap().skipped = None;
                                        *rule_state.context().write().unwrap() = context;
                                    },
                                    Err(err) => log::error!("[{smtp_state}] tried to get old mail context '{message_id}' from the working queue after a delegation, but: {err}")
                                };

                                rule_state.resume();
                                &directive_set[d..]
                            } else {
                                return Status::DelegationResult;
                            }
                        } else {
                            return Status::DelegationResult;
                        }
                    } else {
                        return Status::DelegationResult;
                    }
                }
                Some(status) => return (*status).clone(),
                None => &directive_set[..],
            };

            match self.execute_directives(rule_state, directive_set, smtp_state) {
                Ok(status) => {
                    if status.stop() {
                        log::debug!(
                            "[{smtp_state}] the rule engine will skip all rules because of the previous result."
                        );
                        rule_state.skipping(status.clone());
                    }

                    return status;
                }
                Err(error) => {
                    log::error!("{}", Self::parse_stage_error(error, smtp_state));

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
        state: &mut RuleState,
        directives: &[Directive],
        smtp_state: &StateSMTP,
    ) -> EngineResult<Status> {
        let mut status = Status::Next;

        for directive in directives {
            status = directive.execute(state, &self.ast, smtp_state)?;

            log::debug!(
                "[{smtp_state}] {} '{}' evaluated => {status:?}.",
                directive.directive_type(),
                directive.name(),
            );

            if status != Status::Next {
                break;
            }
        }

        log::debug!("[{smtp_state}] stage evaluated => {status:?}.");

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
                    "stage '{smtp_state}' skipped => rule engine failed in '{rule}':\n\t{error}"
                )
            }
            _ => {
                format!("stage '{smtp_state}' skipped => rule engine failed:\n\t{error}",)
            }
        }
    }

    /// create a rhai engine to compile all scripts with vsl's configuration.
    #[must_use]
    pub fn new_compiler() -> rhai::Engine {
        let mut engine = Engine::new();

        // NOTE: on_parse_token is not deprecated, just subject to change in future releases.
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
            .register_custom_syntax_raw("delegate", parse_delegation, true, create_delegation)
            .register_custom_syntax_raw("object", parse_object, true, create_object)
            .register_custom_syntax_raw("service", parse_service, true, create_service)
            .register_iterator::<Vec<vsmtp_common::Address>>()
            .register_iterator::<Vec<SharedObject>>();

        engine
    }

    /// compile vsl's api into a module.
    ///
    /// # Errors
    /// * Failed to compile the API.
    /// * Failed to create a module from the API.
    pub fn compile_api(engine: &rhai::Engine) -> anyhow::Result<rhai::Module> {
        let ast = engine
            .compile_scripts_with_scope(
                &rhai::Scope::new(),
                [
                    include_str!("../api/codes.rhai"),
                    include_str!("../api/networks.rhai"),
                    include_str!("../api/auth.rhai"),
                    include_str!("../api/utils.rhai"),
                    include_str!("../api/sys-api.rhai"),
                    include_str!("../api/rhai-api.rhai"),
                ],
            )
            .context("failed to compile vsl's api")?;

        rhai::Module::eval_ast_as_new(rhai::Scope::new(), &ast, engine)
            .context("failed to create a module from vsl's api.")
    }

    // FIXME: could be easily refactored.
    //        every `ok_or_else` could be replaced by an unwrap here.
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
            let stage = match StateSMTP::try_from(stage.as_str()) {
                Ok(stage) => stage,
                Err(_) => anyhow::bail!("the '{stage}' smtp stage does not exist."),
            };

            let directive_set = directive_set
                .try_cast::<rhai::Array>()
                .ok_or_else(|| {
                    anyhow::anyhow!("the stage '{stage}' must be declared using the array syntax")
                })?
                .into_iter()
                .map(|rule| {
                    let map = rule.try_cast::<rhai::Map>().unwrap();
                    let directive_type = map
                        .get("type")
                        .ok_or_else(|| anyhow::anyhow!("a directive in stage '{stage}' does not have a valid type"))?
                        .to_string();

                        let name = map
                        .get("name")
                        .ok_or_else(|| anyhow::anyhow!("a directive in stage '{stage}' does not have a name"))?
                        .to_string();

                    let pointer = map
                        .get("evaluate")
                        .ok_or_else(|| anyhow::anyhow!("the directive '{stage}' in stage '{name}' does not have an evaluation function"))?
                        .clone()
                        .try_cast::<rhai::FnPtr>()
                        .ok_or_else(|| anyhow::anyhow!("the evaluation field for the directive '{stage}' in stage '{name}' must be a function pointer"))?;

                    let directive =
                        match directive_type.as_str() {
                            "rule" => Directive::Rule { name, pointer },
                            "action" => Directive::Action { name, pointer },
                            "delegate" => {

                                if !stage.email_received() {
                                    anyhow::bail!("invalid delegation '{name}' in stage '{stage}': delegation directives are available from the 'postq' stage and onwards.");
                                }

                                let service = map
                                    .get("service")
                                    .ok_or_else(|| anyhow::anyhow!("the delegation '{name}' in stage '{stage}' does not have a service to delegate processing to"))?
                                    .clone()
                                    .try_cast::<std::sync::Arc<Service>>()
                                    .ok_or_else(|| anyhow::anyhow!("the field after the 'delegate' keyword in the directive '{name}' in stage '{stage}' must be a smtp service"))?;

                                Directive::Delegation { name, pointer, service }
                            },
                            unknown => anyhow::bail!("unknown directive type '{unknown}' called '{name}'"),
                        };

                    Ok(directive)
                })
                .collect::<anyhow::Result<Vec<_>>>()?;

            directives.insert(stage.to_string(), directive_set);
        }

        let names = directives
            .iter()
            .flat_map(|(_, d)| d)
            .map(crate::dsl::directives::Directive::name)
            .collect::<Vec<_>>();

        // TODO: refactor next loop with templated function 'find_duplicate'.
        for (idx, name) in names.iter().enumerate() {
            for other in &names[idx + 1..] {
                if other == name {
                    anyhow::bail!("found duplicate rule '{name}': a rule must have a unique name",);
                }
            }
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
