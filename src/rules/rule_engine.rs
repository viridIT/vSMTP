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
use crate::config;
use crate::model::envelop::Envelop;
use crate::model::mail::MailContext;

use lazy_static::lazy_static;
use lettre::{Message, SmtpTransport, Transport};
use regex::Regex;
use rhai::{exported_module, Array, Engine, EvalAltResult, LexError, Map, Module, Scope, AST};
use rhai::{plugin::*, ParseError, ParseErrorType};

use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    io::{BufRead, BufReader, Write},
    net::{Ipv4Addr, Ipv6Addr},
    path::Path,
    str::FromStr,
    sync::{Arc, RwLock},
};

#[derive(Debug, Clone)]
pub enum Operation {
    /// header, value
    MutateHeader(String, String),
    /// reason
    Quarantine(String),
}

/// used to yield expensive operations
/// and executing them using rust's context instead of rhai's.
#[derive(Default, Debug, Clone)]
pub struct OperationQueue {
    inner: Vec<Operation>,
}

impl OperationQueue {
    pub fn push(&mut self, op: Operation) {
        self.inner.push(op);
    }
}

#[derive(Debug)]
enum Var {
    Ip4(Ipv4Addr),
    Ip6(Ipv6Addr),
    Address(String),
    Fqdn(String),
    Regex(Regex),
    File(Vec<Var>),

    /// contains a set of objects.
    Group(Vec<Var>),

    /// a string object (default).
    Val(String),
}

