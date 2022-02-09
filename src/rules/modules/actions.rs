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
use rhai::plugin::*;

// exported methods are used in rhai context, so we allow dead code.
#[allow(dead_code)]
#[export_module]
pub mod actions {

    use crate::{config::log_channel::RULES, rules::rule_engine::Status, smtp::mail::MailContext};

    // #[rhai_fn(name = "__SHELL", return_raw)]
    // pub fn shell(command: &str) -> Result<std::process::Output, Box<EvalAltResult>> {
    //     std::process::Command::new("sh")
    //         .arg("-c")
    //         .arg(command)
    //         .output()
    //         .map_err(|e| e.to_string().into())
    // }

    // /// enqueue a block operation on the queue.
    // pub fn op_block(queue: &mut OperationQueue, path: &str) {
    //     queue.enqueue(Operation::Block(path.to_string()))
    // }

    // /// enqueue a quarantine operation on the queue.
    // pub fn op_quarantine(queue: &mut OperationQueue, reason: String) {
    //     queue.enqueue(Operation::Quarantine { reason })
    // }

    // /// enqueue a header mutation operation on the queue.
    // pub fn op_mutate_header(queue: &mut OperationQueue, header: &str, value: &str) {
    //     queue.enqueue(Operation::MutateHeader(
    //         header.to_string(),
    //         value.to_string(),
    //     ))
    // }

    #[rhai_fn(name = "FACCEPT")]
    pub fn faccept() -> Status {
        Status::Faccept
    }

    #[rhai_fn(name = "ACCEPT")]
    pub fn accept() -> Status {
        Status::Accept
    }

    #[rhai_fn(name = "CONTINUE")]
    pub fn ct() -> Status {
        Status::Continue
    }

    #[rhai_fn(name = "DENY")]
    pub fn deny() -> Status {
        Status::Deny
    }

    #[rhai_fn(name = "BLOCK")]
    pub fn block() -> Status {
        Status::Block
    }

    /// logs a message to stdout, stderr or a file.
    #[rhai_fn(name = "LOG", return_raw)]
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
                match std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                {
                    Ok(file) => {
                        let mut writer = std::io::LineWriter::new(file);

                        std::io::Write::write_all(&mut writer, format!("{message}\n").as_bytes())
                            .map_err::<Box<EvalAltResult>, _>(|err| {
                                format!("LOG action error: {err:?}").into()
                            })
                    }
                    Err(err) => Err(format!("LOG action error: {err:?}",).into()),
                }
            }
        }
    }

    /// logs a message to stdout.
    #[rhai_fn(name = "LOG_OUT", return_raw)]
    pub fn log_out(message: &str) -> Result<(), Box<EvalAltResult>> {
        log(message, "stdout")
    }

    /// logs a message to stderr.
    #[rhai_fn(name = "LOG_ERR", return_raw)]
    pub fn log_err(message: &str) -> Result<(), Box<EvalAltResult>> {
        log(message, "stderr")
    }

    // // NOTE: instead of filling the email using arguments, should we create a 'mail' object
    // //       defined beforehand in the user's object files ?
    // /// (WARNING: NOT YET FUNCTIONAL)
    // /// sends a mail.
    // /// the body can be formatted using html.
    // #[rhai_fn(name = "__MAIL", return_raw)]
    // pub fn send_mail(
    //     from: &str,
    //     to: &str,
    //     subject: &str,
    //     body: &str,
    // ) -> Result<(), Box<EvalAltResult>> {
    //     let email = Message::builder()
    //         .from(from.parse().unwrap())
    //         .to(to.parse().unwrap())
    //         .subject(subject)
    //         .body(String::from(body))
    //         .unwrap();

    //     // TODO: replace unencrypted_localhost by a valid host.
    //     // NOTE: unscripted_localhost is used for test purposes.
    //     match SmtpTransport::unencrypted_localhost().send(&email) {
    //         Ok(_) => Ok(()),
    //         Err(error) => Err(EvalAltResult::ErrorInFunctionCall(
    //             "MAIL".to_string(),
    //             "__MAIL".to_string(),
    //             format!("Couldn't send the email: {}", error).into(),
    //             Position::NONE,
    //         )
    //         .into()),
    //     }
    // }

    // #[rhai_fn(name = "__LOOKUP_MAIL_FROM", return_raw)]
    // /// check the client's ip matches against the hostname passed has parameter.
    // /// this can be used, for example, to check if MAIL FROM's value
    // /// is matching the connection, preventing relaying.
    // pub fn lookup_mail_from(
    //     // curried parameters.
    //     connect: std::net::IpAddr,
    //     port: u16,
    //     // exposed parameter.
    //     hostname: &str,
    // ) -> Result<bool, Box<EvalAltResult>> {
    //     if hostname.is_empty() {
    //         return Err(
    //             "the LOOKUP_MAIL_FROM action can only be called after or in the 'mail' stage."
    //                 .into(),
    //         );
    //     }

    //     let engine = acquire_engine();
    //     let objects = engine.objects.read().unwrap();

    //     let hostname = match objects.get(hostname) {
    //         Some(Object::Fqdn(fqdn)) => fqdn.as_str(),
    //         _ => hostname,
    //     };

    //     Ok(format!("{}:{}", hostname, port)
    //         .to_socket_addrs()
    //         .map_err::<Box<EvalAltResult>, _>(|error| {
    //             format!("couldn't process dns lookup: {}", error).into()
    //         })?
    //         .any(|socket| socket.ip() == connect))
    // }

    #[rhai_fn(name = "==")]
    pub fn eq_status_operator(in1: &mut Status, in2: Status) -> bool {
        *in1 == in2
    }

    #[rhai_fn(name = "!=")]
    pub fn neq_status_operator(in1: &mut Status, in2: Status) -> bool {
        !(*in1 == in2)
    }
}
