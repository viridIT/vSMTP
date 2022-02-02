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
use std::collections::HashMap;

use crate::{
    config::server_config::{InnerTlsConfig, ServerConfig, TlsSecurityLevel},
    connection::Connection,
    io_service::IoService,
    model::mail::MailContext,
    processes::ProcessMessage,
    queue::Queue,
    resolver::{smtp_resolver::SMTPResolver, Resolver},
    smtp::code::SMTPReplyCode,
    tls::get_rustls_config,
    transaction::Transaction,
};

pub struct ServerVSMTP {
    resolvers: HashMap<String, Box<dyn Resolver + Send + Sync>>,
    listener: tokio::net::TcpListener,
    listener_submission: tokio::net::TcpListener,
    listener_submissions: tokio::net::TcpListener,
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    config: std::sync::Arc<ServerConfig>,
}

impl ServerVSMTP {
    pub async fn new(config: std::sync::Arc<ServerConfig>) -> anyhow::Result<Self> {
        let spool_dir =
            <std::path::PathBuf as std::str::FromStr>::from_str(&config.delivery.spool_dir)
                .unwrap();

        if !spool_dir.exists() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .create(spool_dir)?;
        }

        let mut resolvers = HashMap::<String, Box<dyn Resolver + Send + Sync>>::new();
        resolvers.insert("default".to_string(), Box::new(SMTPResolver));

