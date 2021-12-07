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
use super::io_service::{IoService, ReadError};
use crate::model::envelop::Envelop;
use crate::model::mail::{ConnectionData, MailContext};
use crate::resolver::DataEndResolver;
use crate::rules::rule_engine::{RuleEngine, Status};
use crate::server::TlsSecurityLevel;
use crate::smtp::code::SMTPReplyCode;
use crate::smtp::event::Event;

/// Abstracted memory of the last client message
#[derive(Debug, Eq, PartialEq, Hash)]
pub enum State {
    Connect,
    Helo,
    NegotiationTLS,
    MailFrom,
    RcptTo,
    Data,
    Stop,
}

impl std::str::FromStr for State {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "connect" => Ok(State::Connect),
            "helo" => Ok(State::Helo),
            "mail" => Ok(State::MailFrom),
            "rcpt" => Ok(State::RcptTo),
            "data" => Ok(State::Data),
            _ => Err("not a valid value"),
        }
    }
}

const MAIL_CAPACITY: usize = 10_000_000; // 10MB
const TIMEOUT_DEFAULT: u64 = 10_000; // 10s

lazy_static::lazy_static! {
    static ref NEXT_LINE_TIMEOUT: std::collections::HashMap<State, std::time::Duration> = {
         crate::config::get::<std::collections::HashMap<String,u64>>("smtp.timeout_client")
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(k, v)|
            <State as std::str::FromStr>::from_str(&k).ok().map(|s| (s, std::time::Duration::from_millis(v))))
            .collect::<std::collections::HashMap<State,std::time::Duration>>()
    };
}

pub struct MailReceiver<'a, R>
where
    R: DataEndResolver,
{
    /// state mutated by the client's commands and the rule engine.
    state: State,

    /// mail information sent by the client.
    mail: MailContext,

    /// rule engine executing the server's rhai configuration.
    rule_engine: RuleEngine<'a>,

    /// tsl metadata.
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    tls_security_level: TlsSecurityLevel,
    is_secured: bool,

    /// timeout configuration.
    next_line_timeout: std::time::Duration,
    _phantom: std::marker::PhantomData<R>,

    error_count: u64,
}

