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
use self::{
    auth_exchange::on_authentication,
    transaction::{Transaction, TransactionResult},
};
use crate::{auth, receiver::auth_exchange::AuthExchangeError};
use vsmtp_common::{
    auth::Mechanism,
    mail_context::MAIL_CAPACITY,
    re::{anyhow, log, tokio},
    state::StateSMTP,
    status::Status,
    CodeID, ConnectionKind, Either, MailParserOnFly, MessageBody, ParserOutcome, RawBody,
};
use vsmtp_config::{re::rustls, Resolvers};
use vsmtp_rule_engine::RuleEngine;

mod auth_exchange;
mod connection;
mod io;
mod on_mail;
pub mod transaction;

pub use connection::Connection;
pub use io::AbstractIO;
pub use on_mail::{MailHandler, MailHandlerError, OnMail};

#[derive(Default)]
struct NoParsing;

#[async_trait::async_trait]
impl MailParserOnFly for NoParsing {
    async fn parse<'a>(
        &'a mut self,
        mut stream: impl tokio_stream::Stream<Item = String> + Unpin + Send + 'a,
    ) -> ParserOutcome {
        let mut headers = Vec::with_capacity(20);
        let mut body = String::with_capacity(MAIL_CAPACITY);

        while let Some(line) = tokio_stream::StreamExt::next(&mut stream).await {
            if line.is_empty() {
                break;
            }
            headers.push(line);
        }

        while let Some(line) = tokio_stream::StreamExt::next(&mut stream).await {
            body.push_str(&line);
            body.push_str("\r\n");
        }

        Ok(Either::Left(RawBody::new(headers, body)))
    }
}

