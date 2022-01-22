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
use crate::model::{envelop::Envelop, mail::MailContext};

use crate::rules::{
    obj::Object,
    operation_queue::{Operation, OperationQueue},
    rule_engine::{acquire_engine, user_exists, Status},
};

use lettre::{Message, SmtpTransport, Transport};
use rhai::plugin::*;

use super::address::Address;

// exported methods are used in rhai context, so we allow dead code.
#[allow(dead_code)]
#[export_module]
pub(super) mod vsl {
    use std::collections::HashSet;

    use crate::{
        config::log_channel::RULES,
        mime::mail::Mail,
        model::mail::{Body, MessageMetadata},
        rules::address::Address,
    };

    /// enqueue a block operation on the queue.
    pub fn op_block(queue: &mut OperationQueue, path: &str) {
        queue.enqueue(Operation::Block(path.to_string()))
    }

    /// enqueue a header mutation operation on the queue.
    pub fn op_mutate_header(queue: &mut OperationQueue, header: &str, value: &str) {
        queue.enqueue(Operation::MutateHeader(
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

    #[rhai_fn(name = "__BLOCK")]
    pub fn block() -> Status {
        Status::Block
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
                // the only writer on "objects" is called and unlocked
                // at the start of the server, we can unwrap here.
                let path = match acquire_engine().objects.read().unwrap().get(path) {
                    // from_str is infallible, we can unwrap.
                    Some(Object::Var(p)) => {
                        <std::path::PathBuf as std::str::FromStr>::from_str(p.as_str()).unwrap()
                    }
                    _ => <std::path::PathBuf as std::str::FromStr>::from_str(path).unwrap(),
                };

                match std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                {
                    Ok(file) => {
                        let mut writer = std::io::LineWriter::new(file);

                        std::io::Write::write_all(&mut writer, message.as_bytes()).map_err::<Box<
                            EvalAltResult,
                        >, _>(
                            |_| format!("could not log to '{:?}'.", path).into(),
                        )?;
                        std::io::Write::write_all(&mut writer, b"\n")
                            .map_err(|_| format!("could not log to '{:?}'.", path).into())
                    }
                    Err(error) => Err(format!(
                        "'{:?}' is not a valid path to log to: {:#?}",
                        path, error
                    )
                    .into()),
                }
            }
        }
    }

    // NOTE: this function needs to be curried to access data,
    //       could it be added to the operation queue ?
    /// write the email to a specified file.
    #[rhai_fn(name = "__WRITE", return_raw)]
    pub fn write_mail(data: Mail, path: &str) -> Result<(), Box<EvalAltResult>> {
        if data.headers.is_empty() {
            return Err("the WRITE action can only be called after or in the 'preq' stage.".into());
        }

        // from_str is infallible, we can unwrap.
        let path = <std::path::PathBuf as std::str::FromStr>::from_str(path).unwrap();

        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            Ok(mut file) => {
                let (headers, body) = data.to_raw();
                std::io::Write::write_all(&mut file, format!("{}\n{}", headers, body).as_bytes())
                    .map_err(|_| format!("could not write email to '{:?}'.", path).into())
            }
            Err(error) => Err(format!(
                "'{:?}' is not a valid path to write the email to: {:#?}",
                path, error
            )
            .into()),
        }
    }

    /// dumps the content of the current connection in a json file.
    /// if some data is missing because of the current stage, it will
    /// be blank in the json representation.
    /// for example, dumping during the rcpt stage will leave the data
    /// field empty.
    #[rhai_fn(name = "__DUMP", return_raw)]
    pub fn dump(
        helo: &str,
        mail: Address,
        rcpt: HashSet<Address>,
        data: Mail,
        metadata: Option<MessageMetadata>,
        path: &str,
    ) -> Result<(), Box<EvalAltResult>> {
        if let Err(error) = std::fs::create_dir_all(path) {
            return Err(format!("could not write email to '{:?}': {}", path, error).into());
        }

        let mut file = match std::fs::OpenOptions::new().write(true).create(true).open({
            // Error is of type Infallible, we can unwrap.
            let mut path = <std::path::PathBuf as std::str::FromStr>::from_str(path).unwrap();
            path.push(
                metadata
                    .as_ref()
                    .ok_or_else::<Box<EvalAltResult>, _>(|| {
                        "could not dump email, metadata has not been received yet.".into()
                    })?
                    .message_id
                    .clone(),
            );
            path.set_extension("json");
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
                mail_from: mail,
                rcpt,
            },
            body: Body::Parsed(data.into()),
            metadata,
        };

        std::io::Write::write_all(&mut file, serde_json::to_string(&ctx).unwrap().as_bytes())
            .map_err(|error| format!("could not write email to '{:?}': {}", path, error).into())
    }

