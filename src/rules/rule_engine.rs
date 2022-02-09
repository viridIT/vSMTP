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
use crate::config::server_config::ServerConfig;
use crate::queue::Queue;
use crate::rules::obj::Object;
use crate::rules::operation_queue::{Operation, OperationQueue};
use crate::smtp::envelop::Envelop;
use crate::smtp::mail::{Body, MailContext, MessageMetadata, MAIL_CAPACITY};

use anyhow::Context;
use rhai::module_resolvers::FileModuleResolver;
use rhai::{
    exported_module, plugin::*, Array, Engine, LexError, Map, ParseError, ParseErrorType, Scope,
    AST,
};

use std::net::{IpAddr, SocketAddr};
use std::{
    net::Ipv4Addr,
    str::FromStr,
    sync::{Arc, RwLock},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Status {
    /// accepts the current stage value, skips all rules in the stage.
    Accept,

    /// continue to the next rule / stage.
    Continue,

    /// immediately stops the transaction and send an error code.
    Deny,

    /// ignore all future rules for the current transaction.
    Faccept,

    /// wait for the email before stopping the transaction and sending an error code,
    /// skips all future rules to fill the envelop and mail data as fast as possible.
    /// also stores the email data in an user defined quarantine directory.
    Block,
}

pub struct RuleState<'a> {
    scope: Scope<'a>,
    ctx: Arc<RwLock<MailContext>>,
    skip: Option<Status>,
}

