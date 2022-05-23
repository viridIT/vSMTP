/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/
use super::connection::Connection;
use crate::log_channels;
use vsmtp_common::{
    addr,
    auth::Mechanism,
    envelop::Envelop,
    event::Event,
    mail_context::{Body, ConnectionContext, MailContext, MessageMetadata, MAIL_CAPACITY},
    rcpt::Rcpt,
    re::{anyhow, log},
    state::StateSMTP,
    status::Status,
    Address, CodeID, ReplyOrCodeID,
};
use vsmtp_config::{Config, Resolvers, TlsSecurityLevel};
use vsmtp_rule_engine::{rule_engine::RuleEngine, rule_state::RuleState};

const TIMEOUT_DEFAULT: u64 = 5 * 60 * 1000; // 5min

pub struct Transaction {
    state: StateSMTP,
    rule_state: RuleState,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
}

#[allow(clippy::module_name_repetitions)]
pub enum TransactionResult {
    Nothing,
    Mail(Box<MailContext>),
    TlsUpgrade,
    Authentication(String, Mechanism, Option<Vec<u8>>),
}

// Generated from a string received
enum ProcessedEvent {
    Nothing,
    Reply(ReplyOrCodeID),
    ChangeState(StateSMTP),
    ReplyChangeState(StateSMTP, ReplyOrCodeID),
    TransactionCompleted(Box<MailContext>),
}

impl Transaction {
    fn parse_and_apply_and_get_reply<
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin,
    >(
        &mut self,
        conn: &Connection<S>,
        client_message: &str,
    ) -> ProcessedEvent {
        log::trace!(
            target: log_channels::TRANSACTION,
            "buffer=\"{}\"",
            client_message
        );

        let command_or_code = if self.state == StateSMTP::Data {
            Event::parse_data
        } else {
            Event::parse_cmd
        }(client_message);

        log::trace!(
            target: log_channels::TRANSACTION,
            "parsed=\"{:?}\"",
            command_or_code
        );

        command_or_code.map_or_else(
            |c| ProcessedEvent::Reply(ReplyOrCodeID::CodeID(c)),
            |command| self.process_event(conn, command),
        )
    }