    // NOTE: instead of filling the email using arguments, should we create a 'mail' object
    //       defined beforehand in the user's object files ?
    /// (WARNING: NOT YET FUNCTIONAL)
    /// sends a mail.
    /// the body can be formatted using html.
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

        // TODO: replace unencrypted_localhost by a valid host.
        // NOTE: unscripted_localhost is used for test purposes.
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

    #[rhai_fn(name = "__R_LOOKUP", return_raw)]
    /// find an address from an object / string literal ip.
    pub fn reverse_lookup(object: &str, port: i64) -> Result<String, Box<EvalAltResult>> {
        use std::net::*;

        match acquire_engine().objects.read().unwrap().get(object) {
            Some(Object::Ip4(addr)) => crate::rules::rule_engine::reverse_lookup(&SocketAddr::new(
                IpAddr::V4(*addr),
                port as u16,
            ))
            .map_err(|error| {
                format!("couldn't process reverse lookup using ipv4: {}", error).into()
            }),

            Some(Object::Ip6(addr)) => crate::rules::rule_engine::reverse_lookup(&SocketAddr::new(
                IpAddr::V6(*addr),
                port as u16,
            ))
            .map_err(|error| {
                format!("couldn't process reverse lookup using ipv6: {}", error).into()
            }),

            _ => match <SocketAddr as std::str::FromStr>::from_str(&format!("{}:{}", object, port))
            {
                Ok(socket) => crate::rules::rule_engine::reverse_lookup(&socket)
                    .map_err(|error| format!("couldn't process reverse lookup: {}", error).into()),
                Err(error) => {
                    Err(format!("couldn't process reverse lookup for {}: {}", object, error).into())
                }
            },
        }
    }

    #[rhai_fn(name = "__R_LOOKUP", return_raw)]
    /// find an address from an IpAddr object (connect).
    pub fn reverse_lookup_from_ip(
        ip: std::net::IpAddr,
        port: i64,
    ) -> Result<String, Box<EvalAltResult>> {
        crate::rules::rule_engine::reverse_lookup(&std::net::SocketAddr::new(ip, port as u16))
            .map_err(|error| {
                format!("couldn't process reverse lookup using ipv4: {}", error).into()
            })
    }

    #[rhai_fn(name = "==")]
    pub fn eq_status_operator(in1: &mut Status, in2: Status) -> bool {
        *in1 == in2
    }

    #[rhai_fn(name = "!=")]
    pub fn neq_status_operator(in1: &mut Status, in2: Status) -> bool {
        !(*in1 == in2)
    }

    /// checks if the object exists and check if it matches against the connect value.
    pub fn __is_connect(connect: &mut std::net::IpAddr, object: &str) -> bool {
        match acquire_engine().objects.read().unwrap().get(object) {
            Some(object) => internal_is_connect(connect, object),
            None => match <std::net::Ipv4Addr as std::str::FromStr>::from_str(object) {
                Ok(ip) => ip == *connect,
                Err(_) => match <std::net::Ipv6Addr as std::str::FromStr>::from_str(object) {
                    Ok(ip) => ip == *connect,
                    Err(_) => {
                        log::error!(
                            target: RULES,
                            "tried to convert '{}' to ipv4 because it is not a object, but conversion failed.",
                            object
                        );
                        false
                    }
                },
            },
        }
    }

    // TODO: the following functions could be refactored as one.
    /// checks if the object exists and check if it matches against the helo value.
    pub fn __is_helo(helo: &str, object: &str) -> bool {
        match acquire_engine().objects.read().unwrap().get(object) {
            Some(object) => internal_is_helo(helo, object),
            _ => object == helo,
        }
    }

    /// checks if the object exists and check if it matches against the mail value.
    pub fn __is_mail(mail: &mut Address, object: &str) -> bool {
        match acquire_engine().objects.read().unwrap().get(object) {
            Some(object) => internal_is_mail(mail, object),
            // TODO: allow for user / domain search with a string.
            _ => object == mail.full(),
        }
    }