impl<'a> RuleState<'a> {
    /// creates a new rule engine with an empty scope.
    pub(crate) fn new(config: &crate::config::server_config::ServerConfig) -> Self {
        let mut scope = Scope::new();
        let ctx = Arc::new(RwLock::new(MailContext {
            client_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
            envelop: Envelop::default(),
            body: Body::Raw(String::default()),
            metadata: None,
        }));

        scope
            // stage specific variables.
            .push("ctx", ctx.clone())
            // data available in every stage.
            .push("date", "")
            .push("time", "")
            .push("connection_timestamp", std::time::SystemTime::now())
            .push("metadata", None::<MessageMetadata>)
            // rule engine's internals.
            .push("__OPERATION_QUEUE", OperationQueue::default())
            .push("__stage", "")
            // configuration variables.
            .push("addr", config.server.addr)
            .push("logs_file", config.log.file.clone())
            .push("spool_dir", config.delivery.spool_dir.clone());

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
        let ctx = Arc::new(RwLock::new(ctx));

        scope
            // stage specific variables.
            .push("ctx", ctx.clone())
            // data available in every stage.
            .push("date", "")
            .push("time", "")
            .push("connection_timestamp", std::time::SystemTime::now())
            .push("metadata", None::<MessageMetadata>)
            // rule engine's internals.
            .push("__OPERATION_QUEUE", OperationQueue::default())
            .push("__stage", "")
            // configuration variables.
            .push("addr", config.server.addr)
            .push("logs_file", config.log.file.clone())
            .push("spool_dir", config.delivery.spool_dir.clone());

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

    /// empty the operation queue and executing all operations stored.
    pub(crate) fn execute_operation_queue(
        &mut self,
        config: &ServerConfig,
        ctx: &MailContext,
    ) -> anyhow::Result<()> {
        for op in self
            .scope
            .get_value::<OperationQueue>("__OPERATION_QUEUE")
            .ok_or_else::<rhai::EvalAltResult, _>(|| {
                rhai::ParseErrorType::MissingSymbol("__OPERATION_QUEUE".to_string()).into()
            })?
            .into_iter()
        {
            log::debug!(target: RULES, "executing deferred operation: {:?}", op);
            match op {
                Operation::Block(path) => {
                    let mut path = std::path::PathBuf::from_str(&path)?;
                    let message_id = &ctx.metadata.as_ref().unwrap().message_id;
                    std::fs::create_dir_all(&path)?;

                    path.push(message_id);
                    path.set_extension("json");

                    let mut file = std::fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .open(path)?;

                    std::io::Write::write_all(&mut file, serde_json::to_string(&ctx)?.as_bytes())?;
                    log::warn!(target: RULES, "'{message_id}' email blocked.");
                }
                Operation::Quarantine { reason } => {
                    log::warn!(
                        target: RULES,
                        "'{}' email quarantined: {reason}.",
                        &ctx.metadata.as_ref().unwrap().message_id
                    );

                    Queue::Quarantine.write_to_queue(config, ctx)?
                }
                Operation::MutateHeader(_, _) => todo!("MutateHeader operation not implemented"),
            }
        }

        Ok(())
    }

    /// fetch the email context (possibly) mutated by the user's rules.
    pub(crate) fn get_context(&mut self) -> Arc<RwLock<MailContext>> {
        self.ctx.clone()
    }

    /// clears the state of the rules.
    pub(crate) fn reset(&mut self) {
        let mut ctx = self.ctx.write().unwrap();

        ctx.body = Body::Raw(String::with_capacity(MAIL_CAPACITY));
        ctx.envelop = Envelop::default();
        ctx.metadata = None;
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
    /// the ast, built from the user's .vsl files.
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

        log::debug!(target: RULES, "[{}] evaluating rules.", stage);

        // updating the internal __stage variable, so that the rhai context
        // knows what rules to execute.
        state.scope.set_value("__stage", stage.to_string());

        // injecting date and time variables.
        let now = chrono::Local::now();
        state
            .scope
            .set_value("date", now.date().format("%Y/%m/%d").to_string());
        state
            .scope
            .set_value("time", now.time().format("%H:%M:%S").to_string());

        let result = self
            .context
            .eval_ast_with_scope::<Status>(&mut state.scope, &self.ast);

        // rules are cleared after each evaluation. This way,
        // scoped variables that are changed by previous rules
        // can be re-injected back into the script.
        state.scope.set_value("__rules", Array::new());

        match result {
            Ok(status) => {
                log::debug!(target: RULES, "[{}] evaluated => {:?}.", stage, status);

                if let Status::Block | Status::Faccept = status {
                    log::trace!(
                        target: RULES,
                        "[{}] the rule engine will skip all rules because of the previous result.",
                        stage
                    );
                    state.skip = Some(status);
                }

                status
            }
            Err(error) => {
                log::error!(
                    target: RULES,
                    "the rule engine skipped stage '{}' because it failed to evaluate a rule:\n\t{}",
                    stage, error
                );
                Status::Continue
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

        engine
        .set_module_resolver(FileModuleResolver::new_with_path_and_extension(script_path.as_ref(), "vsl"))
        .register_global_module(exported_module!(crate::rules::modules::actions::actions).into())
        .register_global_module(exported_module!(crate::rules::modules::types::types).into())
        .register_global_module(exported_module!(crate::rules::modules::email::email).into())
        .disable_symbol("eval")

        // `rule $when$ $name$ #{expr}` container syntax.
        .register_custom_syntax_raw(
            "rule",
            |symbols, look_ahead| match symbols.len() {
                // rule keyword ...
                1 => Ok(Some("$ident$".into())),
                // when the rule will be executed ...
                2 => match symbols[1].as_str() {
                    "connect" | "helo" | "mail" | "rcpt" | "preq" | "postq" | "delivery" => {
                        Ok(Some("$string$".into()))
                    }
                    entry => Err(ParseError(
                        Box::new(ParseErrorType::BadInput(LexError::ImproperSymbol(
                            entry.into(),
                            format!("Improper rule stage '{}'. Must be connect, helo, mail, rcpt, preq, postq or delivery.", entry),
                        ))),
                        Position::NONE,
                    )),
                },
                // name of the rule ...
                3 => Ok(Some("$expr$".into())),
                // map, we are done parsing.
                4 => Ok(None),
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
                let when = input[0].get_string_value().unwrap().to_string();
                let name = input[1].get_literal_value::<ImmutableString>().unwrap();
                let map = &input[2];

                let mut rule: Map = context.eval_expression_tree(map)?.cast();
                rule.insert("name".into(), Dynamic::from(name));
                rule.insert("when".into(), Dynamic::from(when));

                Ok(rule.into())
            },
        )

        // `obj $type$ $name$ #{}` container syntax.
        .register_custom_syntax_raw(
            "obj",
            |symbols, look_ahead| match symbols.len() {
                // obj ...
                1 => Ok(Some("$ident$".into())),
                // the type of the object ...
                2 => match symbols[1].as_str() {
                    "ip4" | "ip6" | "rg4" | "rg6" | "fqdn" | "addr" | "str" | "regex" | "grp" => Ok(Some("$string$".into())),
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
                }
                // file content type or info block / value of object, we are done parsing.
                4 => match symbols[3].as_str() {
                    // NOTE: could it be possible to add a "file" content type ?
                    "ip4" | "ip6" | "rg4" | "rg6" | "fqdn" | "addr" | "str" | "regex" => Ok(Some("$string$".into())),
                    _ =>  Ok(None),
                }
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
                        var_name = input[3].get_literal_value::<ImmutableString>().unwrap().to_string();
                        let object = context.eval_expression_tree(&input[4])?;

                        // the object syntax can use a map or an inline string.
                        if object.is::<Map>() {
                            let mut object: Map = object.cast();
                            object.insert("type".into(), Dynamic::from(var_type.clone()));
                            object.insert("name".into(), Dynamic::from(var_name.clone()));
                            object.insert("content_type".into(), Dynamic::from(content_type.to_string()));
                            object
                        } else if object.is::<String>() {
                            let mut map = Map::new();
                            map.insert("type".into(), Dynamic::from(var_type.clone()));
                            map.insert("name".into(), Dynamic::from(var_name.clone()));
                            map.insert("content_type".into(), Dynamic::from(content_type.to_string()));
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
                    },

                    // generic type, we can parse it easily.
                    _ => {
                        var_name = input[1].get_literal_value::<ImmutableString>().unwrap().to_string();
                        let object = context.eval_expression_tree(&input[2])?;

                        if object.is::<Map>() {
                            let mut object: Map = object.cast();
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

                let obj_ptr = Arc::new(Object::from(&object).map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())?);

                // pushing object in scope, preventing a "let _" statement,
                // and returning a reference to the object in case of a parent group.
                context
                    .scope_mut()
                    .push(var_name, obj_ptr.clone());

                Ok(Dynamic::from(obj_ptr))
            },
        )

        // NOTE: is their a way to defined iterators directly in modules ?
        .register_iterator::<crate::rules::modules::types::types::Rcpt>()
        .register_iterator::<Vec<String>>();

        log::debug!(target: RULES, "compiling rhai script ...");

        // compiling and registering the rule executor as a global module.
        let executor = engine
            .compile(include_str!("rule_executor.rhai"))
            .context("failed to compile rule executor")?;

        engine.register_global_module(
            Module::eval_ast_as_new(Scope::new(), &executor, &engine)
                .context("failed load rule executor")?
                .into(),
        );

        let main_path = std::path::PathBuf::from_iter([script_path.as_ref(), "main.vsl"]);

        let mut scope = Scope::new();
        scope
            // stage specific variables.
            .push(
                "ctx",
                Arc::new(RwLock::new(MailContext {
                    client_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0),
                    envelop: Envelop::default(),
                    body: Body::Raw(String::default()),
                    metadata: None,
                })),
            )
            // data available in every stage.
            .push("date", "")
            .push("time", "")
            .push("connection_timestamp", std::time::SystemTime::now())
            .push("metadata", None::<MessageMetadata>)
            // rule engine's internals.
            .push("__OPERATION_QUEUE", OperationQueue::default())
            .push("__stage", "")
            // configuration variables.
            .push("addr", "")
            .push("logs_file", "")
            .push("spool_dir", "");

        // compiling main script.
        let ast = engine
            .compile_into_self_contained(&scope, std::fs::read_to_string(main_path)?)
            .context("failed to load main.vsl")?;

        log::debug!(target: RULES, "done.");

        Ok(Self {
            context: engine,
            ast,
        })
    }
}

/// use the user cache to check if a user exists on the system.
pub(crate) fn user_exists(_name: &str) -> bool {
    // match acquire_engine().users.lock() {
    //     Ok(users) => users.get_user_by_name(name).is_some(),
    //     Err(error) => {
    //         log::error!("FATAL: {}", error);
    //         false
    //     }
    // }
    false
}

/// using the engine's instance, try to get a specific user.
pub(crate) fn get_user_by_name(_name: &str) -> Option<Arc<users::User>> {
    // match acquire_engine().users.lock() {
    //     Ok(users) => users.get_user_by_name(name),
    //     Err(error) => {
    //         log::error!("FATAL: {}", error);
    //         None
    //     }
    // }
    None
}