impl Var {
    // NOTE: what does the 'static lifetime implies here ?
    fn value<T: 'static + Clone>(map: &Map, key: &str) -> Result<T, Box<dyn Error>> {
        match map.get(key) {
            Some(value) => Ok(value.clone_cast::<T>()),
            None => return Err(format!("{} not found.", key).into()),
        }
    }

    fn from(map: &Map) -> Result<Self, Box<dyn Error>> {
        let t = Var::value::<String>(map, "type")?;

        match t.as_str() {
            "ip4" => Ok(Var::Ip4(Ipv4Addr::from_str(&Var::value::<String>(
                map, "value",
            )?)?)),
            "ip6" => Ok(Var::Ip6(Ipv6Addr::from_str(&Var::value::<String>(
                map, "value",
            )?)?)),
            "fqdn" => {
                let value = Var::value::<String>(map, "value")?;
                match addr::parse_domain_name(&value) {
                    Ok(domain) => Ok(Var::Fqdn(domain.to_string())),
                    Err(_) => Err(format!("'{}' is not a valid fqdn.", value).into()),
                }
            }
            "addr" => {
                let value = Var::value::<String>(map, "value")?;
                match addr::parse_email_address(&value) {
                    Ok(domain) => Ok(Var::Address(domain.to_string())),
                    Err(_) => Err(format!("'{}' is not a valid address.", value).into()),
                }
            }
            "val" => Ok(Var::Val(Var::value::<String>(map, "value")?)),
            "regex" => Ok(Var::Regex(Regex::from_str(&Var::value::<String>(
                map, "value",
            )?)?)),
            "file" => {
                let value = Var::value::<String>(map, "value")?;
                let content_type = Var::value::<String>(map, "content_type")?;
                let reader = BufReader::new(fs::File::open(&value)?);
                let mut content = Vec::with_capacity(20);

                for line in reader.lines() {
                    match line {
                        Ok(line) => match content_type.as_str() {
                            "ip4" => content.push(Var::Ip4(Ipv4Addr::from_str(&line)?)),
                            "ip6" => content.push(Var::Ip6(Ipv6Addr::from_str(&line)?)),
                            "fqdn" => {
                                // TODO: parse fqdn.
                                content.push(Var::Fqdn(line))
                            }
                            "addr" => {
                                // TODO: parse fqdn.
                                content.push(Var::Address(line))
                            }
                            "val" => content.push(Var::Val(line)),
                            "regex" => content.push(Var::Regex(Regex::from_str(&line)?)),
                            _ => {}
                        },
                        Err(error) => log::error!("coudln't read line in '{}': {}", value, error),
                    };
                }

                Ok(Var::File(content))
            }

            "grp" => {
                let mut group = vec![];
                let elements = Var::value::<Array>(map, "value")?;

                for element in elements.iter() {
                    match element.is::<Map>() {
                        true => group.push(Var::from(&element.clone_cast::<Map>())?),
                        false => {
                            return Err(
                                "'{}' is not an inline object or an already defined one.".into()
                            )
                        }
                    }
                }

                Ok(Var::Group(group))
            }

            _ => Err(format!("'{}' is not a known object type.", t).into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Faccept,
    Accept,
    Continue,
    Deny,
}

// exported methods are used in rhai context, so we allow dead code.
#[allow(dead_code)]
#[export_module]
mod vsl {
    use crate::model::{envelop::Envelop, mail::MailContext};

    #[rhai_fn(name = "op_quarantine")]
    pub fn quarantine(queue: &mut OperationQueue, reason: &str) {
        queue.push(Operation::Quarantine(reason.to_string()))
    }

    #[rhai_fn(name = "op_mutate_header")]
    pub fn mutate_header(queue: &mut OperationQueue, header: &str, value: &str) {
        queue.push(Operation::MutateHeader(
            header.to_string(),
            value.to_string(),
        ))
    }

    #[rhai_fn(name = "__FACCEPT")]
    pub fn faccept() -> Status {
        Status::Faccept
    }

    #[rhai_fn(name = "__ACCEPT")]
    pub fn accept() -> Status {
        Status::Accept
    }

    #[rhai_fn(name = "__CONTINUE")]
    pub fn ct() -> Status {
        Status::Continue
    }

    #[rhai_fn(name = "__DENY")]
    pub fn deny() -> Status {
        Status::Deny
    }

    /// logs a message to stdout, stderr or a file.
    #[rhai_fn(name = "__LOG", return_raw)]
    pub fn log(message: &str, path: &str) -> Result<(), Box<EvalAltResult>> {
        match path {
            "stdout" => {
                println!("{}", message);
                Ok(())
            }
            "stderr" => {
                eprintln!("{}", message);
                Ok(())
            }
            _ => {
                let path = std::path::PathBuf::from_str(path).unwrap();
                let file = if !path.exists() {
                    std::fs::File::create(&path)
                } else {
                    std::fs::OpenOptions::new().append(true).open(&path)
                };

                match file {
                    Ok(mut file) => file
                        .write_all(message.as_bytes())
                        .map_err(|_| format!("could not log to '{:?}'.", path).into()),
                    Err(error) => Err(format!(
                        "'{:?}' is not a valid path to log to: {:#?}",
                        path, error
                    )
                    .into()),
                }
            }
        }
    }

    /// write the email to a specified file.
    /// NOTE: this function needs to be curried to access data,
    ///       could it be added to the operation queue ?
    #[rhai_fn(name = "__WRITE", return_raw)]
    pub fn write_mail(data: &str, path: &str) -> Result<(), Box<EvalAltResult>> {
        if data.is_empty() {
            return Err("the WRITE action can only be called in the 'preq' stage or after.".into());
        }

        let path = std::path::PathBuf::from_str(path).unwrap();
        let file = if !path.exists() {
            std::fs::File::create(&path)
        } else {
            std::fs::OpenOptions::new().append(true).open(&path)
        };

        match file {
            Ok(mut file) => file
                .write_all(data.as_bytes())
                .map_err(|_| format!("could not write email to '{:?}'.", path).into()),
            Err(error) => Err(format!(
                "'{:?}' is not a valid path to write the email to: {:#?}",
                path, error
            )
            .into()),
        }
    }

    #[rhai_fn(name = "__DUMP", return_raw)]
    pub fn dump(
        helo: &str,
        mail: &str,
        rcpt: Vec<String>,
        data: &str,
        path: &str,
    ) -> Result<(), Box<EvalAltResult>> {
        if let Err(error) = std::fs::create_dir_all(path) {
            return Err(format!("could not write email to '{:?}': {}", path, error).into());
        }

        let mut file = match std::fs::OpenOptions::new().write(true).create(true).open({
            // Error is of infallible type, we can unwrap.
            let mut path = std::path::PathBuf::from_str(path).unwrap();
            path.push(format!(
                "{}_{}.json",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
            ));
            path
        }) {
            Ok(file) => file,
            Err(error) => {
                return Err(format!("could not write email to '{:?}': {}", path, error).into())
            }
        };

        let ctx = MailContext {
            envelop: Envelop {
                helo: helo.to_string(),
                mail_from: mail.to_string(),
                recipients: rcpt,
            },
            body: data.into(),
        };

        std::io::Write::write_all(&mut file, serde_json::to_string(&ctx).unwrap().as_bytes())
            .map_err(|error| format!("could not write email to '{:?}': {}", path, error).into())
    }

    // NOTE: instead of filling the email using arguments, should we create a 'mail' object
    //       defined beforehand in the user's object files ?
    #[rhai_fn(name = "__MAIL", return_raw)]
    pub fn send_mail(
        from: &str,
        to: &str,
        subject: &str,
        body: &str,
    ) -> Result<(), Box<EvalAltResult>> {
        let email = Message::builder()
            .from(from.parse().unwrap())
            .to(to.parse().unwrap())
            .subject(subject)
            .body(String::from(body))
            .unwrap();

        // Send the email
        match SmtpTransport::unencrypted_localhost().send(&email) {
            Ok(_) => Ok(()),
            Err(error) => Err(EvalAltResult::ErrorInFunctionCall(
                "MAIL".to_string(),
                "__MAIL".to_string(),
                format!("Couldn't send the email: {}", error).into(),
                Position::NONE,
            )
            .into()),
        }
    }

    #[rhai_fn(name = "==")]
    pub fn eq_status_operator(in1: &mut Status, in2: Status) -> bool {
        *in1 == in2
    }

    #[rhai_fn(name = "!=")]
    pub fn neq_status_operator(in1: &mut Status, in2: Status) -> bool {
        !(*in1 == in2)
    }

    // FIXME: 'connect' could be curried with an ipv4 and v6 versions to prevent the conversion.
    pub fn __is_connect(connect: &mut Ipv4Addr, object: &str) -> bool {
        match RHAI_ENGINE.objects.read().unwrap().get(object) {
            Some(object) => internal_is_connect(connect, object),
            None => match Ipv4Addr::from_str(object) {
                Ok(ip) => ip == *connect,
                Err(_) => match Ipv6Addr::from_str(object) {
                    Ok(ip) => ip == connect.to_ipv6_mapped(),
                    Err(_) => {
                        log::error!(
                            target: "rule_engine",
                            "tried to convert '{}' to ipv4 because it is not a object, but convertion failed.",
                            object
                        );
                        false
                    }
                },
            },
        }
    }

    pub fn __is_helo(helo: &str, object: &str) -> bool {
        match RHAI_ENGINE.objects.read().unwrap().get(object) {
            Some(object) => internal_is_helo(helo, object),
            _ => object == helo,
        }
    }

    pub fn __is_mail(mail: &str, object: &str) -> bool {
        match RHAI_ENGINE.objects.read().unwrap().get(object) {
            Some(object) => internal_is_mail(mail, object),
            _ => object == mail,
        }
    }

    pub fn __is_rcpt(rcpt: &mut Vec<String>, object: &str) -> bool {
        match RHAI_ENGINE.objects.read().unwrap().get(object) {
            Some(object) => internal_is_rcpt(rcpt, object),
            _ => rcpt.iter().any(|email| object == *email),
        }
    }

    // TODO: what does the user can do here ?
    pub fn __is_data(_data: &mut Vec<u8>, _object: &str) -> bool {
        false
    }
}

fn internal_is_connect(connect: &Ipv4Addr, object: &Var) -> bool {
    match object {
        Var::Ip4(ip) => *ip == *connect,
        Var::Ip6(ip) => *ip == connect.to_ipv6_mapped(),
        // NOTE: is there a way to get a &str instead of a String here ?
        Var::Regex(re) => re.is_match(connect.to_string().as_str()),
        Var::File(content) => content.iter().any(|ip| match ip {
            Var::Ip4(ip) => *ip == *connect,
            Var::Ip6(ip) => *ip == connect.to_ipv6_mapped(),
            _ => false,
        }),
        Var::Group(group) => group
            .iter()
            .any(|object| internal_is_connect(connect, object)),
        _ => false,
    }
}

fn internal_is_helo(helo: &str, object: &Var) -> bool {
    match object {
        Var::Fqdn(fqdn) => *fqdn == helo,
        Var::Regex(re) => re.is_match(helo),
        Var::File(content) => content.iter().any(|fqdn| match fqdn {
            Var::Fqdn(fqdn) => *fqdn == helo,
            _ => false,
        }),
        Var::Group(group) => group.iter().any(|object| internal_is_helo(helo, object)),
        _ => false,
    }
}

fn internal_is_mail(mail: &str, object: &Var) -> bool {
    match object {
        Var::Address(addr) => *addr == mail,
        Var::Regex(re) => re.is_match(mail),
        Var::File(content) => content.iter().any(|addr| match addr {
            Var::Address(addr) => *addr == mail,
            _ => false,
        }),
        Var::Group(group) => group.iter().any(|object| internal_is_mail(mail, object)),
        _ => false,
    }
}

fn internal_is_rcpt(rcpt: &[String], object: &Var) -> bool {
    match object {
        Var::Address(addr) => rcpt.iter().any(|email| *addr == *email),
        Var::Regex(re) => rcpt.iter().any(|email| re.is_match(email)),
        Var::File(content) => content.iter().any(|addr| match addr {
            Var::Address(addr) => rcpt.iter().any(|email| *addr == *email),
            _ => false,
        }),
        Var::Group(group) => group.iter().any(|object| internal_is_rcpt(rcpt, object)),
        _ => false,
    }
}

/// contains the scope of the connexion and a reference to the RhaiEngine.
pub struct RuleEngine<'a> {
    inner: Scope<'a>,
}

impl<'a> RuleEngine<'a> {
    pub(crate) fn new() -> Self {
        let mut inner = Scope::new();
        inner
            .push("connect", Ipv4Addr::from_str("0.0.0.0"))
            .push("helo", "")
            .push("mail", "")
            .push("rcpt", Vec::<String>::new())
            .push("data", "")
            .push("__OPERATION_QUEUE", OperationQueue::default())
            .push("__step", "")
            .push("__rules", Array::new())
            .push("__init", true)
            .push("addr", config::get::<Vec<String>>("server.addr").unwrap())
            .push(
                "logs_file",
                config::get::<String>("paths.logs_file").unwrap(),
            )
            .push(
                "rules_dir",
                config::get::<String>("paths.rules_dir").unwrap(),
            )
            .push(
                "spool_dir",
                config::get::<String>("paths.spool_dir").unwrap(),
            )
            .push(
                "quarantine_dir",
                config::get::<String>("paths.quarantine_dir").unwrap(),
            )
            .push("clamav", config::get::<String>("clamav").unwrap())
            .push("clamav_port", config::get::<String>("clamav_port").unwrap())
            .push(
                "clamav_address",
                config::get::<String>("clamav_address").unwrap(),
            );

        Self { inner }
    }

    pub(crate) fn add_data<T>(&mut self, name: &'a str, data: T)
    where
        // TODO: find a way to remove the static.
        // maybe create a getter, engine.scope().push(n, v) ?
        T: Clone + Send + Sync + 'static,
    {
        self.inner.push(name, data);
    }

    pub(crate) fn run_when(&mut self, step: &str) -> Status {
        log::debug!(target: "rule_engine", "------ executing rules registered on '{}'.", step);

        self.inner.set_value("__step", step.to_string());
        let result = RHAI_ENGINE
            .context
            .eval_ast_with_scope::<Status>(&mut self.inner, &RHAI_ENGINE.ast);

        log::debug!(target: "rule_engine", "------ evaluation of rules registered on '{}' finished.", step);
        log::trace!(target: "rule_engine", "       result: {:?}.", result);

        // NOTE: does the evaluation risk to crash, even thought the code has been already evaluated once ?
        //        check if a Result<Status, Error> is needed here.
        match result {
            Ok(status) => status,
            Err(_) => Status::Continue,
        }
    }

    pub(crate) fn execute_operation_queue(
        &mut self,
        ctx: &MailContext,
    ) -> Result<(), Box<dyn Error>> {
        for op in self
            .inner
            .get_value::<OperationQueue>("__OPERATION_QUEUE")
            .unwrap()
            .inner
            .iter()
        {
            log::info!(target: "rule_engine", "executing heavy operation: {:?}", op);
            match op {
                // TODO: remove or use the quarantine's reason message.
                Operation::Quarantine(_) => {
                    let folder = config::get::<String>("paths.quarantine_dir")
                        .unwrap_or_else(|_| config::DEFAULT_QUARANTINE_DIR.to_string());
                    std::fs::create_dir_all(&folder)?;

                    let mut file =
                        std::fs::OpenOptions::new()
                            .write(true)
                            .create(true)
                            .open(format!(
                                "{}/{}_{}.json",
                                folder,
                                std::process::id(),
                                std::time::SystemTime::now()
                                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis()
                            ))?;

                    std::io::Write::write_all(&mut file, serde_json::to_string(&ctx)?.as_bytes())?;
                }
                Operation::MutateHeader(_, _) => todo!(),
            }
        }

        Ok(())
    }

    pub(crate) fn get_scoped_envelop(&self) -> Option<Envelop> {
        Some(Envelop {
            helo: self.inner.get_value::<String>("helo")?,
            mail_from: self.inner.get_value::<String>("mail")?,
            recipients: self.inner.get_value::<Vec<String>>("rcpt")?,
        })
    }
}

#[derive(Debug)]
pub(crate) struct RhaiEngine {
    context: Engine,
    ast: AST,

    // ? use SmartString<LazyCompact> ? What about long object names ?
    objects: Arc<RwLock<BTreeMap<String, Var>>>,
}

impl RhaiEngine {
    fn new() -> Result<Self, Box<dyn Error>> {
        let path = config::get::<String>("paths.rules_dir").unwrap();
        let src_path = Path::new(&path);
        let mut engine = Engine::new();

        let objects = Arc::new(RwLock::new(BTreeMap::new()));

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

        let shared_obj = objects.clone();

        // register our vsl global module
        let api_mod = exported_module!(vsl);
        engine
        .register_global_module(api_mod.into())

        // the operation queue is used to defere heavy computation.
        .register_type::<OperationQueue>()

        // adding a string vector as a custom type.
        // it is used to easly manipulate the rcpt container.
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
            |context, input| {
                let when = input[0].get_variable_name().unwrap().to_string();
                let name = input[1].get_literal_value::<ImmutableString>().unwrap();
                let map = &input[2];

                // we parse the rule only if needs to be executed now.
                if let Some(step) = context.scope_mut().get_value::<String>("__step") {
                    if step != when {
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

                // rule is pushed in __rules global so no need to introduce it to the scope.
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
                    "ip4" | "ip6" | "fqdn" | "addr" | "val" | "regex" | "grp" => Ok(Some("$string$".into())),
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
                    "ip4" | "ip6" | "fqdn" | "addr" | "val" | "regex" => Ok(Some("$string$".into())),
                    _ =>  Ok(None),
                }
                // object name for a file.
                5 => Ok(Some("$expr$".into())),
                // done parsing file expression.
                6 => Ok(None),
                _ => Err(ParseError(
                    Box::new(ParseErrorType::BadInput(LexError::UnexpectedInput(
                        format!(
                            "Improper oject declaration: keyword '{}' unknown.",
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

                // checking if object declaration is using a map, an inline string or an array.
                // we create a map either way.
                // FIXME: refactor this expression.
                let object = match var_type.as_str() {
                    "file" => {

                        let content_type = input[2].get_variable_name().unwrap();
                        var_name = input[3].get_literal_value::<ImmutableString>().unwrap().to_string();
                        let object = context.eval_expression_tree(&input[4])?;

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
                match Var::from(&object) {
                    Ok(rust_var) => shared_obj.write()
                        .unwrap()
                        .insert(var_name.to_string(), rust_var),
                    Err(error) => panic!("object '{}' could not be parsed as a '{}' object: {}", var_name, var_type, error),
                };

                // FIXME: there is now way to tell if the parent scope of the object.
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

        let mut src = Vec::with_capacity(100);

        src.extend(include_bytes!("./currying.rhai"));
        src.extend(load_sources(src_path)?.concat().as_bytes());
        src.extend(include_bytes!("./rule_executor.rhai"));

        let src = std::str::from_utf8(&src)?;

        log::debug!(target: "rule_engine", "compiling rhai script ...");
        log::trace!(target: "rule_engine", "sources:\n{}", src);

        let ast = engine.compile(src)?;

        log::debug!(target: "rule_engine", "done.");

        Ok(Self {
            context: engine,
            ast,
            objects,
        })
    }
}

lazy_static! {
    // ! FIXME: this could be slow, locks seems to appen in the engine.
    // ! this could be a solution: https://rhai.rs/book/patterns/parallel.html
    static ref RHAI_ENGINE: RhaiEngine = {
        match RhaiEngine::new() {
            Ok(engine) => engine,
            Err(error) => {
                log::error!("could not initialise the rule engine: {}", error);
                panic!();
            }
        }
    };

    static ref DEFAULT_SCOPE: Scope<'static> = {
        let mut scope = Scope::new();
        scope
        // stage variables.
        .push("connect", Ipv4Addr::from_str("0.0.0.0"))
        .push("helo", "")
        .push("mail", "")
        .push("rcpt", Vec::<String>::new())
        .push("data", "")

        // rule engine's internals.
        .push("__OPERATION_QUEUE", OperationQueue::default())
        .push("__step", "")
        .push("__rules", Array::new())
        .push("__init", false)

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

pub fn init() {
    RHAI_ENGINE
        .context
        .eval_ast_with_scope::<Status>(&mut DEFAULT_SCOPE.clone(), &RHAI_ENGINE.ast)
        .expect("couldn't initialise the rule engine");

    log::debug!(target: "rule_engine", "{} objects found.", RHAI_ENGINE.objects.read().unwrap().len());
    log::trace!(target: "rule_engine", "{:#?}", RHAI_ENGINE.objects.read().unwrap());
}
