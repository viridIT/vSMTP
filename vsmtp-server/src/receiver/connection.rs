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
use super::io_service::{IoService, ReadError};
use vsmtp_common::code::SMTPReplyCode;
use vsmtp_config::{log_channel::RECEIVER, ServerConfig};

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

/// Instance containing connection to the server's information
pub struct Connection<'stream, S>
where
    S: std::io::Read + std::io::Write + Send,
{
    /// server's port
    pub kind: ConnectionKind,
    /// connection timestamp
    pub timestamp: std::time::SystemTime,
    /// is still alive
    pub is_alive: bool,
    /// server's configuration
    pub config: std::sync::Arc<ServerConfig>,
    /// peer socket address
    pub client_addr: std::net::SocketAddr,
    /// number of error the client made so far
    pub error_count: i64,
    /// is under tls (tunneled or opportunistic)
    pub is_secured: bool,
    /// abstraction of the stream
    pub io_stream: &'stream mut IoService<'stream, S>,
}

impl<S> Connection<'_, S>
where
    S: std::io::Read + std::io::Write + Send,
{
    ///
    pub fn from_plain<'a>(
        kind: ConnectionKind,
        client_addr: std::net::SocketAddr,
        config: std::sync::Arc<ServerConfig>,
        io_stream: &'a mut IoService<'a, S>,
    ) -> Connection<S> {
        Connection {
            kind,
            timestamp: std::time::SystemTime::now(),
            is_alive: true,
            config,
            client_addr,
            error_count: 0,
            is_secured: false,
            io_stream,
        }
    }
}

impl<S> Connection<'_, S>
where
    S: std::io::Read + std::io::Write + Send,
{
    /// send a reply code to the client
    ///
    /// # Errors
    pub fn send_code(&mut self, reply_to_send: SMTPReplyCode) -> anyhow::Result<()> {
        log::info!(target: RECEIVER, "send=\"{:?}\"", reply_to_send);

        if reply_to_send.is_error() {
            self.error_count += 1;

            let hard_error = self.config.smtp.error.hard_count;
            let soft_error = self.config.smtp.error.soft_count;

            if hard_error != -1 && self.error_count >= hard_error {
                let mut response_begin = self.config.reply_codes.get(&reply_to_send).to_string();
                response_begin.replace_range(3..4, "-");
                response_begin.push_str(
                    self.config
                        .reply_codes
                        .get(&SMTPReplyCode::Code451TooManyError),
                );
                std::io::Write::write_all(&mut self.io_stream, response_begin.as_bytes())?;

                anyhow::bail!("too many errors")
            }

            std::io::Write::write_all(
                &mut self.io_stream,
                self.config.reply_codes.get(&reply_to_send).as_bytes(),
            )?;

            if soft_error != -1 && self.error_count >= soft_error {
                std::thread::sleep(self.config.smtp.error.delay);
            }
        } else {
            std::io::Write::write_all(
                &mut self.io_stream,
                self.config.reply_codes.get(&reply_to_send).as_bytes(),
            )?;
        }
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
    ) -> Result<std::result::Result<std::string::String, ReadError>, tokio::time::error::Elapsed>
    {
        let future = self.io_stream.get_next_line_async();
        tokio::time::timeout(timeout, future).await
    }
}

impl<S> Connection<'_, S>
where
    S: std::io::Read + std::io::Write + Send,
{
    /// process a tls handshake
    ///
    /// # Errors
    pub fn complete_tls_handshake(
        io: &mut IoService<rustls::Stream<rustls::ServerConnection, &mut S>>,
        timeout: &std::time::Duration,
    ) -> Result<(), std::io::Error> {
        let begin_handshake = std::time::Instant::now();

        while io.inner.conn.is_handshaking() {
            if begin_handshake.elapsed() > *timeout {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "too long",
                ));
            }
            match std::io::Write::flush(&mut io.inner) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}
