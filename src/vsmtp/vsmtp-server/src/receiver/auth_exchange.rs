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
use crate::{
    auth::{self, Session},
    log_channels,
};

use super::Connection;
use vsmtp_common::{
    auth::Mechanism,
    mail_context::ConnectionContext,
    re::{anyhow, base64, log, vsmtp_rsasl},
    CodeID,
};
use vsmtp_config::Resolvers;
use vsmtp_rule_engine::rule_engine::RuleEngine;

/// Result of the AUTH command
#[allow(clippy::module_name_repetitions)]
#[must_use]
pub enum AuthExchangeError {
    /// authentication invalid
    Failed,
    /// the client stopped the exchange
    Canceled,
    /// timeout of the server
    Timeout(std::io::Error),
    ///
    InvalidBase64,
    ///
    Other(anyhow::Error),
}

async fn auth_step<S>(
    conn: &mut Connection<S>,
    session: &mut vsmtp_rsasl::DiscardOnDrop<Session>,
    buffer: &[u8],
) -> Result<bool, AuthExchangeError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin,
{
    if buffer == [b'*'] {
        return Err(AuthExchangeError::Canceled);
    }

    let bytes64decoded = base64::decode(buffer).map_err(|_| AuthExchangeError::InvalidBase64)?;

    match session.step(&bytes64decoded) {
        Ok(vsmtp_rsasl::Step::Done(buffer)) => {
            if !buffer.is_empty() {
                // TODO: send buffer ?
                println!(
                    "Authentication successful, bytes to return to client: {:?}",
                    std::str::from_utf8(&*buffer)
                );
            }

            conn.send_code(CodeID::AuthSucceeded)
                .await
                .map_err(AuthExchangeError::Other)?;
            Ok(true)
        }
        Ok(vsmtp_rsasl::Step::NeedsMore(buffer)) => {
            let reply = format!(
                "334 {}\r\n",
                base64::encode(std::str::from_utf8(&*buffer).unwrap())
            );

            conn.send(&reply).await.map_err(AuthExchangeError::Other)?;
            Ok(false)
        }
        Err(e) if e.matches(vsmtp_rsasl::ReturnCode::GSASL_AUTHENTICATION_ERROR) => {
            Err(AuthExchangeError::Failed)
        }
        Err(e) => Err(AuthExchangeError::Other(anyhow::anyhow!("{}", e))),
    }
}

pub async fn on_authentication<S>(
    conn: &mut Connection<S>,
    rsasl: std::sync::Arc<tokio::sync::Mutex<auth::Backend>>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    resolvers: std::sync::Arc<Resolvers>,
    mechanism: Mechanism,
    initial_response: Option<Vec<u8>>,
) -> Result<(), AuthExchangeError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin,
{
    // TODO: if initial data == "=" ; it mean empty ""

    if mechanism.must_be_under_tls() && !conn.is_secured {
        if conn
            .config
            .server
            .smtp
            .auth
            .as_ref()
            .map_or(false, |auth| auth.enable_dangerous_mechanism_in_clair)
        {
            log::warn!(
                target: log_channels::AUTH,
                "An unsecured AUTH mechanism ({mechanism}) is used on a non-encrypted connection!"
            );
        } else {
            conn.send_code(CodeID::AuthMechanismMustBeEncrypted)
                .await
                .map_err(AuthExchangeError::Other)?;

            return Err(AuthExchangeError::Other(anyhow::anyhow!(
                "{:?}",
                CodeID::AuthMechanismMustBeEncrypted
            )));
        }
    }

    if !mechanism.client_first() && initial_response.is_some() {
        conn.send_code(CodeID::AuthClientMustNotStart)
            .await
            .map_err(AuthExchangeError::Other)?;

        return Err(AuthExchangeError::Other(anyhow::anyhow!(
            "{:?}",
            CodeID::AuthClientMustNotStart
        )));
    }
    let mut guard = rsasl.lock().await;
    let mut session = guard.server_start(&String::from(mechanism)).unwrap();
    session.store(Box::new((
        rule_engine,
        resolvers,
        ConnectionContext {
            timestamp: conn.timestamp,
            credentials: None,
            is_authenticated: conn.is_authenticated,
            is_secured: conn.is_secured,
            server_name: conn.server_name.clone(),
        },
    )));

    let mut succeeded =
        auth_step(conn, &mut session, &initial_response.unwrap_or_default()).await?;

    while !succeeded {
        succeeded = match conn.read(std::time::Duration::from_secs(1)).await {
            Ok(Some(buffer)) => {
                log::trace!(target: log_channels::AUTH, "{buffer}");
                auth_step(conn, &mut session, buffer.as_bytes()).await
            }
            Ok(None) => Err(AuthExchangeError::Other(anyhow::anyhow!("eof"))),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                Err(AuthExchangeError::Timeout(e))
            }
            Err(e) => Err(AuthExchangeError::Other(anyhow::anyhow!("{:?}", e))),
        }?;
    }

    // TODO: if success get session property

    Ok(())
}
