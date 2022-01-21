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
use crate::{
    config::{
        log_channel::{RECEIVER, RULES},
        server_config::{ServerConfig, TlsSecurityLevel},
    },
    connection::Connection,
    io_service::ReadError,
    mime::parser::MailMimeParser,
    model::{
        envelop::Envelop,
        mail::{Body, MailContext, MessageMetadata, MAIL_CAPACITY},
    },
    rules::{
        address::Address,
        rule_engine::{RuleEngine, Status},
    },
    smtp::{code::SMTPReplyCode, event::Event, state::StateSMTP},
};

const TIMEOUT_DEFAULT: u64 = 5 * 60 * 1000; // 5min

pub struct Transaction<'re> {
    state: StateSMTP,
    mail: MailContext,
    rule_engine: RuleEngine<'re>,
}

pub enum TransactionResult {
    Nothing,
    Mail(Box<MailContext>),
    TlsUpgrade,
}

// Generated from a string received
enum ProcessedEvent {
    Nothing,
    Reply(SMTPReplyCode),
    ReplyChangeState(StateSMTP, SMTPReplyCode),
    TransactionCompleted(Box<MailContext>),
}

impl Transaction<'_> {
    fn parse_and_apply_and_get_reply<S: std::io::Read + std::io::Write>(
        &mut self,
        conn: &Connection<S>,
        client_message: String,
    ) -> ProcessedEvent {
        log::trace!(target: RECEIVER, "buffer=\"{}\"", client_message);

        let command_or_code = if self.state == StateSMTP::Data {
            Event::parse_data
        } else {
            Event::parse_cmd
        }(&client_message);

        log::trace!(target: RECEIVER, "parsed=\"{:?}\"", command_or_code);

        command_or_code
            .map(|command| self.process_event(conn, command))
            .unwrap_or_else(ProcessedEvent::Reply)
    }

    fn process_event<S: std::io::Read + std::io::Write>(
        &mut self,
        conn: &Connection<S>,
        event: Event,
    ) -> ProcessedEvent {
        match (&self.state, event) {
            (_, Event::NoopCmd) => ProcessedEvent::Reply(SMTPReplyCode::Code250),

            (_, Event::HelpCmd(_)) => ProcessedEvent::Reply(SMTPReplyCode::Code214),

            (_, Event::RsetCmd) => {
                self.mail.body = Body::Raw(String::with_capacity(MAIL_CAPACITY));
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
                log::trace!(target: RECEIVER, "envelop=\"{:?}\"", self.mail.envelop,);

                match self.rule_engine.run_when("helo") {
                    Status::Deny => {
                        ProcessedEvent::ReplyChangeState(StateSMTP::Stop, SMTPReplyCode::Code554)
                    }
                    _ => ProcessedEvent::ReplyChangeState(StateSMTP::Helo, SMTPReplyCode::Code250),
                }
            }

            (_, Event::EhloCmd(_)) if conn.config.smtp.disable_ehlo => {
                ProcessedEvent::Reply(SMTPReplyCode::Code502unimplemented)
            }

            (_, Event::EhloCmd(helo)) => {
                self.set_helo(helo);
                log::trace!(target: RECEIVER, "envelop=\"{:?}\"", self.mail.envelop,);

                match self.rule_engine.run_when("helo") {
                    Status::Deny => {
                        ProcessedEvent::ReplyChangeState(StateSMTP::Stop, SMTPReplyCode::Code554)
                    }
                    _ => ProcessedEvent::ReplyChangeState(
                        StateSMTP::Helo,
                        if conn.is_secured {
                            SMTPReplyCode::Code250SecuredEsmtp
                        } else {
                            SMTPReplyCode::Code250PlainEsmtp
                        },
                    ),
                }
            }

            (StateSMTP::Helo, Event::StartTls)
                if conn.config.tls.security_level != TlsSecurityLevel::None =>
            {
                ProcessedEvent::ReplyChangeState(StateSMTP::NegotiationTLS, SMTPReplyCode::Code220)
            }
            (StateSMTP::Helo, Event::StartTls)
                if conn.config.tls.security_level == TlsSecurityLevel::None =>
            {
                ProcessedEvent::Reply(SMTPReplyCode::Code454)
            }

            (StateSMTP::Helo, Event::MailCmd(_, _))
                if conn.config.tls.security_level == TlsSecurityLevel::Encrypt
                    && !conn.is_secured =>
            {
                ProcessedEvent::Reply(SMTPReplyCode::Code530)
            }

            (StateSMTP::Helo, Event::MailCmd(mail_from, _body_bit_mime)) => {
                // TODO: store in envelop _body_bit_mime

                self.mail.body = Body::Raw(String::with_capacity(MAIL_CAPACITY));
                self.set_mail_from(mail_from, conn);

                log::trace!(target: RECEIVER, "envelop=\"{:?}\"", self.mail.envelop,);

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

                log::trace!(target: RECEIVER, "envelop=\"{:?}\"", self.mail.envelop,);

                match self.rule_engine.run_when("rcpt") {
                    Status::Deny => {
                        ProcessedEvent::ReplyChangeState(StateSMTP::Stop, SMTPReplyCode::Code554)
                    }
                    _ if self.mail.envelop.rcpt.len()
                        >= conn.config.smtp.rcpt_count_max.unwrap_or(usize::MAX) =>
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
                if let Body::Raw(body) = &mut self.mail.body {
                    body.push_str(&line);
                    body.push('\n');
                }
                ProcessedEvent::Nothing
            }

            (StateSMTP::Data, Event::DataEnd) => {
                let parsed = MailMimeParser::default()
                    .parse(match &self.mail.body {
                        Body::Raw(raw) => raw.as_bytes(),
                        _ => unreachable!("the email cannot be parsed before the DataEnd command"),
                    })
                    // TODO: handle parsing errors instead of going default.
                    .unwrap_or_default();

                self.rule_engine.add_data("data", parsed);

                let status = self.rule_engine.run_when("preq");

                // TODO: block & deny should quarantine the email.
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
                match self.rule_engine.get_scoped_envelop() {
                    Some((envelop, mail)) => {
                        self.mail.envelop = envelop;
                        self.mail.body = Body::Parsed(mail.into());

                        let mut output = MailContext {
                            envelop: Envelop::default(),
                            body: Body::Raw(String::default()),
                            metadata: None,
                        };

                        std::mem::swap(&mut self.mail, &mut output);

                        ProcessedEvent::TransactionCompleted(Box::new(output))
                    }
                    _ => ProcessedEvent::Reply(SMTPReplyCode::Code451),
                }
            }

            _ => ProcessedEvent::Reply(SMTPReplyCode::Code503),
        }
    }
}

