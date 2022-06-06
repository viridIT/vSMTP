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
use self::transaction::{Transaction, TransactionResult};
use crate::{
    auth, log_channels,
    receiver::auth_exchange::{on_authentication, AuthExchangeError},
};
use vsmtp_common::{
    auth::Mechanism,
    mail_context::MessageBody,
    mail_context::MAIL_CAPACITY,
    re::{anyhow, log, tokio},
    state::StateSMTP,
    status::Status,
    CodeID, MailParserOnFly,
};
use vsmtp_config::{re::rustls, Resolvers};
use vsmtp_rule_engine::rule_engine::RuleEngine;

mod auth_exchange;
mod connection;
mod io;
mod on_mail;
pub mod transaction;

pub use connection::{Connection, ConnectionKind};
pub use io::AbstractIO;
pub use on_mail::{MailHandler, OnMail};

#[allow(clippy::too_many_arguments)]
async fn handle_auth<S>(
    conn: &mut Connection<S>,
    rsasl: std::sync::Arc<tokio::sync::Mutex<auth::Backend>>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    resolvers: std::sync::Arc<Resolvers>,
    helo_domain: &mut Option<String>,
    mechanism: Mechanism,
    initial_response: Option<Vec<u8>>,
    helo_pre_auth: String,
) -> anyhow::Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin,
{
    if let Err(e) = on_authentication(
        conn,
        rsasl,
        rule_engine,
        resolvers,
        mechanism,
        initial_response,
    )
    .await
    {
        log::warn!(
            target: log_channels::TRANSACTION,
            "SASL exchange produced an error: {e}"
        );

        match e {
            AuthExchangeError::Failed => {
                conn.send_code(CodeID::AuthInvalidCredentials).await?;
                anyhow::bail!("{}", CodeID::AuthInvalidCredentials)
            }
            AuthExchangeError::Canceled => {
                conn.authentication_attempt += 1;
                *helo_domain = Some(helo_pre_auth);

                let retries_max = conn
                    .config
                    .server
                    .smtp
                    .auth
                    .as_ref()
                    .unwrap()
                    .attempt_count_max;
                if retries_max != -1 && conn.authentication_attempt > retries_max {
                    conn.send_code(CodeID::AuthRequired).await?;
                    anyhow::bail!("Auth: Attempt max {retries_max} reached");
                }
                conn.send_code(CodeID::AuthClientCanceled).await?;
                Ok(())
            }
            AuthExchangeError::Timeout(_) => {
                conn.send_code(CodeID::Timeout).await?;
                anyhow::bail!("{}", CodeID::Timeout)
            }
            AuthExchangeError::InvalidBase64 => {
                conn.send_code(CodeID::AuthErrorDecode64).await?;
                Ok(())
            }
            otherwise => anyhow::bail!("{otherwise}"),
        }
    } else {
        conn.is_authenticated = true;

        // TODO: When a security layer takes effect
        // helo_domain = None;

        *helo_domain = Some(helo_pre_auth);
        Ok(())
    }
}

#[derive(Default)]
struct NoParsing;

#[async_trait::async_trait]
impl MailParserOnFly for NoParsing {
    async fn parse<'a>(
        &'a mut self,
        mut stream: impl tokio_stream::Stream<Item = String> + Unpin + Send + 'a,
    ) -> anyhow::Result<MessageBody> {
        let mut buffer = Vec::with_capacity(MAIL_CAPACITY / 1000);
        while let Some(line) = tokio_stream::StreamExt::next(&mut stream).await {
            buffer.push(line);
        }
        Ok(MessageBody::Raw(buffer))
    }
}