    #[allow(clippy::too_many_lines)]
    fn process_event<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
        &mut self,
        conn: &Connection<S>,
        event: Event,
    ) -> ProcessedEvent {
        match (&self.state, event) {
            (_, Event::NoopCmd) => ProcessedEvent::Reply(ReplyOrCodeID::CodeID(CodeID::Ok)),

            (_, Event::HelpCmd(_)) => ProcessedEvent::Reply(ReplyOrCodeID::CodeID(CodeID::Help)),

            (_, Event::RsetCmd) => {
                {
                    let state = self.rule_state.context();
                    let mut ctx = state.write().unwrap();
                    ctx.body = Body::Empty;
                    ctx.metadata = None;
                    ctx.envelop.rcpt.clear();
                    ctx.envelop.mail_from = addr!("default@domain.com");
                }

                ProcessedEvent::ReplyChangeState(StateSMTP::Helo, ReplyOrCodeID::CodeID(CodeID::Ok))
            }

            (_, Event::ExpnCmd(_) | Event::VrfyCmd(_) /*| Event::PrivCmd*/) => {
                ProcessedEvent::Reply(ReplyOrCodeID::CodeID(CodeID::Unimplemented))
            }

            (_, Event::QuitCmd) => ProcessedEvent::ReplyChangeState(
                StateSMTP::Stop,
                ReplyOrCodeID::CodeID(CodeID::Closing),
            ),

            (_, Event::HeloCmd(helo)) => {
                self.set_helo(helo);

                match self
                    .rule_engine
                    .read()
                    .unwrap()
                    .run_when(&mut self.rule_state, &StateSMTP::Helo)
                {
                    Status::Info(packet) => ProcessedEvent::Reply(packet),
                    Status::Deny(packet) => {
                        ProcessedEvent::ReplyChangeState(StateSMTP::Stop, packet)
                    }
                    _ => ProcessedEvent::ReplyChangeState(
                        StateSMTP::Helo,
                        ReplyOrCodeID::CodeID(CodeID::Ok),
                    ),
                }
            }

            (_, Event::EhloCmd(_)) if conn.config.server.smtp.disable_ehlo => {
                ProcessedEvent::Reply(ReplyOrCodeID::CodeID(CodeID::Unimplemented))
            }

            (_, Event::EhloCmd(helo)) => {
                self.set_helo(helo);

                match self
                    .rule_engine
                    .read()
                    .unwrap()
                    .run_when(&mut self.rule_state, &StateSMTP::Helo)
                {
                    Status::Info(packet) => ProcessedEvent::Reply(packet),
                    Status::Deny(packet) => {
                        ProcessedEvent::ReplyChangeState(StateSMTP::Stop, packet)
                    }
                    _ => ProcessedEvent::ReplyChangeState(
                        StateSMTP::Helo,
                        ReplyOrCodeID::CodeID(if conn.is_secured {
                            CodeID::EhloSecured
                        } else {
                            CodeID::EhloPain
                        }),
                    ),
                }
            }

            (StateSMTP::Helo | StateSMTP::Connect, Event::StartTls)
                if conn.config.server.tls.is_none() =>
            {
                ProcessedEvent::Reply(ReplyOrCodeID::CodeID(CodeID::TlsNotAvailable))
            }

            (StateSMTP::Helo | StateSMTP::Connect, Event::StartTls)
                if conn.config.server.tls.is_some() =>
            {
                ProcessedEvent::ReplyChangeState(
                    StateSMTP::NegotiationTLS,
                    ReplyOrCodeID::CodeID(CodeID::Greetings),
                )
            }

            (StateSMTP::Helo, Event::Auth(mechanism, initial_response))
                if !conn.is_authenticated =>
            {
                ProcessedEvent::ChangeState(StateSMTP::Authentication(mechanism, initial_response))
            }

            (StateSMTP::Helo, Event::MailCmd(..))
                if !conn.is_secured
                    && conn
                        .config
                        .server
                        .tls
                        .as_ref()
                        .map(|smtps| smtps.security_level)
                        == Some(TlsSecurityLevel::Encrypt) =>
            {
                ProcessedEvent::Reply(ReplyOrCodeID::CodeID(CodeID::TlsRequired))
            }

            (StateSMTP::Helo, Event::MailCmd(..))
                if !conn.is_authenticated
                    && conn
                        .config
                        .server
                        .smtp
                        .auth
                        .as_ref()
                        .map_or(false, |auth| auth.must_be_authenticated) =>
            {
                ProcessedEvent::Reply(ReplyOrCodeID::CodeID(CodeID::AuthRequired))
            }

            (StateSMTP::Helo, Event::MailCmd(mail_from, _body_bit_mime, _auth_mailbox)) => {
                // TODO: store in envelop _body_bit_mime & _auth_mailbox
                // TODO: handle : mail_from can be "<>""
                self.set_mail_from(mail_from.unwrap(), conn);

                match self
                    .rule_engine
                    .read()
                    .unwrap()
                    .run_when(&mut self.rule_state, &StateSMTP::MailFrom)
                {
                    Status::Info(packet) => ProcessedEvent::Reply(packet),
                    Status::Deny(packet) => {
                        ProcessedEvent::ReplyChangeState(StateSMTP::Stop, packet)
                    }
                    _ => ProcessedEvent::ReplyChangeState(
                        StateSMTP::MailFrom,
                        ReplyOrCodeID::CodeID(CodeID::Ok),
                    ),
                }
            }

            (StateSMTP::MailFrom | StateSMTP::RcptTo, Event::RcptCmd(rcpt_to)) => {
                self.set_rcpt_to(rcpt_to);

                match self
                    .rule_engine
                    .read()
                    .unwrap()
                    .run_when(&mut self.rule_state, &StateSMTP::RcptTo)
                {
                    Status::Info(packet) => ProcessedEvent::Reply(packet),
                    Status::Deny(packet) => {
                        ProcessedEvent::ReplyChangeState(StateSMTP::Stop, packet)
                    }
                    _ if self.rule_state.context().read().unwrap().envelop.rcpt.len()
                        >= conn.config.server.smtp.rcpt_count_max =>
                    {
                        ProcessedEvent::ReplyChangeState(
                            StateSMTP::RcptTo,
                            ReplyOrCodeID::CodeID(CodeID::TooManyRecipients),
                        )
                    }
                    _ => ProcessedEvent::ReplyChangeState(
                        StateSMTP::RcptTo,
                        ReplyOrCodeID::CodeID(CodeID::Ok),
                    ),
                }
            }

            (StateSMTP::RcptTo, Event::DataCmd) => {
                self.rule_state.context().write().unwrap().body =
                    Body::Raw(String::with_capacity(MAIL_CAPACITY));
                ProcessedEvent::ReplyChangeState(
                    StateSMTP::Data,
                    ReplyOrCodeID::CodeID(CodeID::DataStart),
                )
            }

            (StateSMTP::Data, Event::DataLine(line)) => {
                let state = self.rule_state.context();
                if let Body::Raw(body) = &mut state.write().unwrap().body {
                    body.push_str(&line);
                    body.push('\n');
                }
                ProcessedEvent::Nothing
            }

            (StateSMTP::Data, Event::DataEnd) => {
                match self
                    .rule_engine
                    .read()
                    .unwrap()
                    .run_when(&mut self.rule_state, &StateSMTP::PreQ)
                {
                    Status::Info(packet) => return ProcessedEvent::Reply(packet),
                    Status::Deny(packet) => {
                        return ProcessedEvent::ReplyChangeState(StateSMTP::Stop, packet)
                    }
                    _ => {}
                }

                let state = self.rule_state.context();
                let mut ctx = state.write().unwrap();

                // NOTE: the "skipped" field is updated by the rule engine internal state,
                //       which does result in hard to read code, but it was the fastest way
                //       to propagate the force accept to the server.
                //       Alternatives:
                //        - return ProcessedEvent::CompletedMimeSkipped
                //        - set body to Body::ParsingFailed or Body::ParsingSkipped.
                if let Some(metadata) = &mut ctx.metadata {
                    metadata.skipped = self.rule_state.skipped().cloned();
                }

                let mut output = MailContext {
                    connection: ConnectionContext {
                        timestamp: std::time::SystemTime::now(),
                        credentials: None,
                        is_authenticated: conn.is_authenticated,
                        is_secured: conn.is_secured,
                        server_name: conn.server_name.clone(),
                    },
                    client_addr: ctx.client_addr,
                    envelop: Envelop::default(),
                    body: Body::Empty,
                    metadata: None,
                };

                std::mem::swap(&mut *ctx, &mut output);

                ProcessedEvent::TransactionCompleted(Box::new(output))
            }

            _ => ProcessedEvent::Reply(ReplyOrCodeID::CodeID(CodeID::BadSequence)),
        }
    }

    fn set_connect<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
        &mut self,
        conn: &Connection<S>,
    ) {
        let state = self.rule_state.context();
        let ctx = &mut state.write().unwrap();

        ctx.client_addr = conn.client_addr;
        ctx.connection.timestamp = conn.timestamp;
    }

    fn set_helo(&mut self, helo: String) {
        let state = self.rule_state.context();
        let mut ctx = state.write().unwrap();

        ctx.body = Body::Empty;
        ctx.metadata = None;
        ctx.envelop = Envelop {
            helo,
            mail_from: addr!("no@address.net"),
            rcpt: vec![],
        };
    }

    fn set_mail_from<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
        &mut self,
        mail_from: Address,
        conn: &Connection<S>,
    ) {
        let now = std::time::SystemTime::now();

        let state = self.rule_state.context();
        let mut ctx = state.write().unwrap();
        ctx.body = Body::Empty;
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
            skipped: self.rule_state.skipped().cloned(),
        });

        log::trace!(
            target: log_channels::TRANSACTION,
            "envelop=\"{:?}\"",
            ctx.envelop,
        );
    }

    fn set_rcpt_to(&mut self, rcpt_to: Address) {
        self.rule_state
            .context()
            .write()
            .unwrap()
            .envelop
            .rcpt
            .push(Rcpt::new(rcpt_to));
    }
}

