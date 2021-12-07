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
    rule_engine::{Status, RHAI_ENGINE},
};

use std::{
    io::Write,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    process::Command,
    str::FromStr,
};

use lettre::{Message, SmtpTransport, Transport};
use rhai::plugin::*;

// exported methods are used in rhai context, so we allow dead code.
#[allow(dead_code)]
#[export_module]
pub(super) mod vsl {
    use super::*;

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
                let path = std::path::PathBuf::from_str(path).unwrap();

                // if the file is already containing data, we just append at the end.
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

    // NOTE: this function needs to be curried to access data,
    //       could it be added to the operation queue ?
    /// write the email to a specified file.
    #[rhai_fn(name = "__WRITE", return_raw)]
    pub fn write_mail(data: &str, path: &str) -> Result<(), Box<EvalAltResult>> {
        if data.is_empty() {
            return Err("the WRITE action can only be called after or in the 'preq' stage.".into());
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

    /// dumps the content of the current connection in a json file.
    /// if some data is missing because of the current stage, it will
    /// be blank in the json representation.
    /// for example, dumping during the rcpt stage will leave the data
    /// field empty.
    #[rhai_fn(name = "__DUMP", return_raw)]
    pub fn dump(
        helo: &str,
        mail: &str,
        rcpt: Vec<String>,
        data: &str,
        msg_id: &str,
        path: &str,
    ) -> Result<(), Box<EvalAltResult>> {
        if let Err(error) = std::fs::create_dir_all(path) {
            return Err(format!("could not write email to '{:?}': {}", path, error).into());
        }

        let mut file = match std::fs::OpenOptions::new().write(true).create(true).open({
            // Error is of type Infallible, we can unwrap.
            let mut path = std::path::PathBuf::from_str(path).unwrap();
            path.push(msg_id);
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
                mail_from: mail.to_string(),
                rcpt,
            },
            body: data.into(),
            connection: todo!(),
            timestamp: todo!(),
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
        // NOTE: unencrypted_localhost is used for test purposes.
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

    /// checks if the object exists and check if it matches against the connect value.
    pub fn __is_connect(connect: &mut IpAddr, object: &str) -> bool {
        match RHAI_ENGINE.objects.read().unwrap().get(object) {
            Some(object) => internal_is_connect(connect, object),
            None => match Ipv4Addr::from_str(object) {
                Ok(ip) => ip == *connect,
                Err(_) => match Ipv6Addr::from_str(object) {
                    Ok(ip) => ip == *connect,
                    Err(_) => {
                        log::error!(
                            target: "rule_engine",
                            "tried to convert '{}' to ipv4 because it is not a object, but conversion failed.",
                            object
                        );
                        false
                    }
                },
            },
        }
    }

    // TODO: the following function could be refactored as one.
    /// checks if the object exists and check if it matches against the helo value.
    pub fn __is_helo(helo: &str, object: &str) -> bool {
        match RHAI_ENGINE.objects.read().unwrap().get(object) {
            Some(object) => internal_is_helo(helo, object),
            _ => object == helo,
        }
    }

    /// checks if the object exists and check if it matches against the mail value.
    pub fn __is_mail(mail: &str, object: &str) -> bool {
        match RHAI_ENGINE.objects.read().unwrap().get(object) {
            Some(object) => internal_is_mail(mail, object),
            _ => object == mail,
        }
    }

    /// checks if the object exists and check if it matches against the rcpt value.
    pub fn __is_rcpt(rcpt: &str, object: &str) -> bool {
        match RHAI_ENGINE.objects.read().unwrap().get(object) {
            Some(object) => internal_is_rcpt(rcpt, object),
            _ => rcpt == object,
        }
    }

    /// checks if the given user exists on the system.
    pub fn __user_exists(object: &str) -> bool {
        match RHAI_ENGINE.objects.read().unwrap().get(object) {
            Some(object) => internal_user_exists(object),
            _ => internal_user_exists(&Object::Var(object.to_string())),
        }
    }
}

// NOTE: the following functions use pub(super) because they need to be exposed for tests.
// FIXME: find a way to hide the following function to the parent scope.
/// checks recursively if the current connect value is matching the object's value.
pub(super) fn internal_is_connect(connect: &IpAddr, object: &Object) -> bool {
    match object {
        Object::Ip4(ip) => *ip == *connect,
        Object::Ip6(ip) => *ip == *connect,
        Object::Rg4(range) => match connect {
            IpAddr::V4(ip4) => range.contains(ip4),
            _ => false,
        },
        Object::Rg6(range) => match connect {
            IpAddr::V6(ip6) => range.contains(ip6),
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
pub(super) fn internal_is_mail(mail: &str, object: &Object) -> bool {
    match object {
        Object::Address(addr) => *addr == mail,
        Object::Regex(re) => re.is_match(mail),
        Object::File(content) => content.iter().any(|object| internal_is_mail(mail, object)),
        Object::Group(group) => group.iter().any(|object| internal_is_mail(mail, object)),
        _ => false,
    }
}

/// checks recursively if the current rcpt value is matching the object's value.
pub(super) fn internal_is_rcpt(rcpt: &str, object: &Object) -> bool {
    match object {
        Object::Address(addr) => rcpt == addr.as_str(),
        Object::Regex(re) => re.is_match(rcpt),
        Object::File(content) => content.iter().any(|object| internal_is_rcpt(rcpt, object)),
        Object::Group(group) => group.iter().any(|object| internal_is_rcpt(rcpt, object)),
        _ => false,
    }
}

/// checks recursively if the/all user(s) exists on the system.
pub(super) fn internal_user_exists(user: &Object) -> bool {
    match user {
        Object::Var(user) => user_exists(user),
        Object::File(content) | Object::Group(content) => content.iter().all(internal_user_exists),
        _ => false,
    }
}

/// execute the id shell command, checking if the given user exists.
fn user_exists(user: &str) -> bool {
    match Command::new("sh")
        .args(["-c", &format!("id -u {}", user)])
        .status()
    {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}