impl<R> MailReceiver<'_, R>
where
    R: DataEndResolver,
{
    pub fn new(
        peer_addr: std::net::SocketAddr,
        tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
        tls_security_level: TlsSecurityLevel,
    ) -> Self {
        if tls_security_level != TlsSecurityLevel::None && tls_config.is_none() {
            log::error!(
                target: "mail_receiver",
                "TLS encryption is enabled, but no TLS config provided. If a tls connection is ensured, the server will reply with \"{}\"", SMTPReplyCode::Code454.as_str(),
            );
        } else if tls_security_level == TlsSecurityLevel::None && tls_config.is_some() {
            log::error!(
                target: "mail_receiver",
                "TLS encryption is disabled, but a TLS config is provided. TLS config will be ignored",
            );
        }

        Self {
            state: State::Connect,
            rule_engine: RuleEngine::new(),
            mail: MailContext {
                connection: ConnectionData {
                    peer_addr,
                    timestamp: std::time::SystemTime::now(),
                },
                envelop: Envelop::default(),
                body: String::with_capacity(MAIL_CAPACITY),
                timestamp: None,
            },
            tls_config,
            tls_security_level,
            is_secured: false,
            next_line_timeout: *NEXT_LINE_TIMEOUT
                .get(&State::Connect)
                .unwrap_or(&std::time::Duration::from_millis(TIMEOUT_DEFAULT)),
            _phantom: std::marker::PhantomData,
            error_count: 0,
        }
    }

    fn set_helo(&mut self, helo: String) {
        self.mail.envelop = Envelop {
            helo,
            mail_from: String::new(),
            rcpt: vec![],
        };
        self.rule_engine
            .add_data("helo", self.mail.envelop.helo.clone());
    }

    fn set_mail_from(&mut self, mail_from: String) {
        self.mail.envelop.mail_from = mail_from;
        self.mail.timestamp = Some(std::time::SystemTime::now());
        self.mail.envelop.rcpt = vec![];

        self.rule_engine
            .add_data("mail", self.mail.envelop.mail_from.clone());
    }

    // NOTE: too many clone
    fn set_rcpt_to(&mut self, rcpt_to: String) {
        self.rule_engine.add_data("rcpt", rcpt_to.clone());

        match self.rule_engine.get_data::<Vec<String>>("rcpts") {
            Some(mut rcpts) => {
                rcpts.push(rcpt_to);
                self.mail.envelop.rcpt = rcpts.clone();
                self.rule_engine.add_data("rcpts", rcpts.clone());
            }
            None => unreachable!("rcpts is injected by the default scope"),
        };
    }

    async fn process_event(&mut self, event: Event) -> (Option<State>, Option<SMTPReplyCode>) {
        match (&self.state, event) {
            (_, Event::NoopCmd) => (None, Some(SMTPReplyCode::Code250)),

            (_, Event::HelpCmd(_)) => (None, Some(SMTPReplyCode::Code214)),

            (_, Event::RsetCmd) => {
                self.mail.body = String::with_capacity(MAIL_CAPACITY);
                self.mail.envelop.rcpt = vec![];
                self.mail.envelop.mail_from = String::new();

                (Some(State::Helo), Some(SMTPReplyCode::Code250))
            }

            (_, Event::ExpnCmd(_) | Event::VrfyCmd(_) | Event::PrivCmd) => {
                (None, Some(SMTPReplyCode::Code502))
            } // unimplemented

            (_, Event::QuitCmd) => (Some(State::Stop), Some(SMTPReplyCode::Code221)),

            (_, Event::HeloCmd(helo)) => {
                self.set_helo(helo);
                log::trace!(
                    target: "mail_receiver",
                    "[p:{}] envelop=\"{:?}\"",
                    self.mail.connection.peer_addr.port(), self.mail.envelop,
                );

                let status = self.rule_engine.run_when("helo");
                self.process_rules_status(status, Some(State::Helo), Some(SMTPReplyCode::Code250))
            }

            (_, Event::EhloCmd(helo)) => {
                self.set_helo(helo);
                log::trace!(
                    target: "mail_receiver",
                    "[p:{}] envelop=\"{:?}\"",
                    self.mail.connection.peer_addr.port(), self.mail.envelop,
                );

                let status = self.rule_engine.run_when("helo");
                self.process_rules_status(
                    status,
                    Some(State::Helo),
                    Some(if self.is_secured {
                        SMTPReplyCode::Code250SecuredEsmtp
                    } else {
                        SMTPReplyCode::Code250PlainEsmtp
                    }),
                )
            }

            (State::Helo, Event::StartTls) if self.tls_config.is_some() => {
                (Some(State::NegotiationTLS), Some(SMTPReplyCode::Code220))
            }

            (State::Helo, Event::StartTls) if self.tls_config.is_none() => {
                (None, Some(SMTPReplyCode::Code454))
            }

            (State::Helo, Event::MailCmd(_))
                if self.tls_security_level == TlsSecurityLevel::Encrypt && !self.is_secured =>
            {
                (None, Some(SMTPReplyCode::Code530))
            }

            (State::Helo, Event::MailCmd(mail_from)) => {
                self.mail.body = String::with_capacity(MAIL_CAPACITY);
                self.set_mail_from(mail_from);

                log::trace!(
                    target: "mail_receiver",
                    "[p:{}] envelop=\"{:?}\"",
                    self.mail.connection.peer_addr.port(), self.mail.envelop,
                );

                let status = self.rule_engine.run_when("mail");
                self.process_rules_status(
                    status,
                    Some(State::MailFrom),
                    Some(SMTPReplyCode::Code250),
                )
            }

            (State::MailFrom | State::RcptTo, Event::RcptCmd(rcpt_to)) => {
                self.set_rcpt_to(rcpt_to);

                log::trace!(
                    target: "mail_receiver",
                    "[p:{}] envelop=\"{:?}\"",
                    self.mail.connection.peer_addr.port(), self.mail.envelop,
                );

                let status = self.rule_engine.run_when("rcpt");
                self.process_rules_status(status, Some(State::RcptTo), Some(SMTPReplyCode::Code250))
            }

            (State::RcptTo, Event::DataCmd) => (Some(State::Data), Some(SMTPReplyCode::Code354)),

            (State::Data, Event::DataLine(line)) => {
                self.mail.body.push_str(&line);
                self.mail.body.push('\n');
                (None, None)
            }

            (State::Data, Event::DataEnd) => {
                let (state, code) = R::on_data_end(&self.mail).await;

                self.rule_engine.add_data("data", self.mail.body.clone());

                let status = self.rule_engine.run_when("preq");

                let result = match status {
                    Status::Block => (Some(State::Stop), Some(SMTPReplyCode::Code554)),
                    _ => self.process_rules_status(status, Some(state), Some(code)),
                };

                // executing all registered extensive operations.
                if let Err(error) = self.rule_engine.execute_operation_queue(
                    &self.mail,
                    &format!(
                        "{}_{:?}",
                        self.mail
                            .timestamp
                            .unwrap()
                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_millis(),
                        std::thread::current().id()
                    ),
                ) {
                    log::error!(target: "rule_engine", "failed to empty the operation queue: '{}'", error);
                }

                log::info!(
                    target: "mail_receiver",
                    "final envelop after executing all rules:\n {:#?}",
                    self.rule_engine.get_scoped_envelop()
                );

                // NOTE: clear envelop and mail context ?

                result
            }

            _ => (None, Some(SMTPReplyCode::Code503)),
        }
    }

    /// checks the result of the rule engine and returns the appropriate state and code.
    fn process_rules_status(
        &mut self,
        status: Status,
        desired_state: Option<State>,
        desired_code: Option<SMTPReplyCode>,
    ) -> (Option<State>, Option<SMTPReplyCode>) {
        match status {
            Status::Deny => (Some(State::Stop), Some(SMTPReplyCode::Code554)),
            _ => (desired_state, desired_code),
        }
    }

    /// handle a clear text received with plain_stream or tls_stream
    async fn handle_plain_text(&mut self, client_message: String) -> Option<SMTPReplyCode> {
        log::trace!(
            target: "mail_receiver",
            "[p:{}] buffer=\"{}\"",
            self.mail.connection.peer_addr.port(),
            client_message
        );

        let command_or_code = if self.state == State::Data {
            Event::parse_data
        } else {
            Event::parse_cmd
        }(&client_message);

        log::trace!(
            target: "mail_receiver",
            "[p:{}] parsed=\"{:?}\"",
            self.mail.connection.peer_addr.port(), command_or_code
        );

        let (new_state, reply) = match command_or_code {
            Ok(event) => self.process_event(event).await,
            Err(error) => (None, Some(error)),
        };

        if let Some(new_state) = new_state {
            log::warn!(
                target: "mail_receiver",
                "[p:{}] ================ STATE: /{:?}/ => /{:?}/",
                self.mail.connection.peer_addr.port(), self.state, new_state
            );
            self.state = new_state;
            let new_duration = *NEXT_LINE_TIMEOUT
                .get(&self.state)
                .unwrap_or(&std::time::Duration::from_millis(TIMEOUT_DEFAULT));
            self.next_line_timeout = new_duration;
        }

        reply
    }

    async fn read_and_handle<S>(&mut self, io: &mut IoService<'_, S>) -> Result<(), std::io::Error>
    where
        S: std::io::Write + std::io::Read,
    {
        match tokio::time::timeout(self.next_line_timeout, io.get_next_line_async()).await {
            Ok(Ok(client_message)) => {
                if let Some(response) = self.handle_plain_text(client_message).await {
                    log::warn!(
                        target: "mail_receiver",
                        "[p:{}] send=\"{:?}\"",
                        self.mail.connection.peer_addr.port(), response
                    );

                    if response.is_error() {
                        self.error_count += 1;

                        let hard_error =
                            crate::config::get::<i64>("smtp.error.hard_count").unwrap_or(-1);
                        let soft_error =
                            crate::config::get::<i64>("smtp.error.soft_count").unwrap_or(-1);

                        if hard_error != -1 && self.error_count >= hard_error as u64 {
                            let mut response_begin = response.as_str().to_string();
                            response_begin.replace_range(3..4, "-");
                            response_begin.push_str(SMTPReplyCode::Code451TooManyError.as_str());
                            std::io::Write::write_all(io, response_begin.as_bytes())?;

                            return Err(std::io::Error::new(
                                std::io::ErrorKind::ConnectionAborted,
                                "too many errors",
                            ));
                        }

                        std::io::Write::write_all(io, response.as_str().as_bytes())?;

                        if soft_error != -1 && self.error_count >= soft_error as u64 {
                            std::thread::sleep(std::time::Duration::from_millis(
                                crate::config::get::<u64>("smtp.error.delay").unwrap_or(100),
                            ));
                        }
                    } else {
                        std::io::Write::write_all(io, response.as_str().as_bytes())?;
                    }
                }
                Ok(())
            }
            Ok(Err(ReadError::Blocking)) => Ok(()),
            Ok(Err(ReadError::Eof)) => {
                log::warn!(
                    target: "mail_receiver", "[p:{}] (secured:{}) eof",
                    self.mail.connection.peer_addr.port(), self.is_secured
                );
                self.state = State::Stop;
                Ok(())
            }
            Ok(Err(ReadError::Other(e))) => {
                log::error!(
                    target: "mail_receiver", "[p:{}] (secured:{}) error {}",
                    self.mail.connection.peer_addr.port(), self.is_secured, e
                );
                self.state = State::Stop;
                Err(e)
            }
            Err(e) => {
                std::io::Write::write_all(io, SMTPReplyCode::Code451Timeout.as_str().as_bytes())?;
                Err(std::io::Error::new(std::io::ErrorKind::TimedOut, e))
            }
        }
    }

    fn complete_tls_handshake<S>(
        io: &mut IoService<rustls::Stream<rustls::ServerConnection, S>>,
    ) -> Result<(), std::io::Error>
    where
        S: std::io::Read + std::io::Write,
    {
        let begin_handshake = std::time::Instant::now();
        let duration = std::time::Duration::from_millis(
            crate::config::get::<u64>("tls.handshake_timeout_ms").unwrap_or(1000),
        );

        loop {
            if !io.inner.conn.is_handshaking() {
                break;
            }
            if begin_handshake.elapsed() > duration {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "too long",
                ));
            }
            match std::io::Write::flush(&mut io.inner) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    async fn receive_secured<S>(&mut self, mut plain_stream: S) -> Result<S, std::io::Error>
    where
        S: std::io::Read + std::io::Write,
    {
        let mut tls_connection =
            rustls::ServerConnection::new(self.tls_config.as_ref().unwrap().clone()).unwrap();

        let mut tls_stream: rustls::Stream<rustls::ServerConnection, S> =
            rustls::Stream::new(&mut tls_connection, &mut plain_stream);

        let mut io = IoService::new(&mut tls_stream);

        Self::complete_tls_handshake(&mut io)?;

        // TODO: rfc:
        // The decision of whether or not to believe the authenticity of the
        // other party in a TLS negotiation is a local matter.  However, some
        // general rules for the decisions are:
        //
        // -  A SMTP client would probably only want to authenticate an SMTP
        //    server whose server certificate has a domain name that is the
        //    domain name that the client thought it was connecting to.
        // -  A publicly-referenced  SMTP server would probably want to accept
        //    any verifiable certificate from an SMTP client, and would possibly
        //    want to put distinguishing information about the certificate in
        //    the Received header of messages that were relayed or submitted
        //    from the client.

        log::info!(
            target: "mail_receiver",
            "[p:{}] is_handshaking={}",
            self.mail.connection.peer_addr.port(), io.inner.conn.is_handshaking()
        );

        log::debug!(
            target: "mail_receiver",
            "[p:{}] protocol_version={:#?}\n alpn_protocol={:#?}\n negotiated_cipher_suite={:#?}\n peer_certificates={:#?}\n sni_hostname={:#?}",
            self.mail.connection.peer_addr.port(),
            io.inner.conn.protocol_version(),
            io.inner.conn.alpn_protocol(),
            io.inner.conn.negotiated_cipher_suite(),
            io.inner.conn.peer_certificates(),
            io.inner.conn.sni_hostname(),
        );

        log::warn!(
            target: "mail_receiver",
            "[p:{}] ================ STATE: /{:?}/ => /{:?}/",
            self.mail.connection.peer_addr.port(), self.state, State::Connect
        );

        self.mail.envelop = Envelop::default();
        self.mail.body = String::with_capacity(MAIL_CAPACITY);

        self.state = State::Connect;
        self.is_secured = true;
        self.next_line_timeout = *NEXT_LINE_TIMEOUT
            .get(&self.state)
            .unwrap_or(&std::time::Duration::from_millis(TIMEOUT_DEFAULT));

        while self.state != State::Stop {
            self.read_and_handle(&mut io).await?;
        }

        Ok(plain_stream)
    }

    pub async fn receive_plain<S>(&mut self, mut plain_stream: S) -> Result<S, std::io::Error>
    where
        S: std::io::Write + std::io::Read,
    {
        let mut io = IoService::new(&mut plain_stream);

        match std::io::Write::write_all(&mut io, SMTPReplyCode::Code220.as_str().as_bytes()) {
            Ok(_) => {}
            Err(e) => {
                log::error!(
                    target: "mail_receiver",
                    "Error on sending response (receiving); error = {:?}", e
                );
                return Err(e);
            }
        }

        self.rule_engine
            .add_data("connect", self.mail.connection.peer_addr.ip());
        // self.rule_engine.add_data("msg_id", self.msg_id.clone());

        if let Status::Deny = self.rule_engine.run_when("connect") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "connection at '{}' has been denied when connecting.",
                    self.mail.connection.peer_addr
                ),
            ));
        };

        while self.state != State::Stop {
            if self.state == State::NegotiationTLS {
                return self.receive_secured(plain_stream).await;
            }

            self.read_and_handle(&mut io).await?;
        }
        Ok(plain_stream)
    }
}
