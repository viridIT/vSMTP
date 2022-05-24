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
use crate::{auth, receiver::auth_exchange::on_authentication, ProcessMessage};
use vsmtp_common::{
    auth::Mechanism,
    mail_context::MailContext,
    queue::Queue,
    re::{anyhow, log},
    status::Status,
    CodeID,
};
use vsmtp_config::{create_app_folder, re::rustls, Resolvers};
use vsmtp_rule_engine::rule_engine::RuleEngine;

mod auth_exchange;
mod connection;
mod io;
pub mod transaction;

pub use connection::{Connection, ConnectionKind};
pub use io::AbstractIO;

/// will be executed once the email is received.
#[async_trait::async_trait]
pub trait OnMail {
    /// the server executes this function once the email as been received.
    async fn on_mail<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
        &mut self,
        conn: &mut Connection<S>,
        mail: Box<MailContext>,
    ) -> anyhow::Result<CodeID>;
}

/// default mail handler for production.
pub struct MailHandler {
    /// message pipe to the working process.
    pub working_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
    /// message pipe to the delivery process.
    pub delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
}

#[async_trait::async_trait]
impl OnMail for MailHandler {
    async fn on_mail<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
        &mut self,
        conn: &mut Connection<S>,
        mail: Box<MailContext>,
    ) -> anyhow::Result<CodeID> {
        let metadata = mail.metadata.as_ref().unwrap();

        let next_queue = match &metadata.skipped {
            Some(Status::Quarantine(path)) => {
                let mut path = create_app_folder(&conn.config, Some(path))?;
                path.push(format!("{}.json", metadata.message_id));

                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)?;

                std::io::Write::write_all(
                    &mut file,
                    vsmtp_common::re::serde_json::to_string_pretty(&*mail)?.as_bytes(),
                )?;

                log::warn!("postq & delivery skipped due to quarantine.");
                return Ok(CodeID::Ok);
            }
            Some(reason) => {
                log::warn!("postq skipped due to '{}'.", reason.as_ref());
                Queue::Deliver
            }
            None => Queue::Working,
        };

        if let Err(error) = next_queue.write_to_queue(&conn.config.server.queues.dirpath, &mail) {
            log::error!("couldn't write to '{}' queue: {}", next_queue, error);
            Ok(CodeID::Denied)
        } else {
            match next_queue {
                Queue::Working => &self.working_sender,
                Queue::Deliver => &self.delivery_sender,
                _ => unreachable!(),
            }
            .send(ProcessMessage {
                message_id: metadata.message_id.clone(),
            })
            .await?;

            Ok(CodeID::Ok)
        }
    }
}

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
    match on_authentication(
        conn,
        rsasl,
        rule_engine,
        resolvers,
        mechanism,
        initial_response,
    )
    .await
    {
        Err(auth_exchange::AuthExchangeError::Failed) => {
            conn.send_code(CodeID::AuthInvalidCredentials).await?;
            anyhow::bail!("Auth: Credentials invalid, closing connection");
        }
        Err(auth_exchange::AuthExchangeError::Canceled) => {
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
                anyhow::bail!("Auth: Attempt max {} reached", retries_max);
            }
            conn.send_code(CodeID::AuthClientCanceled).await?;
        }
        Err(auth_exchange::AuthExchangeError::Timeout(e)) => {
            conn.send_code(CodeID::Timeout).await?;
            anyhow::bail!(std::io::Error::new(std::io::ErrorKind::TimedOut, e));
        }
        Err(auth_exchange::AuthExchangeError::InvalidBase64) => {
            conn.send_code(CodeID::AuthErrorDecode64).await?;
        }
        Err(auth_exchange::AuthExchangeError::Other(e)) => anyhow::bail!("{}", e),
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
        match Transaction::receive(conn, &helo_domain, rule_engine.clone(), resolvers.clone())
            .await?
        {
            TransactionResult::Nothing => {}
            TransactionResult::Mail(mail) => {
                let helo = mail.envelop.helo.clone();
                let code = mail_handler.on_mail(conn, mail).await?;
                helo_domain = Some(helo);
                conn.send_code(code).await?;
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
    let smtps_config = conn.config.server.tls.as_ref().ok_or_else(|| {
        anyhow::anyhow!("server accepted tls encrypted transaction, but not tls config provided")
    })?;
    let acceptor = tokio_rustls::TlsAcceptor::from(tls_config);

    let stream = tokio::time::timeout(
        smtps_config.handshake_timeout,
        acceptor.accept(&mut conn.inner.inner),
    )
    .await??;

    let mut secured_conn = Connection::new_with(
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
    );

    if secured_conn.kind == ConnectionKind::Tunneled {
        secured_conn.send_code(CodeID::Greetings).await?;
    }

    let mut helo_domain = None;

    while secured_conn.is_alive {
        match Transaction::receive(
            &mut secured_conn,
            &helo_domain,
            rule_engine.clone(),
            resolvers.clone(),
        )
        .await?
        {
            TransactionResult::Nothing => {}
            TransactionResult::Mail(mail) => {
                let helo = mail.envelop.helo.clone();
                let code = mail_handler.on_mail(&mut secured_conn, mail).await?;
                helo_domain = Some(helo);
                secured_conn.send_code(code).await?;
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
    Ok(())
}
