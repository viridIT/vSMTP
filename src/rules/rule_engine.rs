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
use crate::model::envelop::Envelop;
use crate::model::mail::MailContext;
use crate::rules::obj::Object;
use crate::rules::operation_queue::{Operation, OperationQueue};

use rhai::{exported_module, Array, Engine, EvalAltResult, LexError, Map, Scope, AST};
use rhai::{plugin::*, ParseError, ParseErrorType};

use std::net::IpAddr;
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    net::Ipv4Addr,
    path::Path,
    str::FromStr,
    sync::{Arc, RwLock},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            .push("connect", IpAddr::V4(Ipv4Addr::UNSPECIFIED))
            .push("port", 0)
            .push("helo", "")
            .push("mail", "")
            .push("rcpt", "")
            .push("rcpts", Vec::<String>::new())
            .push("data", "")
            .push("__OPERATION_QUEUE", OperationQueue::default())
            .push("__stage", "")
            .push("__rules", Array::new())
            .push("__init", true)
            .push("date", "")
            .push("time", "")
            .push("msg_id", "")
            .push("addr", config.server.addr.clone())
            .push("logs_file", config.log.file.clone())
            // .push("rules_dir", config.rule.dir)
            .push("spool_dir", config.smtp.spool_dir.clone())
            // quarantine_dir should be ${spool_dir}/quarantine
            // .push(
            //     "quarantine_dir",
            //     config::get::<String>("paths.quarantine_dir").unwrap(),
            // )
            //.push("clamav", config::get::<String>("clamav").unwrap())
            //.push("clamav_port", config::get::<String>("clamav_port").unwrap())
            //.push(
            //    "clamav_address",
            //    config::get::<String>("clamav_address").unwrap(),
            //);
        ;

        Self { scope, skip: None }
    }

    /// add data to the scope of the engine.
    pub(crate) fn add_data<T>(&mut self, name: &'a str, data: T)
    where
        // TODO: find a way to remove the static.
        // maybe create a getter, engine.scope().push(n, v) ?
        T: Clone + Send + Sync + 'static,
    {
        self.scope.set_or_push(name, data);
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

        log::debug!(target: "rule_engine", "[{}] evaluating rules.", stage);

        // updating the internal __stage variable, so that the rhai context
        // knows what rules to execute
        self.scope.set_value("__stage", stage.to_string());

        // injecting date and time variables.
        let now = chrono::Local::now();
        self.scope
            .set_value("date", now.date().format("%Y/%m/%d").to_string());
        self.scope
            .set_value("time", now.time().format("%H:%M:%S").to_string());

        let result = RHAI_ENGINE
            .context
            .eval_ast_with_scope::<Status>(&mut self.scope, &RHAI_ENGINE.ast);

        // FIXME: clarify this comment.
        // rules are cleared after evaluation, this way,
        // scoped variables that are changed by rhai's context
        // can be injected back into fresh new rules.
        self.scope.set_value("__rules", Array::new());

        log::debug!(target: "rule_engine", "[{}] done.", stage);

        match result {
            Ok(status) => {
                log::trace!(target: "rule_engine", "[{}] result: {:?}.", stage, status);

                if let Status::Block | Status::Faccept = status {
                    log::trace!(target: "rule_engine", "[{}] the rule engine will skip all rules because of the previous result.", stage);
                    self.skip = Some(status);
                }

                status
            }
            Err(error) => {
                log::error!(
                    target: "rule_engine",
                    "the rule engine skipped a rule in the '{}' stage because it could not evaluate it: \n\t{}",
                    stage, error
                );
                Status::Continue
            }
        }
    }

    /// empty the operation queue and executing all operations stored.
    pub(crate) fn execute_operation_queue(
        &mut self,
        ctx: &MailContext,
        msg_id: &str,
    ) -> Result<(), Box<dyn Error>> {
        for op in self
            .scope
            .get_value::<OperationQueue>("__OPERATION_QUEUE")
            .unwrap()
            .into_iter()
        {
            log::info!(target: "rule_engine", "executing heavy operation: {:?}", op);
            match op {
                Operation::Block(path) => {
                    let mut path = std::path::PathBuf::from_str(&path)?;
                    std::fs::create_dir_all(&path)?;

                    path.push(msg_id);
                    path.set_extension("json");

                    let mut file = std::fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .open(path)?;

                    std::io::Write::write_all(&mut file, serde_json::to_string(&ctx)?.as_bytes())?;
                }
                Operation::MutateHeader(_, _) => todo!(),
            }
        }

        Ok(())
    }

    /// fetch the whole envelop (possibly) mutated by the user's rules.
    pub(crate) fn get_scoped_envelop(&self) -> Option<Envelop> {
        Some(Envelop {
            helo: self.scope.get_value::<String>("helo")?,
            mail_from: self.scope.get_value::<String>("mail")?,
            rcpt: self.scope.get_value::<Vec<String>>("rcpts")?,
        })
    }
}

