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

    // /// logs a message to stdout, stderr or a file.
    // #[rhai_fn(name = "__LOG", return_raw)]
    // pub fn log(message: &str, path: &str) -> Result<(), Box<EvalAltResult>> {
    //     match path {
    //         "stdout" => {
    //             println!("{}", message);
    //             Ok(())
    //         }
    //         "stderr" => {
    //             eprintln!("{}", message);
    //             Ok(())
    //         }
    //         _ => {
    //             // the only writer on "objects" is called and unlocked
    //             // at the start of the server, we can unwrap here.
    //             let path = match acquire_engine().objects.read().unwrap().get(path) {
    //                 // from_str is infallible, we can unwrap.
    //                 Some(Object::Var(p)) => {
    //                     <std::path::PathBuf as std::str::FromStr>::from_str(p.as_str()).unwrap()
    //                 }
    //                 _ => <std::path::PathBuf as std::str::FromStr>::from_str(path).unwrap(),
    //             };

    //             match std::fs::OpenOptions::new()
    //                 .create(true)
    //                 .append(true)
    //                 .open(&path)
    //             {
    //                 Ok(file) => {
    //                     let mut writer = std::io::LineWriter::new(file);

    //                     std::io::Write::write_all(&mut writer, message.as_bytes()).map_err::<Box<
    //                         EvalAltResult,
    //                     >, _>(
    //                         |_| format!("could not log to '{:?}'.", path).into(),
    //                     )?;
    //                     std::io::Write::write_all(&mut writer, b"\n")
    //                         .map_err(|_| format!("could not log to '{:?}'.", path).into())
    //                 }
    //                 Err(error) => Err(format!(
    //                     "'{:?}' is not a valid path to log to: {:#?}",
    //                     path, error
    //                 )
    //                 .into()),
    //             }
    //         }
    //     }
    // }

    // // NOTE: this function needs to be curried to access data,
    // //       could it be added to the operation queue ?
    // /// write the email to a specified file.
    // #[rhai_fn(name = "__WRITE", return_raw)]
    // pub fn write_mail(data: Mail, path: &str) -> Result<(), Box<EvalAltResult>> {
    //     if data.headers.is_empty() {
    //         return Err("the WRITE action can only be called after or in the 'preq' stage.".into());
    //     }

    //     // from_str is infallible, we can unwrap.
    //     let path = <std::path::PathBuf as std::str::FromStr>::from_str(path).unwrap();

    //     match std::fs::OpenOptions::new()
    //         .create(true)
    //         .append(true)
    //         .open(&path)
    //     {
    //         Ok(mut file) => {
    //             let (headers, body) = data.to_raw();
    //             std::io::Write::write_all(&mut file, format!("{}\n{}", headers, body).as_bytes())
    //                 .map_err(|_| format!("could not write email to '{:?}'.", path).into())
    //         }
    //         Err(error) => Err(format!(
    //             "'{:?}' is not a valid path to write the email to: {:#?}",
    //             path, error
    //         )
    //         .into()),
    //     }
    // }

    // /// dumps the content of the current connection in a json file.
    // /// if some data is missing because of the current stage, it will
    // /// be blank in the json representation.
    // /// for example, dumping during the rcpt stage will leave the data
    // /// field empty.
    // #[rhai_fn(name = "__DUMP", return_raw)]
    // pub fn dump(ctx: &mut MailContext, path: &str) -> Result<(), Box<EvalAltResult>> {
    //     if let Err(error) = std::fs::create_dir_all(path) {
    //         return Err(format!("could not write email to '{:?}': {}", path, error).into());
    //     }

    //     let mut file = match std::fs::OpenOptions::new().write(true).create(true).open({
    //         // Error is of type Infallible, we can unwrap.
    //         let mut path = <std::path::PathBuf as std::str::FromStr>::from_str(path).unwrap();
    //         path.push(
    //             ctx.metadata
    //                 .as_ref()
    //                 .ok_or_else::<Box<EvalAltResult>, _>(|| {
    //                     "could not dump email, metadata has not been received yet.".into()
    //                 })?
    //                 .message_id
    //                 .clone(),
    //         );
    //         path.set_extension("json");
    //         path
    //     }) {
    //         Ok(file) => file,
    //         Err(error) => {
    //             return Err(format!("could not write email to '{:?}': {}", path, error).into())
    //         }
    //     };

    //     std::io::Write::write_all(&mut file, serde_json::to_string(&ctx).unwrap().as_bytes())
    //         .map_err(|error| format!("could not write email to '{:?}': {}", path, error).into())
    // }

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
