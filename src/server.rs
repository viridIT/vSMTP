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
    config::{
        get_logger_config,
        server_config::{ServerConfig, TlsSecurityLevel},
    },
    connection::Connection,
    io_service::IoService,
    resolver::DataEndResolver,
    smtp::code::SMTPReplyCode,
    tls::get_rustls_config,
    transaction::Transaction,
};

pub struct ServerVSMTP {
    listener: tokio::net::TcpListener,
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    config: std::sync::Arc<ServerConfig>,
}

impl ServerVSMTP {
    pub async fn new(
        config: std::sync::Arc<ServerConfig>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        log4rs::init_config(get_logger_config(&config)?)?;

        Ok(Self {
            listener: tokio::net::TcpListener::bind(&config.server.addr).await?,
            tls_config: if config.tls.security_level == TlsSecurityLevel::None {
                None
            } else {
                Some(get_rustls_config(&config))
            },
            config,
        })
    }

    pub fn addr(&self) -> std::net::SocketAddr {
        self.listener
            .local_addr()
            .expect("cannot retrieve local address")
    }

    pub async fn listen_and_serve<R>(
        &self,
        resolver: std::sync::Arc<tokio::sync::Mutex<R>>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        R: DataEndResolver + std::marker::Send + std::marker::Sync + 'static,
    {
        loop {
            match self.listener.accept().await {
                Ok((stream, client_addr)) => {
                    log::warn!("Connection from: {}", client_addr);

                    let mut stream = stream.into_std()?;
                    stream.set_nonblocking(true)?;

                    let config = self.config.clone();
                    let tls_config = self.tls_config.as_ref().map(std::sync::Arc::clone);

                    let resolver = resolver.clone();

                    tokio::spawn(async move {
                        let begin = std::time::SystemTime::now();
                        log::warn!("Handling client: {}", client_addr);

                        let mut io_plain = IoService::new(&mut stream);

                        let mut conn = Connection::<std::net::TcpStream>::from_plain(
                            client_addr,
                            config.clone(),
                            &mut io_plain,
                        )?;
                        Self::handle_connection::<R, std::net::TcpStream>(
                            &mut conn, resolver, tls_config,
                        )
                        .await
                        .map(|_| {
                            log::warn!(
                                "{{ elapsed: {:?} }} Connection {} closed cleanly",
                                begin.elapsed(),
                                client_addr,
                            );
                        })
                        .map_err(|error| {
                            log::error!(
                                "{{ elapsed: {:?} }} Connection {} closed with an error {}",
                                begin.elapsed(),
                                client_addr,
                                error,
                            );
                            error
                        })
                    });
                }
                Err(e) => log::error!("Error accepting socket; error = {:?}", e),
            }
        }
    }

    fn is_version_requirement_satisfied(
        conn: &rustls::ServerConnection,
        config: &ServerConfig,
    ) -> bool {
        let protocol_version_requirement = config
            .tls
            .sni_maps
            .as_ref()
            .and_then(|map| {
                if let Some(sni) = conn.sni_hostname() {
                    for i in map {
                        if i.domain == sni {
                            return Some(i);
                        }
                    }
                }
                None
            })
            .and_then(|i| i.protocol_version.as_ref())
            .unwrap_or(&config.tls.protocol_version);

        conn.protocol_version()
            .map(|protocol_version| {
                protocol_version_requirement
                    .0
                    .iter()
                    .filter(|i| i.0 == protocol_version)
                    .count()
                    != 0
            })
            .unwrap_or(false)
    }

    pub async fn handle_connection<R, S>(
        conn: &mut Connection<'_, S>,
        resolver: std::sync::Arc<tokio::sync::Mutex<R>>,
        tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    ) -> Result<(), std::io::Error>
    where
        R: crate::resolver::DataEndResolver,
        S: std::io::Read + std::io::Write,
    {
        let mut helo_domain = None;

        conn.send_code(SMTPReplyCode::Code220)?;

        while conn.is_alive {
            match Transaction::receive(conn, &helo_domain).await? {
                crate::transaction::TransactionResult::Nothing => {}
                crate::transaction::TransactionResult::Mail(mail) => {
                    helo_domain = Some(mail.envelop.helo.clone());

                    // writing the context to the delivery queue.
                    let code = resolver
                        .lock()
                        .await
                        .on_data_end(&conn.config, &mail)
                        .await?;

                    conn.send_code(code)?;
                }
                crate::transaction::TransactionResult::TlsUpgrade if tls_config.is_none() => {
                    conn.send_code(SMTPReplyCode::Code454)?;
                    conn.send_code(SMTPReplyCode::Code221)?;
                    return Ok(());
                }
                crate::transaction::TransactionResult::TlsUpgrade => {
                    let mut tls_conn = rustls::ServerConnection::new(tls_config.unwrap()).unwrap();
                    let mut tls_stream = rustls::Stream::new(&mut tls_conn, &mut conn.io_stream);
                    let mut io_tls_stream = IoService::new(&mut tls_stream);

                    Connection::<IoService<'_, S>>::complete_tls_handshake(
                        &mut io_tls_stream,
                        &conn.config.tls.handshake_timeout,
                    )?;

                    let mut secured_conn = Connection {
                        timestamp: conn.timestamp,
                        is_alive: true,
                        config: conn.config.clone(),
                        client_addr: conn.client_addr,
                        error_count: conn.error_count,
                        is_secured: true,
                        io_stream: &mut io_tls_stream,
                    };

                    // FIXME: the rejection of the client because of the SSL/TLS protocol
                    // version is done after the handshake...

                    if !Self::is_version_requirement_satisfied(
                        secured_conn.io_stream.inner.conn,
                        &secured_conn.config,
                    ) {
                        log::error!("requirement not satisfied");
                        // TODO: send error 500 ?
                        return Ok(());
                    }

                    let mut secured_helo_domain = None;

                    while secured_conn.is_alive {
                        match Transaction::receive(&mut secured_conn, &secured_helo_domain).await? {
                            crate::transaction::TransactionResult::Nothing => {}
                            crate::transaction::TransactionResult::Mail(mail) => {
                                secured_helo_domain = Some(mail.envelop.helo.clone());

                                let code = resolver
                                    .lock()
                                    .await
                                    .on_data_end(&secured_conn.config, &mail)
                                    .await?;

                                secured_conn.send_code(code)?;
                            }
                            crate::transaction::TransactionResult::TlsUpgrade => todo!(),
                        }
                    }
                    return Ok(());
                }
            }
        }

        Ok(())
    }
}
