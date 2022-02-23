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
use crate::config::log_channel::RULES;
use crate::config::server_config::Service;
use crate::rules::error::RuleEngineError;
use crate::rules::obj::Object;
use crate::smtp::envelop::Envelop;
use crate::smtp::mail::{Body, MailContext};

use anyhow::Context;
use rhai::module_resolvers::FileModuleResolver;
use rhai::{
    exported_module, plugin::*, Array, Engine, LexError, Map, ParseError, ParseErrorType, Scope,
    AST,
};

use std::net::Ipv4Addr;
use std::net::{IpAddr, SocketAddr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Status {
    /// accepts the current stage value, skips all rules in the stage.
    Accept,

    /// continue to the next rule / stage.
    Next,

    /// immediately stops the transaction and send an error code.
    Deny,

    /// ignore all future rules for the current transaction.
    Faccept,
}

impl Status {
    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Accept => "accept",
            Status::Next => "next",
            Status::Deny => "deny",
            Status::Faccept => "faccept",
        }
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub struct RuleState<'a> {
    scope: Scope<'a>,
    ctx: std::sync::Arc<std::sync::RwLock<MailContext>>,
    skip: Option<Status>,
}

impl<'a> RuleState<'a> {
    /// creates a new rule engine with an empty scope.
    pub(crate) fn new(config: &crate::config::server_config::ServerConfig) -> Self {
        let mut scope = Scope::new();
        let ctx = std::sync::Arc::new(std::sync::RwLock::new(MailContext {
            client_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
            envelop: Envelop::default(),
            body: Body::Empty,
            metadata: None,
        }));

        scope
            // stage specific variables.
            .push("ctx", ctx.clone())
            // data available in every stage.
            .push("date", "")
            .push("time", "")
            .push("connection_timestamp", std::time::SystemTime::now())
            // configuration variables.
            .push("addr", config.server.addr)
            .push("logs_file", config.log.file.clone())
            .push("spool_dir", config.delivery.spool_dir.clone())
            .push(
                "services",
                std::sync::Arc::new(config.rules.services.clone()),
            );

        Self {
            scope,
            ctx,
            skip: None,
        }
    }