impl<S> Connection<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + Sync + std::fmt::Debug,
{
    /// Receives the incomings mail of a connection
    ///
    /// # Errors
    ///
    /// * server failed to send a message
    /// * a transaction failed
    /// * the pre-queue processing of the mail failed
    #[tracing::instrument(skip(tls_config, rsasl, rule_engine, resolvers, mail_handler))]
    pub async fn receive<M>(
        &mut self,
        tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
        rsasl: Option<std::sync::Arc<tokio::sync::Mutex<auth::Backend>>>,
        rule_engine: std::sync::Arc<RuleEngine>,
        resolvers: std::sync::Arc<Resolvers>,
        mail_handler: &mut M,
    ) -> anyhow::Result<()>
    where
        M: OnMail + Send,
    {
        if self.kind == ConnectionKind::Tunneled {
            if let Some(tls_config) = tls_config {
                return self
                    .upgrade_to_secured(tls_config, rsasl, rule_engine, resolvers, mail_handler)
                    .await;
            }
            anyhow::bail!("config ill-formed, handling a secured connection without valid config")
        }

        let mut helo_domain = None;

        self.send_code(CodeID::Greetings).await?;

        while self.is_alive {
            let mut transaction =
                Transaction::new(self, &helo_domain, rule_engine.clone(), resolvers.clone()).await;

            if let Some(outcome) = transaction.receive(self, &helo_domain).await? {
                match outcome {
                    TransactionResult::HandshakeSMTP => {
                        if !self
                            .handle_stream(mail_handler, transaction, &mut helo_domain)
                            .await?
                        {
                            return Ok(());
                        }
                    }
                    TransactionResult::HandshakeTLS => {
                        if let Some(tls_config) = tls_config {
                            return self
                                .upgrade_to_secured(
                                    tls_config,
                                    rsasl,
                                    rule_engine,
                                    resolvers,
                                    mail_handler,
                                )
                                .await;
                        }
                        self.send_code(CodeID::TlsNotAvailable).await?;
                        anyhow::bail!("{:?}", CodeID::TlsNotAvailable)
                    }
                    TransactionResult::HandshakeSASL(
                        helo_pre_auth,
                        mechanism,
                        initial_response,
                    ) => {
                        if let Some(rsasl) = &rsasl {
                            self.handle_auth(
                                rsasl.clone(),
                                rule_engine.clone(),
                                resolvers.clone(),
                                &mut helo_domain,
                                mechanism,
                                initial_response,
                                helo_pre_auth,
                            )
                            .await?;
                        } else {
                            self.send_code(CodeID::Unimplemented).await?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    // NOTE: the implementation of `receive` and `receive_secured` are very similar,
    // but need to be distinct (and thus not called in a recursion fashion) because of
    // `rustc --explain E0275`
    // TODO: could keep the `parent` to produce better logs
    #[tracing::instrument(parent = None, skip(rsasl, rule_engine, resolvers, mail_handler))]
    async fn receive_secured<M>(
        &mut self,
        rsasl: Option<std::sync::Arc<tokio::sync::Mutex<auth::Backend>>>,
        rule_engine: std::sync::Arc<RuleEngine>,
        resolvers: std::sync::Arc<Resolvers>,
        mail_handler: &mut M,
    ) -> anyhow::Result<()>
    where
        M: OnMail + Send,
    {
        if self.kind == ConnectionKind::Tunneled {
            self.send_code(CodeID::Greetings).await?;
        }

        let mut helo_domain = None;

        while self.is_alive {
            let mut transaction =
                Transaction::new(self, &helo_domain, rule_engine.clone(), resolvers.clone()).await;

            if let Some(outcome) = transaction.receive(self, &helo_domain).await? {
                match outcome {
                    TransactionResult::HandshakeSMTP => {
                        if !self
                            .handle_stream(mail_handler, transaction, &mut helo_domain)
                            .await?
                        {
                            return Ok(());
                        }
                    }
                    TransactionResult::HandshakeTLS => {
                        self.send_code(CodeID::AlreadyUnderTLS).await?;
                    }
                    TransactionResult::HandshakeSASL(
                        helo_pre_auth,
                        mechanism,
                        initial_response,
                    ) => {
                        if let Some(rsasl) = &rsasl {
                            self.handle_auth(
                                rsasl.clone(),
                                rule_engine.clone(),
                                resolvers.clone(),
                                &mut helo_domain,
                                mechanism,
                                initial_response,
                                helo_pre_auth,
                            )
                            .await?;
                        } else {
                            self.send_code(CodeID::Unimplemented).await?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn upgrade_to_secured<M>(
        &mut self,
        tls_config: std::sync::Arc<rustls::ServerConfig>,
        rsasl: Option<std::sync::Arc<tokio::sync::Mutex<auth::Backend>>>,
        rule_engine: std::sync::Arc<RuleEngine>,
        resolvers: std::sync::Arc<Resolvers>,
        mail_handler: &mut M,
    ) -> anyhow::Result<()>
    where
        M: OnMail + Send,
    {
        let mut secured_conn = {
            let smtps_config = self.config.server.tls.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "server accepted tls encrypted transaction, but not tls config provided"
                )
            })?;
            let acceptor = tokio_rustls::TlsAcceptor::from(tls_config.clone());

            let stream = tokio::time::timeout(
                smtps_config.handshake_timeout,
                acceptor.accept(&mut self.inner.inner),
            )
            .await??;

            Connection::new_with(
                self.kind,
                stream
                    .get_ref()
                    .1
                    .sni_hostname()
                    .unwrap_or(&self.server_name)
                    .to_string(),
                self.timestamp,
                self.config.clone(),
                self.client_addr,
                self.server_addr,
                self.error_count,
                true,
                self.is_authenticated,
                self.authentication_attempt,
                stream,
            )
        };

        secured_conn
            .receive_secured(rsasl, rule_engine, resolvers, mail_handler)
            .await
    }

    async fn handle_stream<M>(
        &mut self,
        mail_handler: &mut M,
        mut transaction: Transaction,
        helo_domain: &mut Option<String>,
    ) -> anyhow::Result<bool>
    where
        M: OnMail + Send,
    {
        // fetching the email using the transaction's stream.
        {
            log::info!("SMTP handshake completed, fetching email");
            let mut body = {
                let stream = Transaction::stream(self);
                tokio::pin!(stream);
                NoParsing::default().parse(stream).await?
            };

            let handle = transaction.rule_state.message();
            let mut message = handle.write().unwrap();

            // Headers could have been added to the email before preq,
            // so we start by prepending them to the headers received.
            let preq_headers = message.inner().headers_lines();

            match &mut body {
                Either::Left(raw) => raw.prepend_header(preq_headers.map(str::to_string)),
                Either::Right(parsed) => parsed.prepend_headers(preq_headers.filter_map(|s| {
                    s.split_once(':')
                        .map(|(key, value)| (key.to_string(), value.to_string()))
                })),
            };

            *message = MessageBody::from(body);
        }

        let status = transaction
            .rule_engine
            .run_when(&mut transaction.rule_state, &StateSMTP::PreQ);

        match status {
            Status::Info(packet) => {
                self.send_reply_or_code(packet).await?;
                return Ok(true);
            }
            Status::Deny(packet) => {
                self.send_reply_or_code(packet).await?;
                return Ok(false);
            }
            _ => (),
        }

        {
            let mail_context = transaction.rule_state.context();
            let mut state_writer = mail_context.write().unwrap();
            if let Some(metadata) = &mut state_writer.metadata {
                metadata.skipped = transaction.rule_state.skipped().cloned();
            }
        }

        let (mail_context, message, _) = transaction.rule_state.take().unwrap();

        let helo = mail_context.envelop.helo.clone();
        let code = mail_handler
            .on_mail(self, Box::new(mail_context), message)
            .await;
        *helo_domain = Some(helo);
        self.send_code(code).await?;

        Ok(true)
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_auth(
        &mut self,
        rsasl: std::sync::Arc<tokio::sync::Mutex<auth::Backend>>,
        rule_engine: std::sync::Arc<RuleEngine>,
        resolvers: std::sync::Arc<Resolvers>,
        helo_domain: &mut Option<String>,
        mechanism: Mechanism,
        initial_response: Option<Vec<u8>>,
        helo_pre_auth: String,
    ) -> anyhow::Result<()> {
        if let Err(e) = on_authentication(
            self,
            rsasl,
            rule_engine,
            resolvers,
            mechanism,
            initial_response,
        )
        .await
        {
            log::warn!("SASL exchange produced an error: {e}");

            match e {
                AuthExchangeError::Failed => {
                    self.send_code(CodeID::AuthInvalidCredentials).await?;
                    anyhow::bail!("{}", CodeID::AuthInvalidCredentials)
                }
                AuthExchangeError::Canceled => {
                    self.authentication_attempt += 1;
                    *helo_domain = Some(helo_pre_auth);

                    let retries_max = self
                        .config
                        .server
                        .smtp
                        .auth
                        .as_ref()
                        .unwrap()
                        .attempt_count_max;
                    if retries_max != -1 && self.authentication_attempt > retries_max {
                        self.send_code(CodeID::AuthRequired).await?;
                        anyhow::bail!("Auth: Attempt max {retries_max} reached");
                    }
                    self.send_code(CodeID::AuthClientCanceled).await?;
                    Ok(())
                }
                AuthExchangeError::Timeout(_) => {
                    self.send_code(CodeID::Timeout).await?;
                    anyhow::bail!("{}", CodeID::Timeout)
                }
                AuthExchangeError::InvalidBase64 => {
                    self.send_code(CodeID::AuthErrorDecode64).await?;
                    Ok(())
                }
                otherwise => anyhow::bail!("{otherwise}"),
            }
        } else {
            self.is_authenticated = true;

            // TODO: When a security layer takes effect
            // helo_domain = None;

            *helo_domain = Some(helo_pre_auth);
            Ok(())
        }
    }
}