impl Transaction {
    #[allow(clippy::too_many_lines)]
    pub async fn receive<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Sync + Send + Unpin>(
        conn: &mut Connection<S>,
        helo_domain: &Option<String>,
        rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
        resolvers: std::sync::Arc<Resolvers>,
    ) -> anyhow::Result<TransactionResult> {
        let rule_state = RuleState::with_connection(
            conn.config.as_ref(),
            resolvers,
            &*rule_engine
                .read()
                .map_err(|_| anyhow::anyhow!("failed to lock rule engine"))?,
            ConnectionContext {
                timestamp: conn.timestamp,
                credentials: None,
                is_authenticated: conn.is_authenticated,
                is_secured: conn.is_secured,
                server_name: conn.server_name.clone(),
            },
        );

        let mut transaction = Self {
            state: if helo_domain.is_none() {
                StateSMTP::Connect
            } else {
                StateSMTP::Helo
            },
            rule_state,
            rule_engine,
        };

        if let Some(helo) = helo_domain.as_ref().cloned() {
            transaction.set_helo(helo);
        } else {
            transaction.set_connect(conn);

            let status = transaction
                .rule_engine
                .read()
                .map_err(|_| anyhow::anyhow!("Rule engine mutex poisoned"))?
                .run_when(&mut transaction.rule_state, &StateSMTP::Connect);

            match status {
                Status::Info(packet) => match packet {
                    ReplyOrCodeID::CodeID(code) => conn.send_code(code).await?,
                    ReplyOrCodeID::Reply(reply) => conn.send_reply(reply).await?,
                },
                Status::Deny(packet) => {
                    match packet {
                        ReplyOrCodeID::CodeID(code) => conn.send_code(code).await?,
                        ReplyOrCodeID::Reply(reply) => conn.send_reply(reply).await?,
                    }

                    anyhow::bail!(
                        "connection at '{}' has been denied when connecting.",
                        conn.client_addr
                    )
                }
                _ => {}
            }
        }

        let mut read_timeout = get_timeout_for_state(&conn.config, &transaction.state);

        loop {
            match transaction.state {
                StateSMTP::NegotiationTLS => return Ok(TransactionResult::TlsUpgrade),
                StateSMTP::Authentication(mechanism, initial_response) => {
                    return Ok(TransactionResult::Authentication(
                        transaction
                            .rule_state
                            .context()
                            .read()
                            .unwrap()
                            .envelop
                            .helo
                            .clone(),
                        mechanism,
                        initial_response,
                    ));
                }
                StateSMTP::Stop => {
                    conn.is_alive = false;
                    return Ok(TransactionResult::Nothing);
                }
                _ => match conn.read(read_timeout).await {
                    Ok(Some(client_message)) => {
                        match transaction.parse_and_apply_and_get_reply(conn, &client_message) {
                            ProcessedEvent::Nothing => {}
                            ProcessedEvent::Reply(reply_to_send) => match reply_to_send {
                                ReplyOrCodeID::CodeID(code) => conn.send_code(code).await?,
                                ReplyOrCodeID::Reply(reply) => conn.send_reply(reply).await?,
                            },
                            ProcessedEvent::ChangeState(new_state) => {
                                log::info!(
                                    target: log_channels::TRANSACTION,
                                    "================ STATE: /{:?}/ => /{:?}/",
                                    transaction.state,
                                    new_state
                                );
                                transaction.state = new_state;
                                read_timeout =
                                    get_timeout_for_state(&conn.config, &transaction.state);
                            }
                            ProcessedEvent::ReplyChangeState(new_state, reply_to_send) => {
                                log::info!(
                                    target: log_channels::TRANSACTION,
                                    "================ STATE: /{:?}/ => /{:?}/",
                                    transaction.state,
                                    new_state
                                );
                                transaction.state = new_state;
                                read_timeout =
                                    get_timeout_for_state(&conn.config, &transaction.state);
                                match reply_to_send {
                                    ReplyOrCodeID::CodeID(code) => conn.send_code(code).await?,
                                    ReplyOrCodeID::Reply(reply) => conn.send_reply(reply).await?,
                                }
                            }
                            ProcessedEvent::TransactionCompleted(mail) => {
                                return Ok(TransactionResult::Mail(mail));
                            }
                        }
                    }
                    Ok(None) => {
                        log::info!(target: log_channels::TRANSACTION, "eof");
                        transaction.state = StateSMTP::Stop;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                        conn.send_code(CodeID::Timeout).await?;
                        anyhow::bail!(e)
                    }
                    Err(e) => {
                        anyhow::bail!(e)
                    }
                },
            }
        }
    }
}

fn get_timeout_for_state(
    config: &std::sync::Arc<Config>,
    state: &StateSMTP,
) -> std::time::Duration {
    match state {
        StateSMTP::Connect => config.server.smtp.timeout_client.connect,
        StateSMTP::Helo => config.server.smtp.timeout_client.helo,
        StateSMTP::MailFrom => config.server.smtp.timeout_client.mail_from,
        StateSMTP::RcptTo => config.server.smtp.timeout_client.rcpt_to,
        StateSMTP::Data => config.server.smtp.timeout_client.data,
        _ => std::time::Duration::from_millis(TIMEOUT_DEFAULT),
    }
}
