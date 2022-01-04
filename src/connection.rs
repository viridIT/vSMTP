use crate::{
    config::{log::RECEIVER, server_config::ServerConfig},
    mailprocessing::io_service::{IoService, ReadError},
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
    pub tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    /*
    io_tls_stream: Option<
        IoService<
            'stream,
            rustls::Stream<
                'stream,
                rustls::ServerConnection,
                IoService<'stream, std::net::TcpStream>,
            >,
        >,
    >,
    */
}

impl<S> Connection<'_, S>
where
    S: std::io::Read + std::io::Write,
{
    pub fn from_plain<'a>(
        client_addr: std::net::SocketAddr,
        config: std::sync::Arc<ServerConfig>,
        io_stream: &'a mut IoService<'a, std::net::TcpStream>,
        tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    ) -> std::io::Result<Connection<'a, std::net::TcpStream>> {
        Ok(Connection {
            timestamp: std::time::SystemTime::now(),
            is_alive: true,
            config,
            client_addr,
            error_count: 0,
            is_secured: false,
            io_stream,
            tls_config,
        })
    }
}

impl<S> Connection<'_, S>
where
    S: std::io::Read + std::io::Write,
{
    pub fn upgrade_tls<'a, 'b>(
        &self,
        io_tls_stream: &'a mut IoService<
            'a,
            rustls::Stream<'a, rustls::ServerConnection, &mut IoService<'a, S>>,
        >,
    ) -> std::io::Result<
        Connection<'a, rustls::Stream<'a, rustls::ServerConnection, &'b mut IoService<'a, S>>>,
    > {
        Self::complete_tls_handshake::<IoService<'a, S>>(
            io_tls_stream,
            &self.config.tls.handshake_timeout,
        )?;

        Ok(Connection {
            timestamp: std::time::SystemTime::now(),
            is_alive: true,
            config: self.config,
            client_addr: self.client_addr,
            error_count: self.error_count,
            is_secured: true,
            io_stream: io_tls_stream,
            tls_config: self.tls_config,
        })
    }
}

impl<S> Connection<'_, S>
where
    S: std::io::Read + std::io::Write,
{
    pub fn send_code(&mut self, reply_to_send: SMTPReplyCode) -> Result<(), std::io::Error> {
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

                return Err(std::io::Error::new(
                    std::io::ErrorKind::ConnectionAborted,
                    "too many errors",
                ));
            }

            std::io::Write::write_all(
                &mut self.io_stream,
                self.config.smtp.get_code().get(&reply_to_send).as_bytes(),
            )?;

            if soft_error != -1 && self.error_count >= soft_error as u64 {
                std::thread::sleep(self.config.smtp.error.delay);
            }

            Ok(())
        } else {
            std::io::Write::write_all(
                &mut self.io_stream,
                self.config.smtp.get_code().get(&reply_to_send).as_bytes(),
            )
        }
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
    fn complete_tls_handshake<SS>(
        io: &mut IoService<rustls::Stream<rustls::ServerConnection, &mut SS>>,
        timeout: &std::time::Duration,
    ) -> Result<(), std::io::Error>
    where
        SS: std::io::Read + std::io::Write,
    {
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