        Ok(Self {
            resolvers,
            listener: tokio::net::TcpListener::bind(&config.server.addr).await?,
            listener_submission: tokio::net::TcpListener::bind(&config.server.addr_submission)
                .await?,
            listener_submissions: tokio::net::TcpListener::bind(&config.server.addr_submissions)
                .await?,
            tls_config: config.tls.as_ref().and_then(|smtps| {
                if smtps.security_level == TlsSecurityLevel::None {
                    None
                } else {
                    Some(get_rustls_config(&config.server.domain, smtps))
                }
            }),
            config,
        })
    }

    pub fn addr(&self) -> Vec<std::net::SocketAddr> {
        vec![
            self.listener
                .local_addr()
                .expect("cannot retrieve local address"),
            self.listener_submission
                .local_addr()
                .expect("cannot retrieve local address"),
            self.listener_submissions
                .local_addr()
                .expect("cannot retrieve local address"),
        ]
    }

    pub fn with_resolver<T>(&mut self, name: &str, resolver: T) -> &mut Self
    where
        T: Resolver + Send + Sync + 'static,
    {
        self.resolvers.insert(name.to_string(), Box::new(resolver));
        self
    }

    pub async fn listen_and_serve(&mut self) -> anyhow::Result<()> {
        let delivery_buffer_size = self
            .config
            .delivery
            .queues
            .get("delivery")
            .map(|q| q.capacity)
            .flatten()
            .unwrap_or(1);

        let (delivery_sender, delivery_receiver) =
            tokio::sync::mpsc::channel::<ProcessMessage>(delivery_buffer_size);

        let working_buffer_size = self
            .config
            .delivery
            .queues
            .get("working")
            .map(|q| q.capacity)
            .flatten()
            .unwrap_or(1);

        let (working_sender, working_receiver) =
            tokio::sync::mpsc::channel::<ProcessMessage>(working_buffer_size);

        let config_deliver = self.config.clone();
        let resolvers = std::mem::take(&mut self.resolvers);
        tokio::spawn(async move {
            let result =
                crate::processes::delivery::start(resolvers, config_deliver, delivery_receiver)
                    .await;
            log::error!("v_deliver ended unexpectedly '{:?}'", result);
        });

        let config_mime = self.config.clone();
        let mime_delivery_sender = delivery_sender.clone();
        tokio::spawn(async move {
            let result =
                crate::processes::mime::start(&config_mime, working_receiver, mime_delivery_sender)
                    .await;
            log::error!("v_mime ended unexpectedly '{:?}'", result);
        });

        let working_sender = std::sync::Arc::new(working_sender);
        let delivery_sender = std::sync::Arc::new(delivery_sender);

        loop {
            let (stream, client_addr, kind) = tokio::select! {
                Ok((stream, client_addr)) = self.listener.accept() => {
                    (stream, client_addr, crate::connection::Kind::Opportunistic)
                }
                Ok((stream, client_addr)) = self.listener_submission.accept() => {
                    (stream, client_addr, crate::connection::Kind::Submission)
                }
                Ok((stream, client_addr)) = self.listener_submissions.accept() => {
                    (stream, client_addr, crate::connection::Kind::Tunneled)
                }
            };

            log::warn!("Connection from: {:?}, {}", kind, client_addr);

            let mut stream = stream.into_std()?;
            stream.set_nonblocking(true)?;

            let config = self.config.clone();
            let tls_config = self.tls_config.clone();

            let working_sender = working_sender.clone();
            let delivery_sender = delivery_sender.clone();

            tokio::spawn(async move {
                let begin = std::time::SystemTime::now();
                log::warn!("Handling client: {}", client_addr);

                let mut io_plain = IoService::new(&mut stream);

                let mut conn = Connection::<std::net::TcpStream>::from_plain(
                    kind,
                    client_addr,
                    config.clone(),
                    &mut io_plain,
                )?;
                match conn.kind {
                    crate::connection::Kind::Opportunistic
                    | crate::connection::Kind::Submission => {
                        Self::handle_connection::<std::net::TcpStream>(
                            &mut conn,
                            working_sender,
                            delivery_sender,
                            tls_config,
                        )
                        .await
                    }
                    crate::connection::Kind::Tunneled => {
                        Self::handle_connection_secured(
                            &mut conn,
                            working_sender,
                            delivery_sender,
                            tls_config,
                        )
                        .await
                    }
                }
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
    }

    fn is_version_requirement_satisfied(
        conn: &rustls::ServerConnection,
        config: &InnerTlsConfig,
    ) -> bool {
        let protocol_version_requirement = config
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
            .unwrap_or(&config.protocol_version);

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

    async fn on_mail<S: std::io::Read + std::io::Write>(
        conn: &mut Connection<'_, S>,
        mail: Box<MailContext>,
        helo_domain: &mut Option<String>,
        working_sender: std::sync::Arc<tokio::sync::mpsc::Sender<ProcessMessage>>,
        delivery_sender: std::sync::Arc<tokio::sync::mpsc::Sender<ProcessMessage>>,
    ) -> anyhow::Result<()> {
        *helo_domain = Some(mail.envelop.helo.clone());

        match &mail.metadata {
            // quietly skipping mime & delivery processes when there is no resolver.
            // (in case of a quarantine for example)
            Some(metadata) if metadata.resolver == "none" => {
                log::warn!("delivery skipped due to NO_DELIVERY action call.");
                conn.send_code(SMTPReplyCode::Code250)?;
            }
            Some(metadata) if metadata.skipped.is_some() => {
                log::warn!("postq skipped due to {:?}.", metadata.skipped.unwrap());
                match Queue::Deliver.write_to_queue(&conn.config, &mail) {
                    Ok(_) => {
                        delivery_sender
                            .send(ProcessMessage {
                                message_id: mail.metadata.as_ref().unwrap().message_id.clone(),
                            })
                            .await?;

                        conn.send_code(SMTPReplyCode::Code250)?
                    }
                    Err(error) => {
                        log::error!("couldn't write to delivery queue: {}", error);
                        conn.send_code(SMTPReplyCode::Code554)?
                    }
                };
            }
            _ => {
                match Queue::Working.write_to_queue(&conn.config, &mail) {
                    Ok(_) => {
                        working_sender
                            .send(ProcessMessage {
                                message_id: mail.metadata.as_ref().unwrap().message_id.clone(),
                            })
                            .await?;

                        conn.send_code(SMTPReplyCode::Code250)?
                    }
                    Err(error) => {
                        log::error!("couldn't write to queue: {}", error);
                        conn.send_code(SMTPReplyCode::Code554)?
                    }
                };
            }
        };

        Ok(())
    }

    pub async fn handle_connection<S>(
        conn: &mut Connection<'_, S>,
        working_sender: std::sync::Arc<tokio::sync::mpsc::Sender<ProcessMessage>>,
        delivery_sender: std::sync::Arc<tokio::sync::mpsc::Sender<ProcessMessage>>,
        tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    ) -> anyhow::Result<()>
    where
        S: std::io::Read + std::io::Write,
    {
        let mut helo_domain = None;

        conn.send_code(SMTPReplyCode::Code220)?;

        while conn.is_alive {
            match Transaction::receive(conn, &helo_domain).await? {
                crate::transaction::TransactionResult::Nothing => {}
                crate::transaction::TransactionResult::Mail(mail) => {
                    Self::on_mail(
                        conn,
                        mail,
                        &mut helo_domain,
                        working_sender.clone(),
                        delivery_sender.clone(),
                    )
                    .await?;
                }
                crate::transaction::TransactionResult::TlsUpgrade if tls_config.is_none() => {
                    conn.send_code(SMTPReplyCode::Code454)?;
                    conn.send_code(SMTPReplyCode::Code221)?;
                    return Ok(());
                }
                crate::transaction::TransactionResult::TlsUpgrade => {
                    return Self::handle_connection_secured(
                        conn,
                        working_sender.clone(),
                        delivery_sender.clone(),
                        tls_config.clone(),
                    )
                    .await;
                }
            }
        }

        Ok(())
    }

    pub async fn handle_connection_secured<S>(
        conn: &mut Connection<'_, S>,
        working_sender: std::sync::Arc<tokio::sync::mpsc::Sender<ProcessMessage>>,
        delivery_sender: std::sync::Arc<tokio::sync::mpsc::Sender<ProcessMessage>>,
        tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    ) -> anyhow::Result<()>
    where
        S: std::io::Read + std::io::Write,
    {
        let smtps_config = conn.config.tls.as_ref().unwrap();

        let mut tls_conn = rustls::ServerConnection::new(tls_config.unwrap()).unwrap();
        let mut tls_stream = rustls::Stream::new(&mut tls_conn, &mut conn.io_stream);
        let mut io_tls_stream = IoService::new(&mut tls_stream);

        Connection::<IoService<'_, S>>::complete_tls_handshake(
            &mut io_tls_stream,
            &smtps_config.handshake_timeout,
        )?;

        let mut secured_conn = Connection {
            kind: conn.kind,
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

        if !Self::is_version_requirement_satisfied(secured_conn.io_stream.inner.conn, smtps_config)
        {
            log::error!("requirement not satisfied");
            // TODO: send error 500 ?
            return Ok(());
        }

        if let crate::connection::Kind::Tunneled = secured_conn.kind {
            secured_conn.send_code(SMTPReplyCode::Code220)?;
        }

        let mut helo_domain = None;

        while secured_conn.is_alive {
            match Transaction::receive(&mut secured_conn, &helo_domain).await? {
                crate::transaction::TransactionResult::Nothing => {}
                crate::transaction::TransactionResult::Mail(mail) => {
                    Self::on_mail(
                        &mut secured_conn,
                        mail,
                        &mut helo_domain,
                        working_sender.clone(),
                        delivery_sender.clone(),
                    )
                    .await?;
                }
                crate::transaction::TransactionResult::TlsUpgrade => todo!(),
            }
        }
        Ok(())
    }
}
