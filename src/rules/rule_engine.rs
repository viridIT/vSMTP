/**
 * vSMTP mail transfer agent
 * Copyright (C) 2021 viridIT SAS
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
use crate::mime::mail::{BodyType, Mail};
use crate::model::envelop::Envelop;
use crate::model::mail::{MailContext, MessageMetadata};
use crate::queue::Queue;
use crate::rules::address::Address;
use crate::rules::obj::Object;
use crate::rules::operation_queue::{Operation, OperationQueue};

use rhai::{exported_module, Array, Engine, EvalAltResult, LexError, Map, Scope, AST};
use rhai::{plugin::*, ParseError, ParseErrorType};
use users::Users;

use std::net::IpAddr;
use std::sync::Mutex;
use std::{
    collections::{BTreeMap, HashSet},
    net::Ipv4Addr,
    path::Path,
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

pub struct RuleEngine<'a> {
    scope: Scope<'a>,
    skip: Option<Status>,
}

impl<'a> RuleEngine<'a> {
    /// creates a new rule engine with an empty scope.
    pub(crate) fn new(config: &crate::config::server_config::ServerConfig) -> Self {
        let mut scope = Scope::new();
        scope
            // stage variables.
            .push("connect", IpAddr::V4(Ipv4Addr::UNSPECIFIED))
            .push("port", 0)
            .push("helo", "")
            .push("mail", Address::default())
            .push("rcpt", Address::default())
            .push("rcpts", HashSet::<Address>::new())
            .push("data", Mail::default())
            // rule engine's internals.
            .push("__OPERATION_QUEUE", OperationQueue::default())
            .push("__stage", "")
            .push("__rules", Array::new())
            .push("__init", false)
            // useful data.
            .push("date", "")
            .push("time", "")
            .push("connection_timestamp", std::time::SystemTime::now())
            .push("metadata", None::<MessageMetadata>)
            // configuration variables.
            .push("addr", config.server.addr)
            .push("logs_file", config.log.file.clone())
            .push("spool_dir", config.delivery.spool_dir.clone());

        Self { scope, skip: None }
    }

    /// add data to the scope of the engine.
    pub(crate) fn add_data<T>(&mut self, name: &'a str, data: T) -> &mut Self
    where
        // TODO: find a way to remove the static.
        // maybe create a getter, engine.scope().push(n, v) ?
        T: Clone + Send + Sync + 'static,
    {
        self.scope.set_or_push(name, data);
        self
    }

    /// fetch data from the scope, cloning the variable in the process.
    pub(crate) fn get_data<T>(&mut self, name: &'a str) -> Option<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        self.scope.get_value(name)
    }

    /// run the engine for a specific stage. (connect, helo, mail, etc ...)
    pub(crate) fn run_when(&mut self, stage: &str) -> Status {
        if let Some(status) = self.skip {
            return status;
        }

        log::debug!(target: RULES, "[{}] evaluating rules.", stage);

        // updating the internal __stage variable, so that the rhai context
        // knows what rules to execute.
        self.scope.set_value("__stage", stage.to_string());

        // injecting date and time variables.
        let now = chrono::Local::now();
        self.scope
            .set_value("date", now.date().format("%Y/%m/%d").to_string());
        self.scope
            .set_value("time", now.time().format("%H:%M:%S").to_string());

        let result = acquire_engine()
            .context
            .eval_ast_with_scope::<Status>(&mut self.scope, &acquire_engine().ast);

        // rules are cleared after each evaluation. This way,
        // scoped variables that are changed by previous rules
        // can be re-injected back into the script.
        self.scope.set_value("__rules", Array::new());

        log::debug!(target: RULES, "[{}] evaluated.", stage);

        match result {
            Ok(status) => {
                log::trace!(target: RULES, "[{}] result: {:?}.", stage, status);

                if let Status::Block | Status::Faccept = status {
                    log::trace!(
                        target: RULES,
                        "[{}] the rule engine will skip all rules because of the previous result.",
                        stage
                    );
                    self.skip = Some(status);
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

    /// fetch the whole envelop (possibly) mutated by the user's rules.
    pub(crate) fn get_scoped_envelop(&self) -> Option<(Envelop, Option<MessageMetadata>, Mail)> {
        Some((
            Envelop {
                helo: self.scope.get_value::<String>("helo")?,
                mail_from: self.scope.get_value::<Address>("mail")?,
                rcpt: self.scope.get_value::<HashSet<Address>>("rcpts")?,
            },
            self.scope
                .get_value::<Option<MessageMetadata>>("metadata")?,
            self.scope.get_value::<Mail>("data")?,
        ))
    }

    /// clears mail_from, metadata, rcpt, rcpts & data values from the scope.
    pub(crate) fn reset(&mut self) {
        self.scope
            .push("mail", Address::default())
            .push("metadata", None::<MessageMetadata>)
            .push("rcpt", Address::default())
            .push("rcpts", HashSet::<Address>::new())
            .push("data", Mail::default());
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
pub struct RhaiEngine<U: Users> {
    /// rhai's engine structure.
    pub(super) context: Engine,
    /// the ast, built from the user's .vsl files.
    pub(super) ast: AST,

    // ? use SmartString<LazyCompact> ? What about long object names ?
    /// objects parsed from rhai's context.
    /// they are accessible from rust function registered into the engine.
    ///
    /// FIXME: remove RwLock, objects are immutable.
    pub(super) objects: Arc<RwLock<BTreeMap<String, Object>>>,

    /// system user cache, used for retrieving user information. (used in vsl.USER_EXISTS for example)
    pub(super) users: Mutex<U>,
}

impl<U: Users> RhaiEngine<U> {
    /// create an engine from a script encoded in raw bytes.
    pub fn from_bytes(src: &[u8], users: U) -> anyhow::Result<Self> {
        let mut engine = Engine::new();
        let objects = Arc::new(RwLock::new(BTreeMap::new()));
        let shared_obj = objects.clone();

        // register the vsl global module.
        let api_mod = exported_module!(crate::rules::actions::vsl);
        engine
        .register_global_module(api_mod.into())

        .register_type::<Address>()
        .register_result_fn("new_address", <Address>::rhai_wrapper)
        .register_fn("to_string", |addr: &mut Address| addr.full().to_string())
        .register_fn("to_debug", |addr: &mut Address| format!("{:?}", addr))
        .register_fn("to_string", |addr: &mut IpAddr| addr.to_string())
        .register_fn("to_debug", |addr: &mut IpAddr| format!("{:?}", addr))

        // local_part + "@" + domain = full.
        .register_get("full", |addr: &mut Address| addr.full().to_string())
        .register_get("local_part", |addr: &mut Address| addr.local_part().to_string())
        .register_get("domain", |addr: &mut Address| addr.domain().to_string())

        // metadata of the email.
        .register_type::<Option<MessageMetadata>>()
        .register_get_result("timestamp", |metadata: &mut Option<MessageMetadata>| match metadata {
            Some(metadata) => Ok(metadata.timestamp),
            None => Err("metadata are not available in the current stage".into())
        })
        .register_get_result("message_id", |metadata: &mut Option<MessageMetadata>| match metadata {
            Some(metadata) => Ok(metadata.message_id.clone()),
            None => Err("metadata are not available in the current stage".into())
        })
        .register_get_result("retry", |metadata: &mut Option<MessageMetadata>| match metadata {
            Some(metadata) => Ok(metadata.retry as u64),
            None => Err("metadata are not available in the current stage".into())
        })
        .register_fn("to_string", |metadata: &mut Option<MessageMetadata>| format!("{:?}", metadata))
        .register_fn("to_debug", |metadata: &mut Option<MessageMetadata>| format!("{:?}", metadata))
        .register_set_result("resolver", |metadata: &mut Option<MessageMetadata>, resolver: String| match metadata {
            Some(metadata) => {
                metadata.resolver = resolver;
                Ok(())
            },
            None => Err("metadata are not available in the current stage".into())
        })

        // exposed structure used to read & rewrite the incoming email's content.
        .register_type::<Mail>()
        .register_get("headers", |mail: &mut Mail| mail.headers.clone())
        .register_get("body", |mail: &mut Mail| mail.body.clone())
        .register_result_fn  ("rewrite_from", |mail: &mut Mail, value: &str| {
            if mail.body == BodyType::Undefined {
                Err("failed to execute 'RW_MAIL': body is undefined".into())
            } else {
                mail.rewrite_from(value);
                Ok(())
            }
        })
        .register_result_fn  ("rewrite_rcpt", |mail: &mut Mail, old: &str, new: &str| {
            if mail.body == BodyType::Undefined {
                Err("failed to execute 'RW_RCPT': body is undefined".into())
            } else {
                mail.rewrite_rcpt(old, new);
                Ok(())
            }
        })
        .register_result_fn  ("add_rcpt", |mail: &mut Mail, new: &str| {
            if mail.body == BodyType::Undefined {
                Err("failed to execute 'ADD_RCPT': body is undefined".into())
            } else {
                mail.add_rcpt(new);
                Ok(())
            }
        })
        .register_result_fn  ("delete_rcpt", |mail: &mut Mail, old: &str| {
            if mail.body == BodyType::Undefined {
                Err("failed to execute 'DEL_RCPT': body is undefined".into())
            } else {
                mail.delete_rcpt(old);
                Ok(())
            }
        })

        // the operation queue is used to defer actions.
        .register_type::<OperationQueue>()

        // time display.
        .register_type::<std::time::SystemTime>()
        .register_fn("to_string", |time: &mut std::time::SystemTime| format!("{}",
            time.duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::ZERO)
                .as_secs()
        ))
        .register_fn("to_debug", |time: &mut std::time::SystemTime| format!("{:?}",
            time.duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_secs()
        ))

        // adding an Address hash set as a custom type.
        // used to easily manipulate the rcpt container.
        .register_iterator::<HashSet<Address>>()
        .register_iterator::<Vec<String>>()
        .register_fn("insert", <HashSet<Address>>::insert)
        // extract all users / domains from the rcpt set.
        .register_get("local_part", |set: &mut HashSet<Address>| -> Vec<String> {
            set.iter().map(|addr| addr.local_part().to_string()).collect()
        })
        .register_get("domain", |set: &mut HashSet<Address>| -> Vec<String> {
            set.iter().map(|addr| addr.domain().to_string()).collect()
        })

        // added an overload to insert an address using a string.
        .register_result_fn("insert", |set: &mut HashSet::<Address>, value: String| {
            match Address::new(&value) {
                Ok(addr) => {
                    set.insert(addr);
                    Ok(())
                },
                Err(error) =>
                    Err(format!(
                        "failed to insert address in set: {}",
                        error
                    )
                    .into()),
            }
        })

        // need to overload remove because the address isn't passed by ref in rhai.
        .register_fn("remove", |set: &mut HashSet::<Address>, addr: Address| {
            set.remove(&addr);
        })

        // added an overload to remove an address using a string.
        .register_result_fn("remove", |set: &mut HashSet::<Address>, value: String| {
            match Address::new(&value) {
                Ok(addr) => {
                    set.remove(&addr);
                    Ok(())
                },
                Err(error) => Err(format!(
                    "failed to remove address from set: {}",
                    error
                )
                .into()),
            }
        })

        // added an overload to replace an address using a string.
        .register_result_fn("replace", |set: &mut HashSet::<Address>, to_replace: String, value: String| {
            let to_replace = match Address::new(&to_replace) {
                Ok(addr) => addr,
                Err(error) => return Err(format!(
                    "failed to replace address from set: {}",
                    error
                )
                .into()),
            };

            if set.contains(&to_replace) {
                set.remove(&to_replace);
                match Address::new(&value) {
                    Ok(addr) => set.insert(addr),
                    Err(error) => return Err(format!(
                        "failed to replace address from set: {}",
                        error
                    )
                    .into()),
                };
            }

            Ok(())
        })

        // eval is not authorized.
        .disable_symbol("eval")

        // `rule $when$ $name$ #{expr}` container syntax.
        .register_custom_syntax_raw(
            "rule",
            |symbols, look_ahead| match symbols.len() {
                // rule keyword ...
                1 => Ok(Some("$ident$".into())),
                // when the rule will be executed ...
                2 => match symbols[1].as_str() {
                    "connect" | "helo" | "mail" | "rcpt" | "preq" | "postq" => {
                        Ok(Some("$string$".into()))
                    }
                    entry => Err(ParseError(
                        Box::new(ParseErrorType::BadInput(LexError::ImproperSymbol(
                            entry.into(),
                            format!("Improper rule stage '{}'. Must be connect, helo, mail, rcpt, preq or postq.", entry),
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

                // we parse the rule only if needs to be executed now.
                if let Some(stage) = context.scope().get_value::<String>("__stage") {
                    if stage != when {
                        return Ok(Dynamic::UNIT);
                    }
                }

                let mut rule: Map = context.eval_expression_tree(map)?.cast();
                rule.insert("name".into(), Dynamic::from(name));
                rule.insert("when".into(), Dynamic::from(when));

                // TODO: rules are re-cloned every call, to optimize.
                if let Some(mut rules) = context.scope_mut().get_value::<Array>("__rules") {
                    rules.push(Dynamic::from(rule));
                    context
                    .scope_mut()
                    .push_dynamic("__rules", Dynamic::from(rules));
                }

                // the rule body is pushed in __rules global so no need to introduce it to the scope.
                Ok(Dynamic::UNIT)
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
                    "ip4" | "ip6" | "rg4" | "rg6" | "fqdn" | "addr" | "val" | "regex" | "grp" => Ok(Some("$string$".into())),
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
                    "ip4" | "ip6" | "rg4" | "rg6" | "fqdn" | "addr" | "val" | "regex" => Ok(Some("$string$".into())),
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
                            object.insert("content_type".into(), Dynamic::from(content_type.to_string()));
                            object
                        } else if object.is::<String>() {
                            let mut map = Map::new();
                            map.insert("type".into(), Dynamic::from(var_type.clone()));
                            map.insert("content_type".into(), Dynamic::from(content_type.to_string()));
                            map.insert("value".into(), object);
                            map
                        } else {
                            return Err(EvalAltResult::ErrorMismatchDataType(
                                "".to_string(),
                                "The 'vars' keyword is not followed by a map or a string".to_string(),
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
                            object
                        } else if object.is::<String>() || object.is::<Array>() {
                            let mut map = Map::new();
                            map.insert("type".into(), Dynamic::from(var_type.clone()));
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

                // we inject objects only once in Rust's scope.
                if let Some(false) = context.scope_mut().get_value::<bool>("__init") {
                    match Object::from(&object) {
                        // write is called once at initialization, no need to check the result.
                        Ok(rust_var) => shared_obj.write()
                            .unwrap()
                            .insert(var_name.clone(), rust_var),
                        Err(error) => panic!("object '{}' could not be parsed as a '{}' object: {}", var_name, var_type, error),
                    };
                }

                // FIXME: there is no way to tell if the parent scope of the object
                //        is a group or the global scope, so we have to inject the variable
                //        two times, one in the case of the global scope and one
                //        in the case of the parent being a group.
                context
                    .scope_mut()
                    .push(var_name, object.clone());

                // the object is returned in case of groups.
                Ok(object.into())
            },
        );

        let mut script = Vec::with_capacity(100);

        // loading scripts that will curry function that needs special
        // variables from stages (helo, rcpt etc ...) and that will
        // execute the rule engine stage logic.
        script.extend(include_bytes!("./currying.rhai"));
        script.extend(src);
        script.extend(include_bytes!("./rule_executor.rhai"));

        let script = std::str::from_utf8(&script)?;

        log::debug!(target: RULES, "compiling rhai script ...");
        log::trace!(target: RULES, "sources:\n{}", script);

        let ast = engine.compile(script)?;

        log::debug!(target: RULES, "done.");

        Ok(Self {
            context: engine,
            ast,
            objects,
            users: Mutex::new(users),
        })
    }
}

#[cfg(not(test))]
impl RhaiEngine<users::UsersCache> {
    /// creates a new instance of the rule engine, reading all files in
    /// src_path parameter.
    fn new(src_path: &str) -> anyhow::Result<Self> {
        // load all sources from file.
        // this function is declared here since it isn't needed anywhere else.
        fn load_sources(path: &Path) -> std::io::Result<Vec<String>> {
            let mut buffer = vec![];

            if path.is_file() {
                match path.extension() {
                    Some(extension) if extension == "vsl" => {
                        buffer.push(format!("{}\n", std::fs::read_to_string(path)?))
                    }
                    _ => {}
                };
            } else if path.is_dir() {
                for entry in std::fs::read_dir(path)? {
                    let dir = entry?;
                    buffer.extend(load_sources(&dir.path())?);
                }
            }

            Ok(buffer)
        }

        let cache = users::UsersCache::default();

        RhaiEngine::from_bytes(
            load_sources(Path::new(src_path))?.concat().as_bytes(),
            cache,
        )
    }
}

#[cfg(test)]
impl RhaiEngine<users::mock::MockUsers> {
    /// creates a new instance of the rule engine, used for tests.
    /// allow unused is () because this new static method is
    /// for tests only.
    pub(super) fn new(src_path: &str, users: users::mock::MockUsers) -> anyhow::Result<Self> {
        // load all sources from file.
        // this function is declared here since it isn't needed anywhere else.
        fn load_sources(path: &Path) -> std::io::Result<Vec<String>> {
            let mut buffer = vec![];

            if path.is_file() {
                match path.extension() {
                    Some(extension) if extension == "vsl" => {
                        buffer.push(format!("{}\n", std::fs::read_to_string(path)?))
                    }
                    _ => {}
                };
            } else if path.is_dir() {
                for entry in std::fs::read_dir(path)? {
                    let dir = entry?;
                    buffer.extend(load_sources(&dir.path())?);
                }
            }

            Ok(buffer)
        }

        RhaiEngine::from_bytes(
            load_sources(Path::new(src_path))?.concat().as_bytes(),
            users,
        )
    }
}

lazy_static::lazy_static! {
    /// a scope that initialize all needed variables by default.
    pub(crate) static ref DEFAULT_SCOPE: Scope<'static> = {
        let mut scope = Scope::new();
        scope
        // stage variables.
        .push("connect", IpAddr::V4(Ipv4Addr::UNSPECIFIED))
        .push("port", 0)
        .push("helo", "")
        .push("mail", Address::default())
        .push("rcpt", Address::default())
        .push("rcpts", HashSet::<Address>::new())
        .push("data", Mail::default())

        // rule engine's internals.
        .push("__OPERATION_QUEUE", OperationQueue::default())
        .push("__stage", "")
        .push("__rules", Array::new())
        .push("__init", false)

        // useful data.
        .push("date", "")
        .push("time", "")
        .push("connection_timestamp", std::time::SystemTime::now())
        .push("metadata", None::<MessageMetadata>)

        // configuration variables.
        .push("addr", Vec::<String>::new())
        .push("logs_file", "")
        .push("spool_dir", "");

        scope
    };
}

#[cfg(not(test))]
lazy_static::lazy_static! {
    // ! FIXME: this could be slow, locks seems to happen in the engine.
    // ! this could be a solution: https://rhai.rs/book/patterns/parallel.html
    /// the rhai engine static that gets initialized once.
    /// it is used internally to evaluate user's scripts with a different
    /// scope for each connection.
    pub(super) static ref RHAI_ENGINE: RhaiEngine<users::UsersCache> = {
        match RhaiEngine::<users::UsersCache>::new(unsafe { RULES_PATH }) {
            Ok(engine) => engine,
            Err(_) => {
                panic!("rules::rule_engine::init() should be called before using the engine.");
            }
        }
    };
}

#[cfg(test)]
lazy_static::lazy_static! {
    // ! FIXME: this could be slow, locks seems to happen in the engine.
    // ! this could be a solution: https://rhai.rs/book/patterns/parallel.html
    /// the rhai engine static that gets initialized once.
    /// it is used internally to evaluate user's scripts with a different
    /// scope for each connection.
    pub(super) static ref RHAI_ENGINE: RwLock<RhaiEngine<users::mock::MockUsers>> = {
        match RhaiEngine::<users::mock::MockUsers>::new(unsafe { RULES_PATH }, users::mock::MockUsers::with_current_uid(1)) {
            Ok(engine) => RwLock::new(engine),
            Err(error) => {
                panic!("could not initialize the rule engine: {error}");
            }
        }
    };
}

static mut RULES_PATH: &str = "./config/rules";
static INIT_RULES_PATH: std::sync::Once = std::sync::Once::new();

/// initialize the default rule path.
/// this is mainly used for test purposes, and does not
/// need to be used most of the time.
pub fn set_rules_path(src: &'static str) {
    INIT_RULES_PATH.call_once(|| unsafe {
        RULES_PATH = src;
    })
}

/// initialize the rule engine.
/// this function checks your given scripts and parses all necessary items.
///
/// not calling this method when initializing your server could lead to
/// undetected configuration errors and a slow process for the first connection.
#[cfg(not(test))]
pub fn init(src: &'static str) -> anyhow::Result<()> {
    set_rules_path(src);

    // creating a temporary engine to try construction.
    match RhaiEngine::<users::UsersCache>::new(unsafe { RULES_PATH }) {
        Ok(engine) => engine,
        Err(error) => anyhow::bail!(error),
    };

    acquire_engine()
        .context
        .eval_ast_with_scope::<Status>(&mut DEFAULT_SCOPE.clone(), &acquire_engine().ast)?;

    log::debug!(
        target: RULES,
        "{} objects found.",
        acquire_engine().objects.read().unwrap().len()
    );
    log::trace!(
        target: RULES,
        "{:#?}",
        acquire_engine().objects.read().unwrap()
    );

    Ok(())
}

#[cfg(not(test))]
/// acquire a ref of the engine for production code.
pub(super) fn acquire_engine() -> &'static RhaiEngine<users::UsersCache> {
    &RHAI_ENGINE
}

#[cfg(test)]
/// mock the acquiring of the engine for test code,
/// because the test Rhai Engine is locked behind a mutex.
pub fn acquire_engine() -> std::sync::RwLockReadGuard<'static, RhaiEngine<users::mock::MockUsers>> {
    RHAI_ENGINE
        .read()
        .expect("engine mutex couldn't be locked for tests")
}

/// use the user cache to check if a user exists on the system.
pub(crate) fn user_exists(name: &str) -> bool {
    match acquire_engine().users.lock() {
        Ok(users) => users.get_user_by_name(name).is_some(),
        Err(error) => {
            log::error!("FATAL: {}", error);
            false
        }
    }
}

/// using the engine's instance, try to get a specific user.
pub(crate) fn get_user_by_name(name: &str) -> Option<Arc<users::User>> {
    match acquire_engine().users.lock() {
        Ok(users) => users.get_user_by_name(name),
        Err(error) => {
            log::error!("FATAL: {}", error);
            None
        }
    }
}