    pub(crate) fn with_context(
        config: &crate::config::server_config::ServerConfig,
        ctx: MailContext,
    ) -> Self {
        let mut scope = Scope::new();
        let ctx = std::sync::Arc::new(std::sync::RwLock::new(ctx));

        scope
            // stage specific variables.
            .push("ctx", ctx.clone())
            // data available in every stage.
            .push("date", "")
            .push("time", "")
            .push("connection_timestamp", std::time::SystemTime::now())
            // configuration variables.
            .push("addr", config.server.addr)
            .push("logs_file", config.log.file.clone())
            .push("spool_dir", config.delivery.spool_dir.clone())
            .push(
                "services",
                std::sync::Arc::new(config.rules.services.clone()),
            );

        Self {
            scope,
            ctx,
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
    pub(crate) fn get_context(&mut self) -> std::sync::Arc<std::sync::RwLock<MailContext>> {
        self.ctx.clone()
    }

    pub fn skipped(&self) -> Option<Status> {
        self.skip
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
    pub(crate) fn run_when(&self, state: &mut RuleState, stage: &str) -> Status {
        if let Some(status) = state.skip {
            return status;
        }

        let now = chrono::Local::now();
        state
            .scope
            .set_value("date", now.date().format("%Y/%m/%d").to_string())
            .set_value("time", now.time().format("%H:%M:%S").to_string());

        let rules = match self
            .context
            .eval_ast_with_scope::<rhai::Map>(&mut state.scope, &self.ast)
        {
            Ok(rules) => rules,
            Err(error) => {
                log::error!(
                    target: RULES,
                    "stage '{}' skipped => rule engine failed to evaluate rules:\n\t{}",
                    stage,
                    error
                );
                return Status::Next;
            }
        };

        match self.context.call_fn(
            &mut state.scope,
            &self.ast,
            "run_rules",
            (rules, stage.to_string()),
        ) {
            Ok(status) => {
                log::debug!(target: RULES, "[{}] evaluated => {:?}.", stage, status);

                match status {
                    Status::Faccept | Status::Deny => {
                        log::debug!(
                        target: RULES,
                        "[{}] the rule engine will skip all rules because of the previous result.",
                        stage
                    );
                        state.skip = Some(status);
                        status
                    }
                    s => s,
                }
            }
            Err(error) => {
                log::error!(target: RULES, "{}", self.parse_stage_error(error, stage));
                Status::Next
            }
        }
    }

    fn parse_stage_error(&self, error: Box<EvalAltResult>, stage: &str) -> String {
        match *error {
            // NOTE: since all errors are caught and thrown in "run_rules", errors
            //       are always wrapped in ErrorInFunctionCall.
            EvalAltResult::ErrorInFunctionCall(_, _, mut error, _) => match *error {
                EvalAltResult::ErrorRuntime(error, _) if error.is::<rhai::Map>() => {
                    let error = error.cast::<rhai::Map>();
                    let rule = error
                        .get("rule")
                        .map(|d| d.to_string())
                        .unwrap_or_else(|| "unknown rule".to_string());
                    let error = error
                        .get("message")
                        .map(|d| d.to_string())
                        .unwrap_or_else(|| "vsl internal unexpected error".to_string());

                    format!(
                        "stage '{}' skipped => rule engine failed in '{}':\n\t{}",
                        stage, rule, error
                    )
                }
                _ => {
                    error.take_position();
                    format!(
                        "stage '{}' skipped => rule engine failed:\n\t{}",
                        stage, error,
                    )
                }
            },
            _ => {
                format!(
                    "rule engine unexpected error in stage '{}':\n\t{:?}",
                    stage, error
                )
            }
        }
    }

    /// creates a new instance of the rule engine, reading all files in
    /// src_path parameter.
    pub fn new<S>(script_path: S) -> anyhow::Result<Self>
    where
        S: AsRef<str>,
    {
        let mut engine = Engine::new();

        let mut module: Module = exported_module!(crate::rules::modules::actions::actions);
        module
            .combine(exported_module!(crate::rules::modules::types::types))
            .combine(exported_module!(crate::rules::modules::email::email));

        engine
            .set_module_resolver(FileModuleResolver::new_with_path_and_extension(
                script_path.as_ref(),
                "vsl",
            ))
            .register_static_module("vsl", module.into())
            .disable_symbol("eval")

            .on_parse_token(|token, _, _| {
                match token {
                    // remap 'is' operator to '==', it's easier than creating a new operator.
                    // NOTE: warning => "is" is a reserved keyword in rhai's tokens, maybe change to "eq" ?
                    rhai::Token::Reserved(s) if &*s == "is" => rhai::Token::EqualsTo,
                    rhai::Token::Identifier(s) if &*s == "not" => rhai::Token::NotEqualsTo,
                    // Pass through all other tokens unchanged
                    _ => token
                }
            })
            // `rule $name$ #{expr}` syntax.
            .register_custom_syntax_raw(
                "rule",
                |symbols, look_ahead| match symbols.len() {
                    // rule keyword ...
                    1 => Ok(Some("$string$".into())),
                    // name of the rule ...
                    2 => Ok(Some("$expr$".into())),
                    // map, we are done parsing.
                    3 => Ok(None),
                    _ => Err(ParseError(
                        Box::new(ParseErrorType::BadInput(LexError::UnexpectedInput(
                            format!(
                                "Improper rule declaration: keyword '{}' unknown.",
                                look_ahead
                            ),
                        ))),
                        Position::NONE,
                    )),
                },
                true,
                move |context, input| {
                    let name = input[0].get_literal_value::<ImmutableString>().unwrap();
                    let expr = context.eval_expression_tree(&input[1])?;

                    Ok(Dynamic::from(Map::from_iter(
                        [
                            ("name".into(), Dynamic::from(name.clone())),
                            ("type".into(), "rule".into()),
                        ]
                        .into_iter()
                        .chain(if expr.is::<Map>() {
                            let properties = expr.cast::<Map>();

                            if properties
                                .get("evaluate")
                                .filter(|f| f.is::<rhai::FnPtr>())
                                .is_none()
                            {
                                return Err(format!(
                                    "'evaluate' function is missing from '{}' rule",
                                    name
                                )
                                .into());
                            }

                            properties.into_iter()
                        } else if expr.is::<rhai::FnPtr>() {
                            Map::from_iter([
                                ("evaluate".into(), expr),
                            ])
                            .into_iter()
                        } else {
                            return Err(format!(
                                "a rule must be a map (#{{}}) or an anonymous function (|| {{}}){}",
                                RuleEngineError::Rule.as_str()
                            )
                            .into());
                        }),
                    )))
                },
            )
            // `action $name$ #{expr}` syntax.
            .register_custom_syntax_raw(
                "action",
                |symbols, look_ahead| match symbols.len() {
                    // action keyword ...
                    1 => Ok(Some("$string$".into())),
                    // name of the action ...
                    2 => Ok(Some("$expr$".into())),
                    // block, we are done parsing.
                    3 => Ok(None),
                    _ => Err(ParseError(
                        Box::new(ParseErrorType::BadInput(LexError::UnexpectedInput(
                            format!(
                                "Improper action declaration: keyword '{}' unknown.",
                                look_ahead
                            ),
                        ))),
                        Position::NONE,
                    )),
                },
                true,
                move |context, input| {
                    let name = input[0].get_literal_value::<ImmutableString>().unwrap();
                    let expr = context.eval_expression_tree(&input[1])?;

                    Ok(Dynamic::from(Map::from_iter(
                        [
                            ("name".into(), Dynamic::from(name.clone())),
                            ("type".into(), "action".into()),
                        ]
                        .into_iter()
                        .chain(if expr.is::<Map>() {
                            let properties = expr.cast::<Map>();

                            if properties
                                .get("evaluate")
                                .filter(|f| f.is::<rhai::FnPtr>())
                                .is_none()
                            {
                                return Err(format!(
                                    "'evaluate' function is missing from '{}' action",
                                    name
                                )
                                .into());
                            }

                            properties.into_iter()
                        } else if expr.is::<rhai::FnPtr>() {
                            Map::from_iter([
                                ("evaluate".into(), expr),
                            ])
                            .into_iter()
                        } else {
                            return Err(format!(
                                "an action must be a map (#{{}}) or an anonymous function (|| {{}}){}",
                                RuleEngineError::Action.as_str()
                            )
                            .into());
                        }),
                    )))
                 },
            )
            // `obj $type[:file_type]$ $name$ #{}` container syntax.
            .register_custom_syntax_raw(
                "object",
                |symbols, look_ahead| match symbols.len() {
                    // obj ...
                    1 => Ok(Some("$ident$".into())),
                    // the type of the object ...
                    2 => match symbols[1].as_str() {
                        "ip4" | "ip6" | "rg4" | "rg6" | "fqdn" | "address" | "ident" | "string" | "regex"
                        | "group" => Ok(Some("$string$".into())),
                        "file" => Ok(Some("$symbol$".into())),
                        entry => Err(ParseError(
                            Box::new(ParseErrorType::BadInput(LexError::ImproperSymbol(
                                entry.into(),
                                format!("Improper object type. '{}'.", entry),
                            ))),
                            Position::NONE,
                        )),
                    },
                    // name of the object or ':' symbol for files ...
                    3 => match symbols[2].as_str() {
                        ":" => Ok(Some("$ident$".into())),
                        _ => Ok(Some("$expr$".into())),
                    },
                    // file content type or info block / value of object, we are done parsing.
                    4 => match symbols[3].as_str() {
                        // NOTE: could it be possible to add a "file" content type ?
                        "ip4" | "ip6" | "rg4" | "rg6" | "fqdn" | "address" | "ident" | "string" | "regex" => {
                            Ok(Some("$string$".into()))
                        }
                        _ => Ok(None),
                    },
                    // object name for a file.
                    5 => Ok(Some("$expr$".into())),
                    // done parsing file expression.
                    6 => Ok(None),
                    _ => Err(ParseError(
                        Box::new(ParseErrorType::BadInput(LexError::UnexpectedInput(
                            format!(
                                "Improper object declaration: keyword '{}' unknown.",
                                look_ahead
                            ),
                        ))),
                        Position::NONE,
                    )),
                },
                true,
                move |context, input| {
                    let var_type = input[0].get_string_value().unwrap().to_string();
                    let var_name: String;

                    // FIXME: refactor this expression.
                    // file type as a special syntax (file:type),
                    // so we need a different method to parse it.
                    let object = match var_type.as_str() {
                        "file" => {
                            let content_type = input[2].get_string_value().unwrap();
                            var_name = input[3]
                                .get_literal_value::<ImmutableString>()
                                .unwrap()
                                .to_string();
                            let object = context.eval_expression_tree(&input[4])?;

                            // the object syntax can use a map or an inline string.
                            if object.is::<Map>() {
                                let mut object: Map =
                                    object.try_cast().ok_or(RuleEngineError::Object)?;
                                object.insert("type".into(), Dynamic::from(var_type.clone()));
                                object.insert("name".into(), Dynamic::from(var_name.clone()));
                                object.insert(
                                    "content_type".into(),
                                    Dynamic::from(content_type.to_string()),
                                );
                                object
                            } else if object.is::<String>() {
                                let mut map = Map::new();
                                map.insert("type".into(), Dynamic::from(var_type.clone()));
                                map.insert("name".into(), Dynamic::from(var_name.clone()));
                                map.insert(
                                    "content_type".into(),
                                    Dynamic::from(content_type.to_string()),
                                );
                                map.insert("value".into(), object);
                                map
                            } else {
                                return Err(EvalAltResult::ErrorMismatchDataType(
                                    "Map | String".to_string(),
                                    object.type_name().to_string(),
                                    Position::NONE,
                                )
                                .into());
                            }
                        }

                        // generic type, we can parse it easily.
                        _ => {
                            var_name = input[1]
                                .get_literal_value::<ImmutableString>()
                                .unwrap()
                                .to_string();
                            let object = context.eval_expression_tree(&input[2])?;

                            if object.is::<Map>() {
                                let mut object: Map =
                                    object.try_cast().ok_or(RuleEngineError::Object)?;
                                object.insert("type".into(), Dynamic::from(var_type.clone()));
                                object.insert("name".into(), Dynamic::from(var_name.clone()));
                                object
                            } else if object.is::<String>() || object.is::<Array>() {
                                let mut map = Map::new();
                                map.insert("type".into(), Dynamic::from(var_type.clone()));
                                map.insert("name".into(), Dynamic::from(var_name.clone()));
                                map.insert("value".into(), object);
                                map
                            } else {
                                return Err(EvalAltResult::ErrorMismatchDataType(
                                    "Map | String".to_string(),
                                    object.type_name().to_string(),
                                    Position::NONE,
                                )
                                .into());
                            }
                        }
                    };

                    let obj_ptr = std::sync::Arc::new(
                        Object::from(&object)
                            .map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())?,
                    );

                    // pushing object in scope, preventing a "let _" statement,
                    // and returning a reference to the object in case of a parent group.
                    context.scope_mut().push(var_name, obj_ptr.clone());

                    Ok(Dynamic::from(obj_ptr))
                },
            )
            // NOTE: is their a way to defined iterators directly in modules ?
            .register_iterator::<crate::rules::modules::types::Rcpt>()
            .register_iterator::<Vec<std::sync::Arc<Object>>>();

        log::debug!(target: RULES, "compiling rhai scripts ...");

        let main_path = std::path::PathBuf::from_iter([script_path.as_ref(), "main.vsl"]);

        let mut scope = Scope::new();
        scope
            // stage specific variables.
            .push(
                "ctx",
                std::sync::Arc::new(std::sync::RwLock::new(MailContext {
                    client_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
                    envelop: Envelop::default(),
                    body: Body::Empty,
                    metadata: None,
                })),
            )
            // data available in every stage.
            .push("date", "")
            .push("time", "")
            .push("connection_timestamp", std::time::SystemTime::now())
            // configuration variables.
            .push(
                "addr",
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
            )
            .push("logs_file", "")
            .push("spool_dir", "")
            .push("services", std::sync::Arc::new(Vec::<Service>::new()));

        let mut ast = engine
            // FIXME: use include_str in production.
            .compile(include_str!("rule_executor.rhai"))
            .context("failed to load the rule executor")?;

        // compiling main script.
        ast += engine
            .compile_with_scope(
                &scope,
                std::fs::read_to_string(&main_path).unwrap_or_else(|err| {
                    log::warn!(
                        target: RULES,
                        "No main.vsl file found at '{:?}', no rules will be processed. {}",
                        main_path,
                        err
                    );
                    "#{}".to_string()
                }),
            )
            .context("failed to compile main.vsl")?;

        engine
            .eval_ast_with_scope::<rhai::Map>(&mut scope, &ast)
            .context("failed to parse rules")?;

        log::debug!(target: RULES, "done.");

        Ok(Self {
            context: engine,
            ast,
        })
    }
}
