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
use self::transaction::{Transaction, TransactionResult};
use crate::{
    processes::ProcessMessage, queue::Queue, receiver::auth::on_authentication, server::SaslBackend,
};
use vsmtp_common::{
    auth::Mechanism,
    code::SMTPReplyCode,
    mail_context::MailContext,
    re::{anyhow, log},
};
use vsmtp_config::re::rustls;
use vsmtp_rule_engine::rule_engine::RuleEngine;

mod auth;
mod connection;
mod io_service;
pub(crate) mod transaction;

pub use connection::{Connection, ConnectionKind};
pub use io_service::IoService;

#[cfg(test)]
mod tests;

// NOTE: not marked as #[cfg(test)] because it is used by the bench/fuzz
/// boilerplate for the tests
pub mod test_helpers;

/// will be executed once the email is received.
#[async_trait::async_trait]
pub trait OnMail {
    /// the server executes this function once the email as been received.
    async fn on_mail<S: std::io::Read + std::io::Write + Send>(
        &mut self,
        conn: &mut Connection<'_, S>,
        mail: Box<MailContext>,
        helo_domain: &mut Option<String>,
    ) -> anyhow::Result<()>;
}

/// default mail handler for production.
pub(crate) struct MailHandler {
    pub working_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
    pub delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
}

#[async_trait::async_trait]
impl OnMail for MailHandler {
    async fn on_mail<S: std::io::Read + std::io::Write + Send>(
        &mut self,
        conn: &mut Connection<'_, S>,
        mail: Box<MailContext>,
        helo_domain: &mut Option<String>,
    ) -> anyhow::Result<()> {
        *helo_domain = Some(mail.envelop.helo.clone());

        if mail
            .envelop
            .rcpt
            .iter()
            .all(|rcpt| rcpt.transfer_method == vsmtp_common::transfer::Transfer::None)
        {
            // quietly skipping mime & delivery processes when all recipients do not need transfer.
            // TODO: move to dead queue.
            log::warn!("delivery skipped because all recipient's transfer method is set to None.");
            conn.send_code(SMTPReplyCode::Code250)?;
            return Ok(());
        }

        match &mail.metadata {
            Some(metadata) if metadata.skipped.is_some() => {
                log::warn!("postq skipped due to {:?}.", metadata.skipped.unwrap());
                match Queue::Deliver.write_to_queue(&conn.config, &mail) {
                    Ok(_) => {
                        self.delivery_sender
                            .send(ProcessMessage {
                                message_id: metadata.message_id.clone(),
                            })
                            .await?;

                        conn.send_code(SMTPReplyCode::Code250)?;
                    }
                    Err(error) => {
                        log::error!("couldn't write to delivery queue: {}", error);
                        conn.send_code(SMTPReplyCode::Code554)?;
                    }
                };
                Ok(())
            }
            Some(metadata) => {
                match Queue::Working.write_to_queue(&conn.config, &mail) {
                    Ok(_) => {
                        self.working_sender
                            .send(ProcessMessage {
                                message_id: metadata.message_id.clone(),
                            })
                            .await?;

                        conn.send_code(SMTPReplyCode::Code250)?;
                    }
                    Err(error) => {
                        log::error!("couldn't write to queue: {}", error);
                        conn.send_code(SMTPReplyCode::Code554)?;
                    }
                };
                Ok(())
            }
            _ => unreachable!(),
        }
    }
}

async fn handle_auth<S>(
    conn: &mut Connection<'_, S>,
    rsasl: std::sync::Arc<tokio::sync::Mutex<SaslBackend>>,
    helo_domain: &mut Option<String>,
    mechanism: Mechanism,
    initial_response: Option<Vec<u8>>,
    helo_pre_auth: String,
) -> anyhow::Result<()>
where
    S: std::io::Read + std::io::Write + Send,
{
    match on_authentication(conn, rsasl, mechanism, initial_response).await {
        Err(auth::AuthExchangeError::Failed) => {
            conn.send_code(SMTPReplyCode::AuthInvalidCredentials)?;
            anyhow::bail!("Auth: Credentials invalid, closing connection");
        }
        Err(auth::AuthExchangeError::Canceled) => {
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
                conn.send_code(SMTPReplyCode::AuthRequired)?;
                anyhow::bail!("Auth: Attempt max {} reached", retries_max);
            }
            conn.send_code(SMTPReplyCode::AuthClientCanceled)?;
        }
        Err(auth::AuthExchangeError::Timeout(e)) => {
            conn.send_code(SMTPReplyCode::Code451Timeout)?;
            anyhow::bail!(std::io::Error::new(std::io::ErrorKind::TimedOut, e));
        }
        Err(auth::AuthExchangeError::InvalidBase64) => {
            conn.send_code(SMTPReplyCode::AuthErrorDecode64)?;
        }
        Err(auth::AuthExchangeError::Other(e)) => anyhow::bail!("{}", e),
        Ok(_) => {
            conn.is_authenticated = true;

            // TODO: When a security layer takes effect
            // helo_domain = None;

            *helo_domain = Some(helo_pre_auth);
        }
    }
    Ok(())
}

