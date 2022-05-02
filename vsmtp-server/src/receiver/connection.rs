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
// use super::io_service::{IoService, ReadError};
use crate::{log_channels, AbstractIO};
use vsmtp_common::{
    code::SMTPReplyCode,
    re::{anyhow, log},
};
use vsmtp_config::Config;

/// how the server would react to tls interaction for this connection
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Copy, Clone)]
pub enum ConnectionKind {
    /// connection may use STARTTLS
    Opportunistic,
    /// Opportunistic and enforced security (auth)
    Submission,
    /// within TLS
    Tunneled,
}

// TODO:? merge with [`ConnectionContext`]
/// Instance containing connection to the server's information
pub struct Connection<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin,
{
    /// server's port
    pub kind: ConnectionKind,
    /// server's domain of the connection, (from config.server.domain or sni)
    pub server_name: String,
    /// connection timestamp
    pub timestamp: std::time::SystemTime,
    /// is still alive
    pub is_alive: bool,
    /// server's configuration
    pub config: std::sync::Arc<Config>,
    /// peer socket address
    pub client_addr: std::net::SocketAddr,
    /// number of error the client made so far
    pub error_count: i64,
    /// is under tls (tunneled or opportunistic)
    pub is_secured: bool,
    /// has completed SASL challenge (AUTH)
    pub is_authenticated: bool,
    /// number of time the AUTH command has been received (and failed)
    pub authentication_attempt: i64,
    /// inner stream
    pub inner: AbstractIO<S>,
}

impl<S> Connection<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin,
{
    ///
    pub fn new(
        kind: ConnectionKind,
        client_addr: std::net::SocketAddr,
        config: std::sync::Arc<Config>,
        inner: S,
    ) -> Self {
        Self {
            kind,
            server_name: config.server.domain.clone(),
            timestamp: std::time::SystemTime::now(),
            is_alive: true,
            config,
            client_addr,
            error_count: 0,
            is_secured: false,
            inner: AbstractIO::new(inner),
            is_authenticated: false,
            authentication_attempt: 0,
        }
    }

    ///
    #[allow(clippy::too_many_arguments)]
    pub fn new_with(
        kind: ConnectionKind,
        server_name: String,
        timestamp: std::time::SystemTime,
        is_alive: bool,
        config: std::sync::Arc<Config>,
        client_addr: std::net::SocketAddr,
        error_count: i64,
        is_secured: bool,
        is_authenticated: bool,
        authentication_attempt: i64,
        inner: S,
    ) -> Self {
        Self {
            kind,
            server_name,
            timestamp,
            is_alive,
            config,
            client_addr,
            error_count,
            is_secured,
            is_authenticated,
            authentication_attempt,
            inner: AbstractIO::new(inner),
        }
    }
}

impl<S> Connection<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin,
{
    /// send a reply code to the client
    ///
    /// # Errors
    ///
    /// # Panics
    ///
    /// * a smtp code is missing, and thus config is ill-formed
    pub async fn send_code(&mut self, reply_to_send: SMTPReplyCode) -> anyhow::Result<()> {
        if reply_to_send.is_error() {
            self.error_count += 1;

            let hard_error = self.config.server.smtp.error.hard_count;
            let soft_error = self.config.server.smtp.error.soft_count;

            if hard_error != -1 && self.error_count >= hard_error {
                let mut response_begin = self
                    .config
                    .server
                    .smtp
                    .codes
                    .get(&reply_to_send)
                    .unwrap()
                    .to_string();
                response_begin.replace_range(3..4, "-");
                response_begin.push_str(
                    self.config
                        .server
                        .smtp
                        .codes
                        .get(&SMTPReplyCode::Code451TooManyError)
                        .unwrap(),
                );
                self.send(&response_begin).await?;

                anyhow::bail!("too many errors")
            }
            log::info!(
                target: log_channels::CONNECTION,
                "send=\"{:?}\"",
                reply_to_send
            );

            tokio::io::AsyncWriteExt::write_all(
                &mut self.inner.inner,
                match &reply_to_send {
                    SMTPReplyCode::Custom(message) => message.as_bytes(),
                    _ => self
                        .config
                        .server
                        .smtp
                        .codes
                        .get(&reply_to_send)
                        .unwrap()
                        .as_bytes(),
                },
            )
            .await?;

            if soft_error != -1 && self.error_count >= soft_error {
                std::thread::sleep(self.config.server.smtp.error.delay);
            }
        } else {
            log::info!(
                target: log_channels::CONNECTION,
                "send=\"{:?}\"",
                reply_to_send
            );

            tokio::io::AsyncWriteExt::write_all(
                &mut self.inner.inner,
                match &reply_to_send {
                    SMTPReplyCode::Custom(message) => message.as_bytes(),
                    _ => self
                        .config
                        .server
                        .smtp
                        .codes
                        .get(&reply_to_send)
                        .unwrap()
                        .as_bytes(),
                },
            )
            .await?;
        }
        tokio::io::AsyncWriteExt::flush(&mut self.inner.inner).await?;
        Ok(())
    }

    /// Send a buffer
    ///
    /// # Errors
    ///
    /// * internal connection writer error
    pub async fn send(&mut self, reply: &str) -> anyhow::Result<()> {
        log::info!(target: log_channels::CONNECTION, "send=\"{}\"", reply);
        tokio::io::AsyncWriteExt::write_all(&mut self.inner.inner, reply.as_bytes()).await?;
        tokio::io::AsyncWriteExt::flush(&mut self.inner.inner).await?;
        Ok(())
    }

    /// read a line from the client
    ///
    /// # Errors
    ///
    /// * timed-out
    /// * stream's error
    pub async fn read(
        &mut self,
        timeout: std::time::Duration,
    ) -> std::io::Result<Option<std::string::String>> {
        self.inner.next_line(Some(timeout)).await
    }
}
