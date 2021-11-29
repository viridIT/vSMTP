use std::net::Ipv4Addr;
use std::str::FromStr;

use super::reader::{MyConnectionIO, ReadError};
use crate::model::envelop::Envelop;
use crate::model::mail::MailContext;
use crate::resolver::DataEndResolver;
use crate::rules::rule_engine::{RuleEngine, Status};
use crate::server::TlsSecurityLevel;
use crate::smtp::code::SMTPReplyCode;
use crate::smtp::event::Event;

/// Abstracted memory of the last client message
#[derive(Debug, PartialEq)]
pub enum State {
    Connect,
    Helo,
    NegotiationTLS,
    MailFrom,
    RcptTo,
    Data,
    Stop,
}

pub struct MailReceiver<'a, R>
where
    R: DataEndResolver,
{
    ip: String,
    port: u16,
    state: State,
    mail: MailContext,
    rule_engine: RuleEngine<'a>,
    force_accept: bool,
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    tls_security_level: TlsSecurityLevel,
    is_secured: bool,
    _phantom: std::marker::PhantomData<R>,
}

impl<R> MailReceiver<'_, R>
where
    R: DataEndResolver,
{
    pub fn new(
        addr: &std::net::SocketAddr,
        tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
        tls_security_level: TlsSecurityLevel,
    ) -> Self {
        Self {
            ip: addr.ip().to_string(),
            port: addr.port(),
            state: State::Connect,
            rule_engine: RuleEngine::new(),
            force_accept: false,
            mail: MailContext {
                envelop: Envelop::default(),
                body: Vec::with_capacity(20_000),
            },
            tls_config,
            tls_security_level,
            is_secured: false,
            _phantom: std::marker::PhantomData,
        }
    }

    fn reset(&mut self) {
        self.mail = MailContext {
            envelop: Envelop::default(),
            body: Vec::with_capacity(20_000),
        };
    }

    async fn process_event(&mut self, event: Event) -> (Option<State>, Option<SMTPReplyCode>) {
        match (&self.state, event) {
            (_, Event::NoopCmd) => (None, Some(SMTPReplyCode::Code250)),

            (_, Event::HelpCmd(_)) => (None, Some(SMTPReplyCode::Code214)),

            (_, Event::RsetCmd) => {
                self.mail.body = Vec::with_capacity(20_000);
                // NOTE: clear envelop but keep helo
                self.mail.envelop.recipients = vec![];
                self.mail.envelop.mail_from = String::new();

                (Some(State::Helo), Some(SMTPReplyCode::Code250))
            }

            (_, Event::ExpnCmd(_) | Event::VrfyCmd(_) | Event::PrivCmd) => {
                (None, Some(SMTPReplyCode::Code502))
            } // unimplemented

            (_, Event::QuitCmd) => (Some(State::Stop), Some(SMTPReplyCode::Code221)),

            // A mail transaction may be aborted by a new EHLO command.
            // TODO: clear envelop ?
            (_, Event::HeloCmd(helo)) => {
                self.mail.envelop.helo = helo.clone();
                log::trace!(
                    target: "mail_receiver",
                    "[p:{}] envelop=\"{:?}\"",
                    self.port, self.mail.envelop,
                );

                self.rule_engine.add_data("helo", helo);

                if self.force_accept {
                    (Some(State::Helo), Some(SMTPReplyCode::Code250))
                } else {
                    let status = self.rule_engine.run_when("helo");
                    self.process_rules_status(
                        status,
                        Some(State::Helo),
                        Some(SMTPReplyCode::Code250),
                    )
                }
            }

            (_, Event::EhloCmd(helo)) => {
                self.mail.envelop.helo = helo.clone();
                log::trace!(
                    target: "mail_receiver",
                    "[p:{}] envelop=\"{:?}\"",
                    self.port, self.mail.envelop,
                );
                self.rule_engine.add_data("helo", helo);

                let reply_code = if self.is_secured {
                    SMTPReplyCode::Code250SecuredEsmtp
                } else {
                    SMTPReplyCode::Code250PlainEsmtp
                };

                if self.force_accept {
                    (Some(State::Helo), Some(reply_code))
                } else {
                    let status = self.rule_engine.run_when("helo");
                    self.process_rules_status(status, Some(State::Helo), Some(reply_code))
                }
            }

            (State::Helo, Event::STARTTLS) if self.tls_config.is_some() => {
                (Some(State::NegotiationTLS), Some(SMTPReplyCode::Code220))
            }

            (State::Helo, Event::STARTTLS) if self.tls_config.is_none() => {
                (None, Some(SMTPReplyCode::Code454))
            }

            (State::Helo, Event::MailCmd(_))
                if self.tls_security_level == TlsSecurityLevel::Encrypt && !self.is_secured =>
            {
                (None, Some(SMTPReplyCode::Code530))
            }

            // SMTP pipeline
            (State::Helo, Event::MailCmd(mail_from)) => {
                // TODO: handle case when sender is already defined.
                self.mail.envelop.set_sender(&mail_from);
                log::trace!(
                    target: "mail_receiver",
                    "[p:{}] envelop=\"{:?}\"",
                    self.port, self.mail.envelop,
                );
                self.rule_engine.add_data("mail", mail_from);

                if self.force_accept {
                    (Some(State::MailFrom), Some(SMTPReplyCode::Code250))
                } else {
                    let status = self.rule_engine.run_when("mail");
                    self.process_rules_status(
                        status,
                        Some(State::MailFrom),
                        Some(SMTPReplyCode::Code250),
                    )
                }
            }

            (State::MailFrom | State::RcptTo, Event::RcptCmd(rcpt_to)) => {
                // TODO: handle case when rcpt is already defined.
                self.mail.envelop.add_rcpt(&rcpt_to);
                log::trace!(
                    target: "mail_receiver",
                    "[p:{}] envelop=\"{:?}\"",
                    self.port, self.mail.envelop,
                );

                (Some(State::RcptTo), Some(SMTPReplyCode::Code250))
            }

            (State::RcptTo, Event::DataCmd) => {
                // NOTE: is it wise to execute the rcpt rule on a DataCmd event ?
                //       it is done this way because the `RCPT TO` command can
                //       be called multiple times.

                self.rule_engine
                    .add_data("rcpt", self.mail.envelop.recipients.clone());

                if self.force_accept {
                    (Some(State::Data), Some(SMTPReplyCode::Code354))
                } else {
                    let status = self.rule_engine.run_when("rcpt");
                    self.process_rules_status(
                        status,
                        Some(State::Data),
                        Some(SMTPReplyCode::Code354),
                    )
                }
            }

            (State::Data, Event::DataLine(line)) => {
                self.mail.body.extend(line.as_bytes().iter());
                self.mail.body.push(b'\n');
                (None, None)
            }

            (State::Data, Event::DataEnd) => {
                let (state, code) = R::on_data_end(&self.mail).await;
                // NOTE: clear envelop and raw_data

                self.rule_engine
                    .add_data("data", match std::str::from_utf8(&self.mail.body) {
                        Ok(data) => data.to_string(),
                        Err(error) => {
                            log::error!(target: "rule_engine", "Couldn't send mail data into rhai context: {}", error);
                            "".to_string()
                        }
                    });

                let result = if self.force_accept {
                    (Some(state), Some(code))
                } else {
                    let status = self.rule_engine.run_when("preq");
                    self.process_rules_status(status, Some(state), Some(code))
                };

                // executing all registered extensive operations.
                if let Err(error) = self.rule_engine.execute_operation_queue(&self.mail) {
                    log::error!(target: "rule_engine", "failed to empty the operation queue: '{}'", error);
                }

                log::info!(
                    "final envelop after executing all rules:\n {:#?}",
                    self.rule_engine.get_scoped_envelop()
                );

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
            Status::Accept | Status::Continue => (desired_state, desired_code),
            Status::Faccept => {
                self.force_accept = true;
                (desired_state, desired_code)
            }
            Status::Deny => (Some(State::Stop), Some(SMTPReplyCode::Code554)),
        }
    }

    /// handle a clear text received with plain_stream or tls_stream
    async fn handle_plain_text(&mut self, client_message: String) -> Option<String> {
        log::trace!(target: "mail_receiver", "[p:{}] buffer=\"{}\"", self.port, client_message);

        let command_or_code = if self.state == State::Data {
            Event::parse_data
        } else {
            Event::parse_cmd
        }(&client_message);

        log::trace!(
            target: "mail_receiver",
            "[p:{}] parsed=\"{:?}\"",
            self.port, command_or_code
        );

        let (new_state, reply) = match command_or_code {
            Ok(event) => self.process_event(event).await,
            Err(error) => (None, Some(error)),
        };

        if let Some(new_state) = new_state {
            log::warn!(
                target: "mail_receiver",
                "[p:{}] ================ STATE: /{:?}/ => /{:?}/",
                self.port, self.state, new_state
            );
            self.state = new_state;
        }

        if let Some(rp) = reply {
            log::warn!(
                target: "mail_receiver",
                "[p:{}] send=\"{:?}\"",
                self.port, rp
            );

            Some(rp.as_str().to_string())
        } else {
            None
        }
    }

    async fn read_and_handle<ReadWrite>(
        &mut self,
        stream: &mut MyConnectionIO<'_, ReadWrite>,
    ) -> Result<(), std::io::Error>
    where
        ReadWrite: std::io::Write + std::io::Read,
    {
        match stream.get_next_line() {
            Ok(client_message) => match self.handle_plain_text(client_message).await {
                Some(response) => stream.write_to_stream(&response),
                None => Ok(()),
            },
            Err(ReadError::Blocking) => Ok(()),
            Err(ReadError::Eof) => {
                log::warn!(target: "mail_receiver", "[p:{}] (secured:{}) eof", self.port, self.is_secured);
                self.state = State::Stop;
                Ok(())
            }
            Err(ReadError::Other(e)) => {
                log::error!(target: "mail_receiver", "[p:{}] (secured:{}) error {}", self.port, self.is_secured, e);
                self.state = State::Stop;
                Err(e)
            }
        }
    }

    fn complete_tls_handshake<ReadWrite: std::io::Write + std::io::Read>(
        io: &mut MyConnectionIO<rustls::Stream<rustls::ServerConnection, ReadWrite>>,
    ) -> Result<(), std::io::Error> {
        let begin_handshake = std::time::Instant::now();
        let duration = std::time::Duration::from_millis(
            crate::config::get::<u64>("tls.handshake_timeout_ms").unwrap_or(10),
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

    async fn receive_secured<ReadWrite>(
        &mut self,
        plain_stream: &'_ mut ReadWrite,
    ) -> Result<(), std::io::Error>
    where
        ReadWrite: std::io::Read + std::io::Write,
    {
        let mut tls_connection =
            rustls::ServerConnection::new(self.tls_config.as_ref().unwrap().clone()).unwrap();

        let mut tls_stream: rustls::Stream<rustls::ServerConnection, ReadWrite> =
            rustls::Stream::new(&mut tls_connection, plain_stream);

        let mut io = MyConnectionIO::new(&mut tls_stream);

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
            self.port, io.inner.conn.is_handshaking()
        );

        log::debug!(
            target: "mail_receiver",
            "[p:{}] protocol_version={:#?}\nalpn_protocol={:#?}\nnegotiated_cipher_suite={:#?}\npeer_certificates={:#?}\nsni_hostname={:#?}",
            self.port,
            io.inner.conn.protocol_version(),
            io.inner.conn.alpn_protocol(),
            io.inner.conn.negotiated_cipher_suite(),
            io.inner.conn.peer_certificates(),
            io.inner.conn.sni_hostname(),
        );

        log::warn!(
            target: "mail_receiver",
            "[p:{}] ================ STATE: /{:?}/ => /{:?}/",
            self.port, self.state, State::Connect
        );

        self.reset();
        self.state = State::Connect;
        self.is_secured = true;

        while self.state != State::Stop {
            self.read_and_handle(&mut io).await?;
        }
        Ok(())
    }

    pub async fn receive_plain<ReadWrite>(
        &mut self,
        mut plain_stream: &'_ mut ReadWrite,
    ) -> Result<(), std::io::Error>
    where
        ReadWrite: std::io::Write + std::io::Read,
    {
        let mut io = MyConnectionIO::new(&mut plain_stream);

        io.write_to_stream(SMTPReplyCode::Code220.as_str())?;

        self.rule_engine.add_data(
            "connect",
            match Ipv4Addr::from_str(&self.ip) {
                Ok(addr) => {
                    log::debug!("connect is '{}'", addr);
                    addr
                }
                Err(error) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("couldn't parse the ip & port: {}", error),
                    ));
                }
            },
        );

        match self.rule_engine.run_when("connect") {
            Status::Deny => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!(
                        "connection at '{}' has been denied when connecting.",
                        self.ip
                    ),
                ))
            }
            Status::Faccept => self.force_accept = true,
            _ => {}
        };

        while self.state != State::Stop {
            if self.state == State::NegotiationTLS {
                return self.receive_secured(plain_stream).await;
            }

            self.read_and_handle(&mut io).await?;
        }
        Ok(())
    }
}