/// a sharable rhai engine.
/// contains an ast representing the user's parsed .vsl script files,
/// and objects parsed from rhai's context to rust's, this way,
/// they can be used directly into rust functions, and the engine
/// doesn't need to evaluate them each call.
#[derive(Debug)]
pub(crate) struct RhaiEngine {
    /// rhai's engine structure.
    pub(super) context: Engine,
    /// the ast, built from the user's .vsl files.
    pub(super) ast: AST,

    // ? use SmartString<LazyCompact> ? What about long object names ?
    /// objects parsed from rhai's context.
    /// they are accessible from rust function registered into the engine.
    ///
    /// ! you should not use a writer to modify the variables.
    /// ! objects are immutable.
    pub(super) objects: Arc<RwLock<BTreeMap<String, Object>>>,
}

impl RhaiEngine {
    /// create an engine from a script encoded in raw bytes.
    pub(crate) fn from_bytes(src: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut engine = Engine::new();
        let objects = Arc::new(RwLock::new(BTreeMap::new()));
        let shared_obj = objects.clone();

        // register the vsl global module.
        let api_mod = exported_module!(crate::rules::actions::vsl);
        engine
        .register_global_module(api_mod.into())

        // the operation queue is used to defer actions.
        .register_type::<OperationQueue>()
        .register_type::<std::time::SystemTime>()

        // adding a string vector as a custom type.
        // it is used to easily manipulate the rcpt container.
        .register_iterator::<Vec<String>>()
        .register_fn("push", <Vec<String>>::push)

        // NOTE: we can't register Vec<String>::remove & replace method because usize doesn't exists in rhai.
        //       here we create custom replacements that accepts i64 values.
        .register_fn("custom_remove", |vec: &mut Vec<String>, index: i64| {

            if index as usize >= vec.len() {
                return;
            }

            vec.remove(index as usize);
        })
        .register_fn("custom_replace", |vec: &mut Vec<String>, index: i64, value: String| {
            if index as usize >= vec.len() {
                return;
            }

            vec[index as usize] = value;
        })

        // eval is not authorized.
        .disable_symbol("eval")

        // `rule $when$ $name$ #{}` container syntax.
        .register_custom_syntax_raw(
            "rule",
            |symbols, look_ahead| match symbols.len() {
                // rule keyword ...
                1 => Ok(Some("$ident$".into())),
                // when the rule will be executed ...
                2 => match symbols[1].as_str() {
                    "connect" | "helo" | "mail" | "rcpt" | "preq" => {
                        Ok(Some("$string$".into()))
                    }
                    entry => Err(ParseError(
                        Box::new(ParseErrorType::BadInput(LexError::ImproperSymbol(
                            entry.into(),
                            format!("Improper rule stage '{}'. Must be connect, helo, mail, rcpt or preq.", entry),
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
                let when = input[0].get_variable_name().unwrap().to_string();
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
                // we parse the object only once.
                if let Some(true) = context.scope_mut().get_value::<bool>("__init") {
                    return Ok(Dynamic::UNIT);
                }

                let var_type = input[0].get_variable_name().unwrap().to_string();
                let var_name: String;

                // FIXME: refactor this expression.
                // file type as a special syntax (file:type),
                // so we need a different method to parse it.
                let object = match var_type.as_str() {
                    "file" => {

                        let content_type = input[2].get_variable_name().unwrap();
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

                    // generic type, we can parse it easlly.
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

                // injecting the object in rust's scope.
                match Object::from(&object) {
                    Ok(rust_var) => shared_obj.write()
                        .unwrap()
                        .insert(var_name.to_string(), rust_var),
                    Err(error) => panic!("object '{}' could not be parsed as a '{}' object: {}", var_name, var_type, error),
                };

                // FIXME: there is no way to tell if the parent scope of the object
                //        is a group or the global scope, so we have to inject the variable
                //        two times, one in the case of the global scope, one
                //        in the case of the parent being a group.

                // injecting the object in rhai's scope as a new variable.
                context
                    .scope_mut()
                    .push_dynamic(var_name, Dynamic::from(object.clone()));

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

        log::debug!(target: "rule_engine", "compiling rhai script ...");
        log::trace!(target: "rule_engine", "sources:\n{}", script);

        let ast = engine.compile(script)?;

        log::debug!(target: "rule_engine", "done.");

        Ok(Self {
            context: engine,
            ast,
            objects,
        })
    }

    /// creates a new instance of the rule engine, reading all files in
    /// paths.rules_dir configuration variable.
    fn new() -> Result<Self, Box<dyn Error>> {
        // TODO:
        // let path = config::get::<String>("paths.rules_dir").unwrap();
        let src_path = Path::new("./config/rules");

        // load all sources from file.
        // this function is declared here since it isn't needed anywhere else.
        fn load_sources(path: &Path) -> Result<Vec<String>, Box<dyn Error>> {
            let mut buffer = vec![];

            if path.is_file() {
                buffer.push(format!("{}\n", fs::read_to_string(path)?));
            } else if path.is_dir() {
                for entry in fs::read_dir(path)? {
                    let dir = entry?;
                    buffer.extend(load_sources(&dir.path())?);
                }
            }

            Ok(buffer)
        }

        RhaiEngine::from_bytes(load_sources(src_path)?.concat().as_bytes())
    }
}

lazy_static::lazy_static! {
    // ! FIXME: this could be slow, locks seems to happen in the engine.
    // ! this could be a solution: https://rhai.rs/book/patterns/parallel.html
    /// the rhai engine static that gets initialized once.
    /// it is used internally to evaluate user's scripts with a scope
    /// different for each connection.
    pub(super) static ref RHAI_ENGINE: RhaiEngine = {
        match RhaiEngine::new() {
            Ok(engine) => engine,
            Err(error) => {
                log::error!("could not initialize the rule engine: {}", error);
                panic!();
            }
        }
    };

    /// an scope that initialize all needed variables.
    pub(crate) static ref DEFAULT_SCOPE: Scope<'static> = {
        let mut scope = Scope::new();
        scope
        // stage variables.
        .push("connect", IpAddr::V4(Ipv4Addr::UNSPECIFIED))
        .push("port", 0)
        .push("helo", "")
        .push("mail", "")
        .push("rcpt", "")
        .push("rcpts", Vec::<String>::new())
        .push("data", "")

        // rule engine's internals.
        .push("__OPERATION_QUEUE", OperationQueue::default())
        .push("__stage", "")
        .push("__rules", Array::new())
        .push("__init", false)

        // useful data.
        .push("date", "")
        .push("time", "")
        .push("connection_timestamp", std::time::SystemTime::now())
        .push("mail_timestamp", None::<std::time::SystemTime>)

        // configuration variables.
        .push("addr", Vec::<String>::new())
        .push("logs_file", "")
        .push("rules_dir", "")
        .push("spool_dir", "")
        .push("quarantine_dir", "")
        .push("clamav", "")
        .push("clamav_port", "")
        .push("clamav_address", "");

        scope
    };
}

/// initialize the rule engine.
/// this function checks your given scripts and parses all necessary items.
///
/// not calling this method when initializing your server could lead to
/// uncached configuration error and a slow process for the first connection.
pub fn init() {
    RHAI_ENGINE
        .context
        .eval_ast_with_scope::<Status>(&mut DEFAULT_SCOPE.clone(), &RHAI_ENGINE.ast)
        .expect("couldn't initialize the rule engine");

    log::debug!(target: "rule_engine", "{} objects found.", RHAI_ENGINE.objects.read().unwrap().len());
    log::trace!(target: "rule_engine", "{:#?}", RHAI_ENGINE.objects.read().unwrap());
}