impl Transaction<'_> {
    fn set_connect<S: std::io::Read + std::io::Write>(&mut self, conn: &Connection<S>) {
        self.rule_engine.add_data("connect", conn.client_addr.ip());
        self.rule_engine.add_data("port", conn.client_addr.port());
        self.rule_engine
            .add_data("connection_timestamp", conn.timestamp);
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

    fn set_mail_from<S>(&mut self, mail_from: String, conn: &Connection<'_, S>)
    where
        S: std::io::Write + std::io::Read,
    {
        match Address::new(&mail_from) {
            Err(_) => (),
            Ok(mail_from) => {
                self.mail.envelop.mail_from = mail_from;
                self.mail.envelop.rcpt.clear();
                self.rule_engine.reset();

                let now = std::time::SystemTime::now();

                self.mail.metadata = Some(MessageMetadata {
                    timestamp: now,
                    // TODO: find a way to handle SystemTime failure.
                    message_id: format!(
                        "{}{}{}",
                        now.duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .unwrap_or(std::time::Duration::ZERO)
                            .as_micros(),
                        conn.timestamp
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
    }

    // FIXME: too many clone
    fn set_rcpt_to(&mut self, rcpt_to: String) {
        match Address::new(&rcpt_to) {
            Err(_) => (),
            Ok(rcpt_to) => {
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
            }
        }
    }
}

impl Transaction<'_> {
    pub async fn receive<'a, 'b, S: std::io::Read + std::io::Write>(
        conn: &'a mut Connection<'b, S>,
        helo_domain: &Option<String>,
    ) -> std::io::Result<TransactionResult> {
        let mut transaction = Transaction {
            state: if helo_domain.is_none() {
                StateSMTP::Connect
            } else {
                StateSMTP::Helo
            },
            mail: MailContext {
                envelop: Envelop::default(),
                body: Body::Raw(String::with_capacity(MAIL_CAPACITY)),
                metadata: None,
            },
            rule_engine: RuleEngine::new(conn.config.as_ref()),
        };

        transaction.set_connect(conn);

        if let Some(helo) = helo_domain.as_ref().cloned() {
            transaction.set_helo(helo)
        }

        if let Status::Deny = transaction.rule_engine.run_when("connect") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "connection at '{}' has been denied when connecting.",
                    conn.client_addr
                ),
            ));
        };

        fn get_timeout_for_state(
            config: &std::sync::Arc<ServerConfig>,
            state: &StateSMTP,
        ) -> std::time::Duration {
            config
                .smtp
                .timeout_client
                .as_ref()
                .map(|map| map.get(state).map(|t| t.alias))
                .flatten()
                .unwrap_or_else(|| std::time::Duration::from_millis(TIMEOUT_DEFAULT))
        }

        let mut read_timeout = get_timeout_for_state(&conn.config, &transaction.state);

        while transaction.state != StateSMTP::Stop {
            if transaction.state == StateSMTP::NegotiationTLS {
                return Ok(TransactionResult::TlsUpgrade);
            }
            match conn.read(read_timeout).await {
                Ok(Ok(client_message)) => {
                    match transaction.parse_and_apply_and_get_reply(conn, client_message) {
                        ProcessedEvent::Reply(reply_to_send) => {
                            conn.send_code(reply_to_send)?;
                        }
                        ProcessedEvent::ReplyChangeState(new_state, reply_to_send) => {
                            log::info!(
                                target: RECEIVER,
                                "================ STATE: /{:?}/ => /{:?}/",
                                transaction.state,
                                new_state
                            );
                            transaction.state = new_state;
                            read_timeout = get_timeout_for_state(&conn.config, &transaction.state);
                            conn.send_code(reply_to_send)?;
                        }
                        ProcessedEvent::TransactionCompleted(mail) => {
                            return Ok(TransactionResult::Mail(mail))
                        }
                        ProcessedEvent::Nothing => {}
                    }
                }
                Ok(Err(ReadError::Blocking)) => {}
                Ok(Err(ReadError::Eof)) => {
                    log::info!(target: RECEIVER, "eof");
                    transaction.state = StateSMTP::Stop;
                }
                Ok(Err(ReadError::Other(e))) => {
                    // TODO: send error to client ?
                    return Err(e);
                }
                Err(e) => {
                    conn.send_code(SMTPReplyCode::Code451Timeout)?;
                    return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, e));
                }
            }
        }

        conn.is_alive = false;
        Ok(TransactionResult::Nothing)
    }
}
