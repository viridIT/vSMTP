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
    model::{
        envelop::Envelop,
        mail::{Body, MailContext, MessageMetadata, MAIL_CAPACITY},
    },
    rules::{
        address::Address,
        rule_engine::{RuleEngine, RuleState, Status},
    },
    smtp::{code::SMTPReplyCode, event::Event, state::StateSMTP},
};

const TIMEOUT_DEFAULT: u64 = 5 * 60 * 1000; // 5min

pub struct Transaction<'re> {
    state: StateSMTP,
    rule_state: RuleState<'re>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
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
                {
                    let ctx = self.rule_state.get_context();
                    let mut ctx = ctx.write().unwrap();
                    ctx.body = Body::Raw(String::with_capacity(MAIL_CAPACITY));
                    ctx.envelop.rcpt.clear();
                    ctx.envelop.mail_from = Address::default();
                }

                self.rule_state.reset();

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

                match self
                    .rule_engine
                    .read()
                    .unwrap()
                    .run_when(&mut self.rule_state, "helo")
                {
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

                match self
                    .rule_engine
                    .read()
                    .unwrap()
                    .run_when(&mut self.rule_state, "helo")
                {
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

            (StateSMTP::Helo, Event::StartTls) => {
                match conn.config.tls.as_ref().map(|smtps| smtps.security_level) {
                    None | Some(TlsSecurityLevel::None) => {
                        ProcessedEvent::Reply(SMTPReplyCode::Code454)
                    }
                    _ => ProcessedEvent::ReplyChangeState(
                        StateSMTP::NegotiationTLS,
                        SMTPReplyCode::Code220,
                    ),
                }
            }

            (StateSMTP::Helo, Event::MailCmd(_, _))
                if !conn.is_secured
                    && conn.config.tls.as_ref().map(|smtps| smtps.security_level)
                        == Some(TlsSecurityLevel::Encrypt) =>
            {
                ProcessedEvent::Reply(SMTPReplyCode::Code530)
            }

            (StateSMTP::Helo, Event::MailCmd(mail_from, _body_bit_mime)) => {
                // TODO: store in envelop _body_bit_mime
                self.set_mail_from(mail_from, conn);

                match self
                    .rule_engine
                    .read()
                    .unwrap()
                    .run_when(&mut self.rule_state, "mail")
                {
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

                match self
                    .rule_engine
                    .read()
                    .unwrap()
                    .run_when(&mut self.rule_state, "rcpt")
                {
                    Status::Deny => {
                        ProcessedEvent::ReplyChangeState(StateSMTP::Stop, SMTPReplyCode::Code554)
                    }
                    _ if self
                        .rule_state
                        .get_context()
                        .read()
                        .unwrap()
                        .envelop
                        .rcpt
                        .len()
                        >= conn.config.smtp.rcpt_count_max =>
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
                let ctx = self.rule_state.get_context();
                if let Body::Raw(body) = &mut ctx.write().unwrap().body {
                    body.push_str(&line);
                    body.push('\n');
                }
                ProcessedEvent::Nothing
            }

            (StateSMTP::Data, Event::DataEnd) => {
                if let Status::Block | Status::Deny = self
                    .rule_engine
                    .read()
                    .unwrap()
                    .run_when(&mut self.rule_state, "preq")
                {
                    return ProcessedEvent::ReplyChangeState(
                        StateSMTP::Stop,
                        SMTPReplyCode::Code554,
                    );
                }

                let ctx = self.rule_state.get_context();
                let mut ctx = ctx.write().unwrap();

                // executing all registered extensive operations.
                if let Err(error) = self.rule_state.execute_operation_queue(&conn.config, &ctx) {
                    log::error!(
                        target: RULES,
                        "failed to empty the operation queue: '{}'",
                        error
                    );
                }
                // TODO: find a better way to propagate force accept.
                // the "skipped" field is updated by the rule engine internal state,
                // which does result in hard to read code, but it was the fastest way
                // to propagate the force accept to the server.
                // Alternatives:
                //  - return ProcessedEvent::CompletedMimeSkipped
                //  - set body to Body::ParsingFailed or Body::ParsingSkipped.
                if let Some(metadata) = &mut ctx.metadata {
                    metadata.skipped = self.rule_state.skipped();
                }

                let mut output = MailContext {
                    client_addr: ctx.client_addr,
                    envelop: Envelop::default(),
                    body: Body::Raw(String::default()),
                    metadata: None,
                };

                std::mem::swap(&mut *ctx, &mut output);

                ProcessedEvent::TransactionCompleted(Box::new(output))
            }
            _ => ProcessedEvent::Reply(SMTPReplyCode::Code503),
        }
    }
}

impl Transaction<'_> {
    fn set_connect<S: std::io::Read + std::io::Write>(&mut self, conn: &Connection<S>) {
        self.rule_state.add_data("connect", conn.client_addr.ip());
        self.rule_state.add_data("port", conn.client_addr.port());
        self.rule_state
            .add_data("connection_timestamp", conn.timestamp);
    }

    fn set_helo(&mut self, helo: String) {
        self.rule_state.reset();

        let ctx = self.rule_state.get_context();
        let mut ctx = ctx.write().unwrap();
        ctx.envelop = Envelop {
            helo,
            mail_from: Address::default(),
            rcpt: std::collections::HashSet::default(),
        };

        log::trace!(target: RECEIVER, "envelop=\"{:?}\"", ctx.envelop);
    }

    fn set_mail_from<S>(&mut self, mail_from: String, conn: &Connection<'_, S>)
    where
        S: std::io::Write + std::io::Read,
    {
        match Address::new(&mail_from) {
            Err(_) => (),
            Ok(mail_from) => {
                let now = std::time::SystemTime::now();

                let ctx = self.rule_state.get_context();
                let mut ctx = ctx.write().unwrap();
                ctx.body = Body::Raw(String::with_capacity(MAIL_CAPACITY));
                ctx.envelop.rcpt.clear();
                ctx.envelop.mail_from = mail_from;
                ctx.metadata = Some(MessageMetadata {
                    timestamp: now,
                    // TODO: find a way to handle SystemTime failure.
                    message_id: format!(
                        "{}{}{}{}",
                        now.duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .unwrap_or(std::time::Duration::ZERO)
                            .as_micros(),
                        conn.timestamp
                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .unwrap_or(std::time::Duration::ZERO)
                            .as_millis(),
                        std::iter::repeat_with(fastrand::alphanumeric)
                            .take(36)
                            .collect::<String>(),
                        std::process::id()
                    ),
                    retry: 0,
                    resolver: "default".to_string(),
                    skipped: self.rule_state.skipped(),
                });

                log::trace!(target: RECEIVER, "envelop=\"{:?}\"", ctx.envelop,);
            }
        }
    }

    fn set_rcpt_to(&mut self, rcpt_to: String) {
        match Address::new(&rcpt_to) {
            Err(_) => (),
            Ok(rcpt_to) => {
                let ctx = self.rule_state.get_context();
                let mut ctx = ctx.write().unwrap();
                ctx.envelop.rcpt.insert(rcpt_to);

                log::trace!(target: RECEIVER, "envelop=\"{:?}\"", ctx.envelop,);
            }
        }
    }
}

impl Transaction<'_> {
    pub async fn receive<'a, 'b, S: std::io::Read + std::io::Write>(
        conn: &'a mut Connection<'b, S>,
        helo_domain: &Option<String>,
        rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    ) -> anyhow::Result<TransactionResult> {
        let mut transaction = Transaction {
            state: if helo_domain.is_none() {
                StateSMTP::Connect
            } else {
                StateSMTP::Helo
            },
            rule_state: RuleState::new(conn.config.as_ref()),
            rule_engine,
        };

        transaction.set_connect(conn);

        if let Some(helo) = helo_domain.as_ref().cloned() {
            transaction.set_helo(helo)
        }

        if let Status::Deny = transaction
            .rule_engine
            .read()
            .unwrap()
            .run_when(&mut transaction.rule_state, "connect")
        {
            anyhow::bail!(
                "connection at '{}' has been denied when connecting.",
                conn.client_addr
            )
        };

        fn get_timeout_for_state(
            config: &std::sync::Arc<ServerConfig>,
            state: &StateSMTP,
        ) -> std::time::Duration {
            config
                .smtp
                .timeout_client
                .get(state)
                .map(|t| t.alias)
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
                    anyhow::bail!(e)
                }
                Err(e) => {
                    conn.send_code(SMTPReplyCode::Code451Timeout)?;
                    anyhow::bail!(std::io::Error::new(std::io::ErrorKind::TimedOut, e))
                }
            }
        }

        conn.is_alive = false;
        Ok(TransactionResult::Nothing)
    }
}