// NOTE: handle_connection and handle_connection_secured do the same things..
// but i struggle to unify these function because of recursive type

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
pub async fn handle_connection<S, M>(
    conn: &mut Connection<'_, S>,
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    rsasl: Option<std::sync::Arc<tokio::sync::Mutex<SaslBackend>>>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    mail_handler: &mut M,
) -> anyhow::Result<()>
where
    S: std::io::Read + std::io::Write + Send,
    M: OnMail + Send,
{
    if let ConnectionKind::Tunneled = conn.kind {
        if let Some(tls_config) = tls_config {
            return handle_connection_secured(conn, tls_config, rsasl, rule_engine, mail_handler)
                .await;
        }
        conn.send_code(SMTPReplyCode::Code454)?;
        conn.send_code(SMTPReplyCode::Code221)?;
        return Ok(());
    }

    let mut helo_domain = None;

    conn.send_code(SMTPReplyCode::Greetings)?;

    while conn.is_alive {
        match Transaction::receive(conn, &helo_domain, rule_engine.clone()).await? {
            TransactionResult::Nothing => {}
            TransactionResult::Mail(mail) => {
                mail_handler.on_mail(conn, mail, &mut helo_domain).await?;
            }
            TransactionResult::TlsUpgrade => {
                if let Some(tls_config) = tls_config {
                    return handle_connection_secured(
                        conn,
                        tls_config,
                        rsasl,
                        rule_engine,
                        mail_handler,
                    )
                    .await;
                }
                conn.send_code(SMTPReplyCode::Code454)?;
                conn.send_code(SMTPReplyCode::Code221)?;
                return Ok(());
            }
            TransactionResult::Authentication(helo_pre_auth, mechanism, initial_response) => {
                if let Some(rsasl) = &rsasl {
                    handle_auth(
                        conn,
                        rsasl.clone(),
                        &mut helo_domain,
                        mechanism,
                        initial_response,
                        helo_pre_auth,
                    )
                    .await?;
                } else {
                    todo!()
                }
            }
        }
    }

    Ok(())
}

async fn handle_connection_secured<S, M>(
    conn: &mut Connection<'_, S>,
    tls_config: std::sync::Arc<rustls::ServerConfig>,
    rsasl: Option<std::sync::Arc<tokio::sync::Mutex<SaslBackend>>>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    mail_handler: &mut M,
) -> anyhow::Result<()>
where
    S: std::io::Read + std::io::Write + Send,
    M: OnMail + Send,
{
    let smtps_config = conn.config.server.tls.as_ref().ok_or_else(|| {
        anyhow::anyhow!("server accepted tls encrypted transaction, but not tls config provided")
    })?;

    let mut tls_conn = rustls::ServerConnection::new(tls_config).unwrap();
    let mut tls_stream = rustls::Stream::new(&mut tls_conn, &mut conn.io_stream);
    let mut io_tls_stream = IoService::new(&mut tls_stream);

    Connection::<IoService<'_, S>>::complete_tls_handshake(
        &mut io_tls_stream,
        &smtps_config.handshake_timeout,
    )?;

    let mut secured_conn = Connection {
        kind: conn.kind,
        timestamp: conn.timestamp,
        config: conn.config.clone(),
        client_addr: conn.client_addr,
        error_count: conn.error_count,
        is_authenticated: conn.is_authenticated,
        authentication_attempt: conn.authentication_attempt,
        is_alive: true,
        is_secured: true,
        io_stream: &mut io_tls_stream,
    };

    if let ConnectionKind::Tunneled = secured_conn.kind {
        secured_conn.send_code(SMTPReplyCode::Greetings)?;
    }

    let mut helo_domain = None;

    while secured_conn.is_alive {
        match Transaction::receive(&mut secured_conn, &helo_domain, rule_engine.clone()).await? {
            TransactionResult::Nothing => {}
            TransactionResult::Mail(mail) => {
                mail_handler
                    .on_mail(&mut secured_conn, mail, &mut helo_domain)
                    .await?;
            }
            TransactionResult::TlsUpgrade => {
                secured_conn.send_code(SMTPReplyCode::TlsAlreadyUnderTls)?;
            }
            TransactionResult::Authentication(helo_pre_auth, mechanism, initial_response) => {
                if let Some(rsasl) = &rsasl {
                    handle_auth(
                        &mut secured_conn,
                        rsasl.clone(),
                        &mut helo_domain,
                        mechanism,
                        initial_response,
                        helo_pre_auth,
                    )
                    .await?;
                } else {
                    todo!();
                }
            }
        }
    }
    Ok(())
}
