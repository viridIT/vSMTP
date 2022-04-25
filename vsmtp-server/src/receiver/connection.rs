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
use crate::log_channels;
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

///
#[derive(Debug)]
pub struct AbstractIO<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin,
{
    ///
    pub inner: S,
    buffer: Vec<u8>,
    //    read: usize,
}

impl<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin> std::future::Future
    for AbstractIO<S>
{
    type Output = std::io::Result<String>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        loop {
            let (done, used) = {
                let mut temp = self.buffer.clone();
                let mut buf = tokio::io::ReadBuf::new(&mut temp);
                match std::pin::Pin::new(&mut self.inner).poll_read(cx, &mut buf) {
                    std::task::Poll::Ready(t) => t,
                    std::task::Poll::Pending => return std::task::Poll::Pending,
                }?;
                let available = buf.filled();
                let needle = b"\r\n";

                if let Some(i) = available
                    .windows(needle.len())
                    .position(|window| window == needle)
                {
                    self.buffer.extend_from_slice(&available[..i + 1]);
                    (true, i + 2)
                } else {
                    self.buffer.extend_from_slice(available);
                    (false, available.len())
                }
            };
            // self.inner.consume(used);
            // self.read += used;
            if done || used == 0 {
                let out = self.buffer.drain(..used).collect::<Vec<_>>();
                return std::task::Poll::Ready(Ok(String::from_utf8(out).unwrap()));
            }
        }
    }
}

/*
impl<S> tokio::io::AsyncRead for AbstractIO<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin,
{
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<S> tokio::io::AsyncWrite for AbstractIO<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin,
{
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        std::pin::Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
*/

/*
impl<S> tokio::io::AsyncBufRead for AbstractIO<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin,
{
    fn poll_fill_buf(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<&[u8]>> {
        let this = self.get_mut();
        println!("{:?}", std::str::from_utf8(this.buffer.as_slice()).unwrap());
        if this.buffer.is_empty() {
            let mut raw = vec![0; 100];
            let mut buf = tokio::io::ReadBuf::new(&mut raw);
            let res = dbg!(tokio::io::AsyncRead::poll_read(
                std::pin::Pin::new(&mut this.inner),
                cx,
                &mut buf
            ));

            match res {
                std::task::Poll::Pending => return std::task::Poll::Pending,
                std::task::Poll::Ready(Ok(_)) => this.buffer = buf.filled().to_vec(),
                std::task::Poll::Ready(Err(e)) => return std::task::Poll::Ready(Err(e)),
            };
        }
        std::task::Poll::Ready(Ok(&this.buffer))
    }

    fn consume(mut self: std::pin::Pin<&mut Self>, amt: usize) {
        println!(
            "before con: {:?}",
            std::str::from_utf8(self.buffer.as_slice()).unwrap()
        );
        self.buffer.drain(..amt);
        // self.buffer = self.buffer[amt..].to_vec();
        println!(
            "after  con: {:?}",
            std::str::from_utf8(self.buffer.as_slice()).unwrap()
        );
    }
}
*/

impl<S> AbstractIO<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin,
{
    ///
    pub fn new(stream: S) -> Self {
        Self {
            inner: stream,
            buffer: Vec::new(),
            // read: 0,
        }
    }

    /*
    async fn inner_next_line(&mut self) -> std::io::Result<String> {
        let mut line = String::new();

        let mut temp = Vec::with_capacity(1000);

        loop {
            dbg!(&line);
            match line.find("\r\n") {
                Some(pos) => {
                    self.buffer = line[pos..].as_bytes().to_vec();
                    return Ok(line[..pos].to_string());
                }
                None => {
                    match tokio::io::AsyncReadExt::read(&mut self.inner, &mut temp).await? {
                        0 => {
                            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, ""));
                        }
                        size_read => {
                            dbg!(&line);

                            line.push_str(std::str::from_utf8(&temp[..size_read]).map_err(
                                |e| std::io::Error::new(std::io::ErrorKind::InvalidData, e),
                            )?);

                            temp = temp[size_read..].to_vec();
                        }
                    };
                }
            }
        }
    }
    */

    ///
    /// # Errors
    ///
    pub async fn next_line(
        &mut self,
        timeout: Option<std::time::Duration>,
    ) -> std::io::Result<String> {
        tokio::time::timeout(
            timeout.unwrap_or(std::time::Duration::from_millis(100)),
            self,
        )
        .await
        .map_err(|t| std::io::Error::new(std::io::ErrorKind::TimedOut, t))?
    }
}

/// Instance containing connection to the server's information
pub struct Connection<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin,
{
    /// server's port
    pub kind: ConnectionKind,
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
    /// abstraction of the stream
    pub io_stream: AbstractIO<S>,
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
        io_stream: S,
    ) -> Self {
        Self {
            kind,
            timestamp: std::time::SystemTime::now(),
            is_alive: true,
            config,
            client_addr,
            error_count: 0,
            is_secured: false,
            io_stream: AbstractIO::new(io_stream),
            is_authenticated: false,
            authentication_attempt: 0,
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
                &mut self.io_stream.inner,
                self.config
                    .server
                    .smtp
                    .codes
                    .get(&reply_to_send)
                    .unwrap()
                    .as_bytes(),
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
                &mut self.io_stream.inner,
                self.config
                    .server
                    .smtp
                    .codes
                    .get(&reply_to_send)
                    .unwrap()
                    .as_bytes(),
            )
            .await?;
        }
        Ok(())
    }

    /// Send a buffer
    ///
    /// # Errors
    ///
    /// * internal connection writer error
    pub async fn send(&mut self, reply: &str) -> anyhow::Result<()> {
        log::info!(target: log_channels::CONNECTION, "send=\"{}\"", reply);
        tokio::io::AsyncWriteExt::write_all(&mut self.io_stream.inner, reply.as_bytes())
            .await
            .map_err(anyhow::Error::new)
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
    ) -> std::io::Result<std::string::String> {
        self.io_stream.next_line(Some(timeout)).await
    }
}
