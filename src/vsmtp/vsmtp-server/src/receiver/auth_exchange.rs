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
use crate::auth::{self, Session};

use super::Connection;
use vsmtp_common::{
    auth::Mechanism,
    mail_context::ConnectionContext,
    re::{anyhow, base64, log, tokio, vsmtp_rsasl},
    CodeID,
};
use vsmtp_config::Resolvers;
use vsmtp_rule_engine::RuleEngine;

#[allow(clippy::module_name_repetitions)]
#[must_use]
#[derive(thiserror::Error, Debug)]
pub enum AuthExchangeError {
    #[error("authentication invalid")]
    Failed,
    #[error("authentication cancelled")]
    Canceled,
    #[error("authentication timeout")]
    Timeout(std::io::Error),
    #[error("base64 decoding error")]
    InvalidBase64,
    #[error("error while sending a response: `{0}`")]
    SendingResponse(anyhow::Error),
    #[error("error while reading message: `{0}`")]
    ReadingMessage(std::io::Error),
    #[error("internal error while processing the SASL exchange: `{0}`")]
    StepError(vsmtp_rsasl::SaslError),
    #[error("mechanism `{0}` must be used in encrypted connection")]
    AuthMechanismMustBeEncrypted(Mechanism),
    #[error("client started the authentication but server did not send any challenge: `{0}`")]
    AuthClientMustNotStart(Mechanism),
}

async fn auth_step<S>(
    conn: &mut Connection<S>,
    session: &mut vsmtp_rsasl::DiscardOnDrop<Session>,
    buffer: &[u8],
) -> Result<bool, AuthExchangeError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + std::fmt::Debug,
{
    if buffer == [b'*'] {
        return Err(AuthExchangeError::Canceled);
    }

    let bytes64decoded = base64::decode(buffer).map_err(|_| AuthExchangeError::InvalidBase64)?;

    match session.step(&bytes64decoded) {
        Ok(vsmtp_rsasl::Step::Done(buffer)) => {
            if !buffer.is_empty() {
                todo!(
                    "Authentication successful, bytes to return to client: {:?}",
                    std::str::from_utf8(&*buffer)
                );
            }

            conn.send_code(CodeID::AuthSucceeded)
                .await
                .map_err(AuthExchangeError::SendingResponse)?;
            Ok(true)
        }
        Ok(vsmtp_rsasl::Step::NeedsMore(buffer)) => {
            let reply = format!(
                "334 {}\r\n",
                base64::encode(std::str::from_utf8(&*buffer).unwrap())
            );

            conn.send(&reply)
                .await
                .map_err(AuthExchangeError::SendingResponse)?;
            Ok(false)
        }
        Err(e) if e.matches(vsmtp_rsasl::ReturnCode::GSASL_AUTHENTICATION_ERROR) => {
            Err(AuthExchangeError::Failed)
        }
        Err(e) => Err(AuthExchangeError::StepError(e)),
    }
}

const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

pub async fn on_authentication<S>(
    conn: &mut Connection<S>,
    rsasl: std::sync::Arc<tokio::sync::Mutex<auth::Backend>>,
    rule_engine: std::sync::Arc<RuleEngine>,
    resolvers: std::sync::Arc<Resolvers>,
    mechanism: Mechanism,
    initial_response: Option<Vec<u8>>,
) -> Result<(), AuthExchangeError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + std::fmt::Debug,
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
                "An unsecured AUTH mechanism ({mechanism}) is used on a non-encrypted connection!"
            );
        } else {
            conn.send_code(CodeID::AuthMechanismMustBeEncrypted)
                .await
                .map_err(AuthExchangeError::SendingResponse)?;

            return Err(AuthExchangeError::AuthMechanismMustBeEncrypted(mechanism));
        }
    }

    if !mechanism.client_first() && initial_response.is_some() {
        conn.send_code(CodeID::AuthClientMustNotStart)
            .await
            .map_err(AuthExchangeError::SendingResponse)?;

        return Err(AuthExchangeError::AuthClientMustNotStart(mechanism));
    }

    let mut guard = rsasl.lock().await;
    let mut session = guard.server_start(&format!("{mechanism}")).unwrap();
    session.store(Box::new((
        rule_engine,
        resolvers,
        ConnectionContext {
            timestamp: conn.timestamp,
            credentials: None,
            is_authenticated: conn.is_authenticated,
            is_secured: conn.is_secured,
            server_name: conn.server_name.clone(),
            server_address: conn.server_addr,
        },
    )));

    let mut succeeded =
        auth_step(conn, &mut session, &initial_response.unwrap_or_default()).await?;

    while !succeeded {
        succeeded = match conn.read(READ_TIMEOUT).await {
            Ok(Some(buffer)) => {
                log::trace!("{buffer}");
                auth_step(conn, &mut session, buffer.as_bytes()).await
            }
            Ok(None) => Err(AuthExchangeError::ReadingMessage(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "unexpected EOF during SASL exchange",
            ))),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                Err(AuthExchangeError::Timeout(e))
            }
            Err(e) => Err(AuthExchangeError::ReadingMessage(e)),
        }?;
    }

    // TODO: if success get session property

    Ok(())
}