    /// checks if the object exists and check if it matches against the rcpt value.
    pub fn __is_rcpt(rcpt: &mut Address, object: &str) -> bool {
        match acquire_engine().objects.read().unwrap().get(object) {
            Some(object) => internal_is_rcpt(rcpt, object),
            // TODO: allow for user / domain search with a string.
            _ => rcpt.full() == object,
        }
    }

    /// check if the given object matches one of the incoming recipients.
    pub fn __contains_rcpt(rcpts: &mut HashSet<Address>, object: &str) -> bool {
        match acquire_engine().objects.read().unwrap().get(object) {
            Some(object) => rcpts.iter().any(|rcpt| internal_is_rcpt(rcpt, object)),
            // TODO: allow for user / domain search with a string.
            _ => rcpts.iter().any(|rcpt| rcpt.full() == object),
        }
    }

    /// checks if the given user exists on the system.
    pub fn __user_exists(object: &str) -> bool {
        match acquire_engine().objects.read().unwrap().get(object) {
            Some(object) => internal_user_exists(object),
            _ => internal_user_exists(&Object::Var(object.to_string())),
        }
    }
}

// NOTE: the following functions use pub(super) because they need to be exposed for tests.
// FIXME: find a way to hide the following function to the parent scope.
/// checks recursively if the current connect value is matching the object's value.
pub(super) fn internal_is_connect(connect: &std::net::IpAddr, object: &Object) -> bool {
    match object {
        Object::Ip4(ip) => *ip == *connect,
        Object::Ip6(ip) => *ip == *connect,
        Object::Rg4(range) => match connect {
            std::net::IpAddr::V4(ip4) => range.contains(ip4),
            _ => false,
        },
        Object::Rg6(range) => match connect {
            std::net::IpAddr::V6(ip6) => range.contains(ip6),
            _ => false,
        },
        // NOTE: is there a way to get a &str instead of a String here ?
        Object::Regex(re) => re.is_match(connect.to_string().as_str()),
        Object::File(content) => content
            .iter()
            .any(|object| internal_is_connect(connect, object)),
        Object::Group(group) => group
            .iter()
            .any(|object| internal_is_connect(connect, object)),
        _ => false,
    }
}

/// checks recursively if the current helo value is matching the object's value.
pub(super) fn internal_is_helo(helo: &str, object: &Object) -> bool {
    match object {
        Object::Fqdn(fqdn) => *fqdn == helo,
        Object::Regex(re) => re.is_match(helo),
        Object::File(content) => content.iter().any(|object| internal_is_helo(helo, object)),
        Object::Group(group) => group.iter().any(|object| internal_is_helo(helo, object)),
        _ => false,
    }
}

/// checks recursively if the current mail value is matching the object's value.
pub(super) fn internal_is_mail(mail: &Address, object: &Object) -> bool {
    match object {
        Object::Var(user) => mail.local_part() == user,
        Object::Fqdn(domain) => mail.domain() == domain,
        Object::Address(addr) => addr == mail,
        Object::Regex(re) => re.is_match(mail.full()),
        Object::File(content) => content.iter().any(|object| internal_is_mail(mail, object)),
        Object::Group(group) => group.iter().any(|object| internal_is_mail(mail, object)),
        _ => false,
    }
}

/// checks recursively if the current rcpt value is matching the object's value.
pub(super) fn internal_is_rcpt(rcpt: &Address, object: &Object) -> bool {
    match object {
        Object::Var(user) => rcpt.local_part() == user,
        Object::Fqdn(domain) => rcpt.domain() == domain,
        Object::Address(addr) => rcpt == addr,
        Object::Regex(re) => re.is_match(rcpt.full()),
        Object::File(content) => content.iter().any(|object| internal_is_rcpt(rcpt, object)),
        Object::Group(group) => group.iter().any(|object| internal_is_rcpt(rcpt, object)),
        _ => false,
    }
}

/// checks recursively if the/all user(s) exists on the system.
pub(super) fn internal_user_exists(user: &Object) -> bool {
    match user {
        Object::Var(user) => user_exists(user),
        Object::Address(addr) => user_exists(addr.local_part()),
        Object::File(content) | Object::Group(content) => content.iter().all(internal_user_exists),
        _ => false,
    }
}
