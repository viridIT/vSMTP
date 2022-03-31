use crate::auth;

use super::Connection;
use vsmtp_common::{
    auth::Mechanism,
    code::SMTPReplyCode,
    re::{anyhow, log, rsasl},
};

/// Result of the AUTH command
#[allow(clippy::pedantic)]
#[must_use]
pub enum AuthExchangeError {
    /// authentication invalid
    Failed,
    /// the client stopped the exchange
    Canceled,
    /// timeout of the server
    Timeout(tokio::time::error::Elapsed),
    ///
    InvalidBase64,
    ///
    Other(anyhow::Error),
}

// /// authentication exchange is still processing
// Processing,
// /// authentication valid
// Success,

fn auth_step<S>(
    conn: &mut Connection<'_, S>,
    session: &mut rsasl::DiscardOnDrop<rsasl::Session<()>>,
    buffer: &[u8],
) -> Result<bool, AuthExchangeError>
where
    S: std::io::Read + std::io::Write + Send,
{
    if buffer == [b'*'] {
        return Err(AuthExchangeError::Canceled);
    }

    let bytes64decoded = base64::decode(buffer).map_err(|_| AuthExchangeError::InvalidBase64)?;

    match session.step(&bytes64decoded) {
        Ok(rsasl::Step::Done(buffer)) => {
            if !buffer.is_empty() {
                // TODO: send buffer ?
                println!(
                    "Authentication successful, bytes to return to client: {:?}",
                    std::str::from_utf8(&*buffer)
                );
            }

            conn.send_code(SMTPReplyCode::AuthSucceeded)
                .map_err(AuthExchangeError::Other)?;
            Ok(true)
        }
        Ok(rsasl::Step::NeedsMore(buffer)) => {
            let reply = format!(
                "334 {}\r\n",
                base64::encode(std::str::from_utf8(&*buffer).unwrap())
            );

            conn.send(&reply).map_err(AuthExchangeError::Other)?;
            Ok(false)
        }
        Err(e) if e.matches(rsasl::ReturnCode::GSASL_AUTHENTICATION_ERROR) => {
            Err(AuthExchangeError::Failed)
        }
        Err(e) => Err(AuthExchangeError::Other(anyhow::anyhow!("{}", e))),
    }
}

pub async fn on_authentication<S>(
    conn: &mut Connection<'_, S>,
    rsasl: std::sync::Arc<tokio::sync::Mutex<auth::Backend>>,
    mechanism: Mechanism,
    initial_response: Option<Vec<u8>>,
) -> Result<(), AuthExchangeError>
where
    S: std::io::Read + std::io::Write + Send,
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
            conn.send_code(SMTPReplyCode::AuthMechanismMustBeEncrypted)
                .map_err(AuthExchangeError::Other)?;

            return Err(AuthExchangeError::Other(anyhow::anyhow!(
                SMTPReplyCode::AuthMechanismMustBeEncrypted.to_string()
            )));
        }
    }

    if !mechanism.client_first() && initial_response.is_some() {
        conn.send_code(SMTPReplyCode::AuthClientMustNotStart)
            .map_err(AuthExchangeError::Other)?;

        return Err(AuthExchangeError::Other(anyhow::anyhow!(
            SMTPReplyCode::AuthClientMustNotStart.to_string()
        )));
    }
    let mut guard = rsasl.lock().await;
    let mut session = guard.server_start(&String::from(mechanism)).unwrap();

    let mut succeeded = auth_step(conn, &mut session, &initial_response.unwrap_or_default())?;

    while !succeeded {
        succeeded = match conn.read(std::time::Duration::from_secs(1)).await {
            Ok(Ok(buffer)) => {
                log::trace!("{}", buffer);
                auth_step(conn, &mut session, buffer.as_bytes())
            }
            Ok(Err(e)) => Err(AuthExchangeError::Other(anyhow::anyhow!("{:?}", e))),
            Err(e) => Err(AuthExchangeError::Timeout(e)),
        }?;
    }

    // TODO: if success get session property

    Ok(())
}
