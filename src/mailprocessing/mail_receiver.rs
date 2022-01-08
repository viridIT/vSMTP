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
use super::state::StateSMTP;
use crate::config::log::{RECEIVER, RULES};
use crate::config::server_config::{ServerConfig, TlsSecurityLevel};
use crate::model::envelop::Envelop;
use crate::model::mail::{ConnectionData, MailContext, MessageMetadata};
use crate::resolver::DataEndResolver;
use crate::rules::address::Address;
use crate::rules::rule_engine::{RuleEngine, Status};
use crate::smtp::code::SMTPReplyCode;
use crate::smtp::event::Event;

const MAIL_CAPACITY: usize = 10_000_000; // 10MB

// TODO: move that cleanly in config
const TIMEOUT_DEFAULT: u64 = 10_000; // 10s

enum ProcessedEvent {
    Nothing,
    Reply(SMTPReplyCode),
    ReplyChangeState(StateSMTP, SMTPReplyCode),
    TransactionCompleted(Box<MailContext>),
}

pub struct MailReceiver<'a, R>
where
    R: DataEndResolver,
{
    /// config
    server_config: std::sync::Arc<ServerConfig>,
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    smtp_timeouts: std::collections::HashMap<StateSMTP, std::time::Duration>,

    /// rule engine executing the server's rhai configuration.
    rule_engine: RuleEngine<'a>,

    /// Current connection data
    state: StateSMTP,
    mail: MailContext,
    error_count: u64,
    is_secured: bool,

    /// cached state // TODO: move that cleanly in config
    next_line_timeout: std::time::Duration,

    _phantom: std::marker::PhantomData<R>,
}

