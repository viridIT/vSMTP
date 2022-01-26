/**
 * vSMTP mail transfer agent
 * Copyright (C) 2021 viridIT SAS
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
use crate::{
    config::{log_channel::RECEIVER, server_config::ServerConfig},
    io_service::{IoService, ReadError},
    smtp::code::SMTPReplyCode,
};

pub struct Connection<'stream, S>
where
    S: std::io::Read + std::io::Write,
{
    pub timestamp: std::time::SystemTime,
    pub is_alive: bool,
    pub config: std::sync::Arc<crate::config::server_config::ServerConfig>,
    pub client_addr: std::net::SocketAddr,
    pub error_count: u64,
    pub is_secured: bool,
    pub io_stream: &'stream mut IoService<'stream, S>,
}

impl<S> Connection<'_, S>
where
    S: std::io::Read + std::io::Write,
{
    pub fn from_plain<'a>(
        client_addr: std::net::SocketAddr,
        config: std::sync::Arc<ServerConfig>,
        io_stream: &'a mut IoService<'a, S>,
    ) -> std::io::Result<Connection<'a, S>> {
        Ok(Connection {
            timestamp: std::time::SystemTime::now(),
            is_alive: true,
            config,
            client_addr,
            error_count: 0,
            is_secured: false,
            io_stream,
        })
    }
}

impl<S> Connection<'_, S>
where
    S: std::io::Read + std::io::Write,
{
    pub fn send_code(&mut self, reply_to_send: SMTPReplyCode) -> anyhow::Result<()> {
        log::info!(target: RECEIVER, "send=\"{:?}\"", reply_to_send);

        if reply_to_send.is_error() {
            self.error_count += 1;

            let hard_error = self.config.smtp.error.hard_count;
            let soft_error = self.config.smtp.error.soft_count;

            if hard_error != -1 && self.error_count >= hard_error as u64 {
                let mut response_begin =
                    self.config.smtp.get_code().get(&reply_to_send).to_string();
                response_begin.replace_range(3..4, "-");
                response_begin.push_str(
                    self.config
                        .smtp
                        .get_code()
                        .get(&SMTPReplyCode::Code451TooManyError),
                );
                std::io::Write::write_all(&mut self.io_stream, response_begin.as_bytes())?;

                anyhow::bail!("too many errors")
            }

            std::io::Write::write_all(
                &mut self.io_stream,
                self.config.smtp.get_code().get(&reply_to_send).as_bytes(),
            )?;

            if soft_error != -1 && self.error_count >= soft_error as u64 {
                std::thread::sleep(self.config.smtp.error.delay);
            }
        } else {
            std::io::Write::write_all(
                &mut self.io_stream,
                self.config.smtp.get_code().get(&reply_to_send).as_bytes(),
            )?;
        }

        Ok(())
    }

    pub async fn read(
        &mut self,
        timeout: std::time::Duration,
    ) -> std::result::Result<
        std::result::Result<std::string::String, ReadError>,
        tokio::time::error::Elapsed,
    > {
        tokio::time::timeout(timeout, self.io_stream.get_next_line_async()).await
    }
}

impl<S> Connection<'_, S>
where
    S: std::io::Read + std::io::Write,
{
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