async fn handle_stream<S, M>(
    conn: &mut Connection<S>,
    mail_handler: &mut M,
    mut transaction: Transaction,
    helo_domain: &mut Option<String>,
) -> anyhow::Result<bool>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + Sync,
    M: OnMail + Send,
{
    let body = {
        let stream = Transaction::stream(conn);
        tokio::pin!(stream);
        NoParsing::default().parse(stream).await?
    };

    *transaction.rule_state.message().write().unwrap() = Some(body);

    let status = transaction
        .rule_engine
        .read()
        .unwrap()
        .run_when(&mut transaction.rule_state, &StateSMTP::PreQ);
    match status {
        Status::Info(packet) => {
            conn.send_reply_or_code(packet).await?;
            return Ok(true);
        }
        Status::Deny(packet) => {
            conn.send_reply_or_code(packet).await?;
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

    let (mail_context, message) = transaction.rule_state.take().unwrap();

    let helo = mail_context.envelop.helo.clone();
    let code = mail_handler
        .on_mail(conn, Box::new(mail_context), message.unwrap())
        .await;
    *helo_domain = Some(helo);
    conn.send_code(code).await?;

    Ok(true)
}

// NOTE: handle_connection and handle_connection_secured do the same things..
// but i struggle to unify these function because of recursive type
// see `rustc --explain E0275`

/// Receives the incomings mail of a connection
///
/// # Errors
///
/// * server failed to send a message
/// * a transaction failed
/// * the pre-queue processing of the mail failed
///
/// # Panics
/// * the authentication is issued but gsasl was not found.
#[allow(clippy::too_many_lines)]
pub async fn handle_connection<S, M>(
    conn: &mut Connection<S>,
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    rsasl: Option<std::sync::Arc<tokio::sync::Mutex<auth::Backend>>>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    resolvers: std::sync::Arc<Resolvers>,
    mail_handler: &mut M,
) -> anyhow::Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + Sync,
    M: OnMail + Send,
{
    if conn.kind == ConnectionKind::Tunneled {
        if let Some(tls_config) = tls_config {
            return handle_connection_secured(
                conn,
                tls_config,
                rsasl,
                rule_engine,
                resolvers,
                mail_handler,
            )
            .await;
        }
        anyhow::bail!("config ill-formed, handling a secured connection without valid config")
    }

    let mut helo_domain = None;

    conn.send_code(CodeID::Greetings).await?;

    while conn.is_alive {
        let mut transaction =
            Transaction::new(conn, &helo_domain, rule_engine.clone(), resolvers.clone()).await?;

        if let Some(outcome) = transaction.receive(conn, &helo_domain).await? {
            match outcome {
                TransactionResult::Data => {
                    if !handle_stream(conn, mail_handler, transaction, &mut helo_domain).await? {
                        return Ok(());
                    }
                }
                TransactionResult::TlsUpgrade => {
                    if let Some(tls_config) = tls_config {
                        return handle_connection_secured(
                            conn,
                            tls_config,
                            rsasl,
                            rule_engine,
                            resolvers,
                            mail_handler,
                        )
                        .await;
                    }
                    conn.send_code(CodeID::TlsNotAvailable).await?;
                    anyhow::bail!("{:?}", CodeID::TlsNotAvailable)
                }
                TransactionResult::Authentication(helo_pre_auth, mechanism, initial_response) => {
                    if let Some(rsasl) = &rsasl {
                        handle_auth(
                            conn,
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
                        conn.send_code(CodeID::Unimplemented).await?;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn handle_connection_secured<S, M>(
    conn: &mut Connection<S>,
    tls_config: std::sync::Arc<rustls::ServerConfig>,
    rsasl: Option<std::sync::Arc<tokio::sync::Mutex<auth::Backend>>>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    resolvers: std::sync::Arc<Resolvers>,
    mail_handler: &mut M,
) -> anyhow::Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + Sync,
    M: OnMail + Send,
{
    let mut secured_conn = {
        let smtps_config = conn.config.server.tls.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "server accepted tls encrypted transaction, but not tls config provided"
            )
        })?;
        let acceptor = tokio_rustls::TlsAcceptor::from(tls_config);

        let stream = tokio::time::timeout(
            smtps_config.handshake_timeout,
            acceptor.accept(&mut conn.inner.inner),
        )
        .await??;

        Connection::new_with(
            conn.kind,
            stream
                .get_ref()
                .1
                .sni_hostname()
                .unwrap_or(&conn.server_name)
                .to_string(),
            conn.timestamp,
            conn.config.clone(),
            conn.client_addr,
            conn.error_count,
            true,
            conn.is_authenticated,
            conn.authentication_attempt,
            stream,
        )
    };

    if secured_conn.kind == ConnectionKind::Tunneled {
        secured_conn.send_code(CodeID::Greetings).await?;
    }

    let mut helo_domain = None;

    while secured_conn.is_alive {
        let mut transaction = Transaction::new(
            &mut secured_conn,
            &helo_domain,
            rule_engine.clone(),
            resolvers.clone(),
        )
        .await?;

        if let Some(outcome) = transaction.receive(&mut secured_conn, &helo_domain).await? {
            match outcome {
                TransactionResult::Data => {
                    if !handle_stream(
                        &mut secured_conn,
                        mail_handler,
                        transaction,
                        &mut helo_domain,
                    )
                    .await?
                    {
                        return Ok(());
                    }
                }
                TransactionResult::TlsUpgrade => {
                    secured_conn.send_code(CodeID::AlreadyUnderTLS).await?;
                }
                TransactionResult::Authentication(helo_pre_auth, mechanism, initial_response) => {
                    if let Some(rsasl) = &rsasl {
                        handle_auth(
                            &mut secured_conn,
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
                        secured_conn.send_code(CodeID::Unimplemented).await?;
                    }
                }
            }
        }
    }

    Ok(())
}