impl<R> MailReceiver<'_, R>
where
    R: DataEndResolver,
{
    pub fn new(
        peer_addr: std::net::SocketAddr,
        tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
        server_config: std::sync::Arc<ServerConfig>,
    ) -> Self {
        // TODO: move that cleanly in config
        let smtp_timeouts = server_config
            .smtp
            .timeout_client
            .iter()
            .filter_map(|(k, v)| match humantime::parse_duration(v) {
                Ok(v) => Some((*k, v)),
                Err(e) => {
                    log::error!(
                        target: RECEIVER,
                        "error \"{}\" parsing timeout for key={}, ignored",
                        e,
                        k
                    );
                    None
                }
            })
            .collect::<std::collections::HashMap<_, _>>();

        Self {
            state: StateSMTP::Connect,
            rule_engine: RuleEngine::new(server_config.as_ref()),
            mail: MailContext {
                connection: ConnectionData {
                    peer_addr,
                    timestamp: std::time::SystemTime::now(),
                },
                envelop: Envelop::default(),
                body: String::with_capacity(MAIL_CAPACITY),
                metadata: None,
            },
            tls_config,
            is_secured: false,
            next_line_timeout: *smtp_timeouts
                .get(&StateSMTP::Connect)
                .unwrap_or(&std::time::Duration::from_millis(TIMEOUT_DEFAULT)),
            smtp_timeouts,
            server_config,
            error_count: 0,
            _phantom: std::marker::PhantomData,
        }
    }

    fn set_helo(&mut self, helo: String) {
        self.mail.envelop = Envelop {
            helo,
            mail_from: Address::default(),
            rcpt: std::collections::HashSet::default(),
        };
        self.rule_engine.reset();

        self.rule_engine
            .add_data("helo", self.mail.envelop.helo.clone());
    }

    fn set_mail_from(&mut self, mail_from: String) {
        if let Ok(mail_from) = Address::new(&mail_from) {
            self.mail.envelop.mail_from = mail_from;
            self.mail.envelop.rcpt.clear();
            self.rule_engine.reset();

            let now = std::time::SystemTime::now();

            // generating email metadata.
            self.mail.metadata = Some(MessageMetadata {
                timestamp: now,
                // TODO: find a way to handle SystemTime failure.
                message_id: format!(
                    "{}{}{}",
                    now.duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .unwrap_or(std::time::Duration::ZERO)
                        .as_micros(),
                    self.mail
                        .connection
                        .timestamp
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .unwrap_or(std::time::Duration::ZERO)
                        .as_millis(),
                    std::process::id()
                ),
                retry: 0,
            });

            self.rule_engine
                .add_data("mail", self.mail.envelop.mail_from.clone());
            self.rule_engine
                .add_data("metadata", self.mail.metadata.clone());
        }
    }

    // FIXME: too many clone
    fn set_rcpt_to(&mut self, rcpt_to: String) {
        if let Ok(rcpt_to) = Address::new(&rcpt_to) {
            self.rule_engine.add_data("rcpt", rcpt_to.clone());

            match self
                .rule_engine
                .get_data::<std::collections::HashSet<Address>>("rcpts")
            {
                Some(mut rcpts) => {
                    rcpts.insert(rcpt_to);
                    self.mail.envelop.rcpt = rcpts.clone();
                    self.rule_engine.add_data("rcpts", rcpts.clone());
                }
                None => unreachable!("rcpts is injected by the default scope"),
            };
        } else {
            log::error!(target: RECEIVER, "rcpt's email address is invalid.");
        }
    }

    fn process_event(&mut self, event: Event) -> ProcessedEvent {
        match (&self.state, event) {
            (_, Event::NoopCmd) => ProcessedEvent::Reply(SMTPReplyCode::Code250),

            (_, Event::HelpCmd(_)) => ProcessedEvent::Reply(SMTPReplyCode::Code214),

            (_, Event::RsetCmd) => {
                self.mail.body = String::with_capacity(MAIL_CAPACITY);
                self.mail.envelop.rcpt.clear();
                self.mail.envelop.mail_from = Address::default();
                self.rule_engine.reset();

                ProcessedEvent::ReplyChangeState(StateSMTP::Helo, SMTPReplyCode::Code250)
            }

            (_, Event::ExpnCmd(_) | Event::VrfyCmd(_) /*| Event::PrivCmd*/) => {
                ProcessedEvent::Reply(SMTPReplyCode::Code502unimplemented)
            }

            (_, Event::QuitCmd) => {
                ProcessedEvent::ReplyChangeState(StateSMTP::Stop, SMTPReplyCode::Code221)
            }

            (_, Event::HeloCmd(helo)) => {
                self.set_helo(helo);
                log::trace!(
                    target: RECEIVER,
                    "[p:{}] envelop=\"{:?}\"",
                    self.mail.connection.peer_addr.port(),
                    self.mail.envelop,
                );

                match self.rule_engine.run_when("helo") {
                    Status::Deny => {
                        ProcessedEvent::ReplyChangeState(StateSMTP::Stop, SMTPReplyCode::Code554)
                    }
                    _ => ProcessedEvent::ReplyChangeState(StateSMTP::Helo, SMTPReplyCode::Code250),
                }
            }

            (_, Event::EhloCmd(_)) if self.server_config.smtp.disable_ehlo => {
                ProcessedEvent::Reply(SMTPReplyCode::Code502unimplemented)
            }

            (_, Event::EhloCmd(helo)) => {
                self.set_helo(helo);
                log::trace!(
                    target: RECEIVER,
                    "[p:{}] envelop=\"{:?}\"",
                    self.mail.connection.peer_addr.port(),
                    self.mail.envelop,
                );

                match self.rule_engine.run_when("helo") {
                    Status::Deny => {
                        ProcessedEvent::ReplyChangeState(StateSMTP::Stop, SMTPReplyCode::Code554)
                    }
                    _ => ProcessedEvent::ReplyChangeState(
                        StateSMTP::Helo,
                        if self.is_secured {
                            SMTPReplyCode::Code250SecuredEsmtp
                        } else {
                            SMTPReplyCode::Code250PlainEsmtp
                        },
                    ),
                }
            }

            (StateSMTP::Helo, Event::StartTls) if self.tls_config.is_some() => {
                ProcessedEvent::ReplyChangeState(StateSMTP::NegotiationTLS, SMTPReplyCode::Code220)
            }
            (StateSMTP::Helo, Event::StartTls) if self.tls_config.is_none() => {
                ProcessedEvent::Reply(SMTPReplyCode::Code454)
            }

            (StateSMTP::Helo, Event::MailCmd(_, _))
                if self.server_config.tls.security_level == TlsSecurityLevel::Encrypt
                    && !self.is_secured =>
            {
                ProcessedEvent::Reply(SMTPReplyCode::Code530)
            }

            (StateSMTP::Helo, Event::MailCmd(mail_from, _body_bit_mime)) => {
                // TODO: store in envelop _body_bit_mime

                self.mail.body = String::with_capacity(MAIL_CAPACITY);
                self.set_mail_from(mail_from);

                log::trace!(
                    target: RECEIVER,
                    "[p:{}] envelop=\"{:?}\"",
                    self.mail.connection.peer_addr.port(),
                    self.mail.envelop,
                );

                match self.rule_engine.run_when("mail") {
                    Status::Deny => {
                        ProcessedEvent::ReplyChangeState(StateSMTP::Stop, SMTPReplyCode::Code554)
                    }
                    _ => ProcessedEvent::ReplyChangeState(
                        StateSMTP::MailFrom,
                        SMTPReplyCode::Code250,
                    ),
                }
            }

            (StateSMTP::MailFrom | StateSMTP::RcptTo, Event::RcptCmd(rcpt_to)) => {
                self.set_rcpt_to(rcpt_to);

                log::trace!(
                    target: RECEIVER,
                    "[p:{}] envelop=\"{:?}\"",
                    self.mail.connection.peer_addr.port(),
                    self.mail.envelop,
                );

                match self.rule_engine.run_when("rcpt") {
                    Status::Deny => {
                        ProcessedEvent::ReplyChangeState(StateSMTP::Stop, SMTPReplyCode::Code554)
                    }
                    _ if self.mail.envelop.rcpt.len()
                        >= self.server_config.smtp.rcpt_count_max.unwrap_or(usize::MAX) =>
                    {
                        ProcessedEvent::ReplyChangeState(
                            StateSMTP::RcptTo,
                            SMTPReplyCode::Code452TooManyRecipients,
                        )
                    }
                    _ => {
                        ProcessedEvent::ReplyChangeState(StateSMTP::RcptTo, SMTPReplyCode::Code250)
                    }
                }
            }

            (StateSMTP::RcptTo, Event::DataCmd) => {
                ProcessedEvent::ReplyChangeState(StateSMTP::Data, SMTPReplyCode::Code354)
            }

            (StateSMTP::Data, Event::DataLine(line)) => {
                self.mail.body.push_str(&line);
                self.mail.body.push('\n');
                ProcessedEvent::Nothing
            }

            (StateSMTP::Data, Event::DataEnd) => {
                self.rule_engine.add_data("data", self.mail.body.clone());

                let status = self.rule_engine.run_when("preq");

                if let Status::Block | Status::Deny = status {
                    return ProcessedEvent::ReplyChangeState(
                        StateSMTP::Stop,
                        SMTPReplyCode::Code554,
                    );
                }

                // executing all registered extensive operations.
                if let Err(error) = self.rule_engine.execute_operation_queue(&self.mail) {
                    log::error!(
                        target: RULES,
                        "failed to empty the operation queue: '{}'",
                        error
                    );
                }

                // getting the server's envelop, that could have mutated in the
                // rule engine.
                if let Some(envelop) = self.rule_engine.get_scoped_envelop() {
                    self.mail.envelop = envelop;

                    let mut output = MailContext {
                        envelop: Envelop::default(),
                        body: String::with_capacity(MAIL_CAPACITY),
                        connection: self.mail.connection,
                        metadata: None,
                    };

                    std::mem::swap(&mut self.mail, &mut output);

                    ProcessedEvent::TransactionCompleted(Box::new(output))
                } else {
                    ProcessedEvent::ReplyChangeState(StateSMTP::MailFrom, SMTPReplyCode::Code554)
                }
            }

            _ => ProcessedEvent::Reply(SMTPReplyCode::Code503),
        }
    }

    pub fn set_state(&mut self, new_state: StateSMTP) {
        log::info!(
            target: RECEIVER,
            "[p:{}] ================ STATE: /{:?}/ => /{:?}/",
            self.mail.connection.peer_addr.port(),
            self.state,
            new_state
        );
        self.state = new_state;
        self.next_line_timeout = *self
            .smtp_timeouts
            .get(&self.state)
            .unwrap_or(&std::time::Duration::from_millis(TIMEOUT_DEFAULT));
    }

    /// handle a clear text received with plain_stream or tls_stream
    fn handle_plain_text(&mut self, client_message: String) -> ProcessedEvent {
        log::trace!(
            target: RECEIVER,
            "[p:{}] buffer=\"{}\"",
            self.mail.connection.peer_addr.port(),
            client_message
        );

        let command_or_code = if self.state == StateSMTP::Data {
            Event::parse_data
        } else {
            Event::parse_cmd
        }(&client_message);

        log::trace!(
            target: RECEIVER,
            "[p:{}] parsed=\"{:?}\"",
            self.mail.connection.peer_addr.port(),
            command_or_code
        );

        match command_or_code
            .map(|command| self.process_event(command))
            .unwrap_or_else(ProcessedEvent::Reply)
        {
            ProcessedEvent::ReplyChangeState(new_state, reply) => {
                self.set_state(new_state);
                ProcessedEvent::Reply(reply)
            }
            otherwise => otherwise,
        }
    }

    pub fn send_reply<IO>(
        &mut self,
        io: &mut IoService<'_, IO>,
        reply_to_send: SMTPReplyCode,
    ) -> Result<(), std::io::Error>
    where
        IO: std::io::Read + std::io::Write,
    {
        log::info!(
            target: RECEIVER,
            "[p:{}] send=\"{:?}\"",
            self.mail.connection.peer_addr.port(),
            reply_to_send
        );

        if reply_to_send.is_error() {
            self.error_count += 1;

            let hard_error = self.server_config.smtp.error.hard_count;
            let soft_error = self.server_config.smtp.error.soft_count;

            if hard_error != -1 && self.error_count >= hard_error as u64 {
                let mut response_begin = self
                    .server_config
                    .smtp
                    .get_code()
                    .get(&reply_to_send)
                    .to_string();
                response_begin.replace_range(3..4, "-");
                response_begin.push_str(
                    self.server_config
                        .smtp
                        .get_code()
                        .get(&SMTPReplyCode::Code451TooManyError),
                );
                std::io::Write::write_all(io, response_begin.as_bytes())?;

                return Err(std::io::Error::new(
                    std::io::ErrorKind::ConnectionAborted,
                    "too many errors",
                ));
            }

            std::io::Write::write_all(
                io,
                self.server_config
                    .smtp
                    .get_code()
                    .get(&reply_to_send)
                    .as_bytes(),
            )?;

            if soft_error != -1 && self.error_count >= soft_error as u64 {
                std::thread::sleep(self.server_config.smtp.error.delay);
            }

            Ok(())
        } else {
            std::io::Write::write_all(
                io,
                self.server_config
                    .smtp
                    .get_code()
                    .get(&reply_to_send)
                    .as_bytes(),
            )
        }
    }

    async fn read_and_handle<S>(
        &mut self,
        io: &mut IoService<'_, S>,
    ) -> Result<Option<MailContext>, std::io::Error>
    where
        S: std::io::Write + std::io::Read,
    {
        match tokio::time::timeout(self.next_line_timeout, io.get_next_line_async()).await {
            Ok(Ok(client_message)) => match self.handle_plain_text(client_message) {
                ProcessedEvent::Reply(reply_to_send) => {
                    self.send_reply(io, reply_to_send).map(|_| None)
                }
                ProcessedEvent::TransactionCompleted(mail) => Ok(Some(*mail)),
                ProcessedEvent::Nothing => Ok(None),
                ProcessedEvent::ReplyChangeState(_, _) => unreachable!(),
            },
            Ok(Err(ReadError::Blocking)) => Ok(None),
            Ok(Err(ReadError::Eof)) => {
                log::info!(
                    target: RECEIVER,
                    "[p:{}] (secured:{}) eof",
                    self.mail.connection.peer_addr.port(),
                    self.is_secured
                );
                self.state = StateSMTP::Stop;
                Ok(None)
            }
            Ok(Err(ReadError::Other(e))) => {
                log::error!(
                    target: RECEIVER,
                    "[p:{}] (secured:{}) error {}",
                    self.mail.connection.peer_addr.port(),
                    self.is_secured,
                    e
                );
                self.state = StateSMTP::Stop;
                Err(e)
            }
            Err(e) => {
                std::io::Write::write_all(
                    io,
                    self.server_config
                        .smtp
                        .get_code()
                        .get(&SMTPReplyCode::Code451Timeout)
                        .as_bytes(),
                )?;
                Err(std::io::Error::new(std::io::ErrorKind::TimedOut, e))
            }
        }
    }

    fn complete_tls_handshake<S>(
        io: &mut IoService<rustls::Stream<rustls::ServerConnection, S>>,
        timeout: &std::time::Duration,
    ) -> Result<(), std::io::Error>
    where
        S: std::io::Read + std::io::Write,
    {
        let begin_handshake = std::time::Instant::now();

        while io.inner.conn.is_handshaking() {
            if begin_handshake.elapsed() > *timeout {
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
        let mut tls_connection = rustls::ServerConnection::new(
            self.tls_config
                .as_ref()
                .expect("Failed to get tsl_config")
                .clone(),
        )
        .expect("Failed to open tsl connection");

        let mut tls_stream: rustls::Stream<rustls::ServerConnection, S> =
            rustls::Stream::new(&mut tls_connection, &mut plain_stream);

        let mut io = IoService::new(&mut tls_stream);

        Self::complete_tls_handshake(&mut io, &self.server_config.tls.handshake_timeout)?;

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

        log::debug!(
            target: RECEIVER,
            "[p:{}] protocol_version={:#?}\n alpn_protocol={:#?}\n negotiated_cipher_suite={:#?}\n peer_certificates={:#?}\n sni_hostname={:#?}",
            self.mail.connection.peer_addr.port(),
            io.inner.conn.protocol_version(),
            io.inner.conn.alpn_protocol(),
            io.inner.conn.negotiated_cipher_suite(),
            io.inner.conn.peer_certificates(),
            io.inner.conn.sni_hostname(),
        );

        log::warn!(
            target: RECEIVER,
            "[p:{}] ================ STATE: /{:?}/ => /{:?}/",
            self.mail.connection.peer_addr.port(),
            self.state,
            StateSMTP::Connect
        );

        self.mail.envelop = Envelop::default();
        self.mail.body = String::with_capacity(MAIL_CAPACITY);

        self.state = StateSMTP::Connect;
        self.is_secured = true;
        self.next_line_timeout = *self
            .smtp_timeouts
            .get(&self.state)
            .unwrap_or(&std::time::Duration::from_millis(TIMEOUT_DEFAULT));

        while self.state != StateSMTP::Stop {
            if let Some(mail) = self.read_and_handle(&mut io).await? {
                let code = R::on_data_end(&self.server_config, &mail).await?;
                self.send_reply(&mut io, code)?;
                self.set_state(StateSMTP::Helo);
            }
        }

        Ok(plain_stream)
    }

    pub async fn receive_plain<S>(&mut self, mut plain_stream: S) -> Result<S, std::io::Error>
    where
        S: std::io::Write + std::io::Read,
    {
        let mut io = IoService::new(&mut plain_stream);

        self.send_reply(&mut io, SMTPReplyCode::Code220)?;

        self.rule_engine
            .add_data("connect", self.mail.connection.peer_addr.ip());
        self.rule_engine
            .add_data("port", self.mail.connection.peer_addr.port());
        self.rule_engine
            .add_data("connection_timestamp", self.mail.connection.timestamp);

        if let Status::Deny = self.rule_engine.run_when("connect") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "connection at '{}' has been denied when connecting.",
                    self.mail.connection.peer_addr
                ),
            ));
        };

        while self.state != StateSMTP::Stop {
            if self.state == StateSMTP::NegotiationTLS {
                return self.receive_secured(plain_stream).await;
            }
            if let Some(mail) = self.read_and_handle(&mut io).await? {
                let code = R::on_data_end(&self.server_config, &mail).await?;
                self.send_reply(&mut io, code)?;
                self.set_state(StateSMTP::Helo);
            }
        }
        Ok(plain_stream)
    }
}
