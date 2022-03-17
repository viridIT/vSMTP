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
use crate::{
    processes::ProcessMessage,
    receiver::{
        handle_connection, handle_connection_secured, IoService, {Connection, ConnectionKind},
    },
};
use vsmtp_common::code::SMTPReplyCode;
use vsmtp_config::{rustls_helper::get_rustls_config, Config};
use vsmtp_rule_engine::rule_engine::RuleEngine;

/// TCP/IP server
#[allow(clippy::module_name_repetitions)]
pub struct ServerVSMTP {
    listener: tokio::net::TcpListener,
    listener_submission: tokio::net::TcpListener,
    listener_submissions: tokio::net::TcpListener,
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    config: std::sync::Arc<Config>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    working_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
    delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
}

impl ServerVSMTP {
    /// Create a server with the configuration provided, and the sockets already bound
    ///
    /// # Errors
    ///
    /// * `spool_dir` does not exist and failed to be created
    /// * cannot convert sockets to [tokio::net::TcpListener]
    /// * cannot initialize [rustls] config
    pub fn new(
        config: std::sync::Arc<Config>,
        sockets: (
            std::net::TcpListener,
            std::net::TcpListener,
            std::net::TcpListener,
        ),
        rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
        working_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
        delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
    ) -> anyhow::Result<Self> {
        // NOTE: move that in config builder ?
        if !config.server.queues.dirpath.exists() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .create(&config.server.queues.dirpath)?;
        }

        Ok(Self {
            listener: tokio::net::TcpListener::from_std(sockets.0)?,
            listener_submission: tokio::net::TcpListener::from_std(sockets.1)?,
            listener_submissions: tokio::net::TcpListener::from_std(sockets.2)?,
            tls_config: if let Some(smtps) = &config.server.tls {
                Some(std::sync::Arc::new(get_rustls_config(smtps)?))
            } else {
                None
            },
            config,
            rule_engine,
            working_sender,
            delivery_sender,
        })
    }

    /// Get the local address of the tcp listener
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

    /// Main loop of vSMTP's server
    ///
    /// # Errors
    ///
    /// * failed to initialize the [RuleEngine]
    ///
    /// # Panics
    ///
    /// * [tokio::spawn]
    /// * [tokio::select]
    #[allow(clippy::too_many_lines)]
    pub async fn listen_and_serve(&mut self) -> anyhow::Result<()> {
        let client_counter = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));

        loop {
            let (mut stream, client_addr, kind) = tokio::select! {
                Ok((stream, client_addr)) = self.listener.accept() => {
                    (stream, client_addr, ConnectionKind::Opportunistic)
                }
                Ok((stream, client_addr)) = self.listener_submission.accept() => {
                    (stream, client_addr, ConnectionKind::Submission)
                }
                Ok((stream, client_addr)) = self.listener_submissions.accept() => {
                    (stream, client_addr, ConnectionKind::Tunneled)
                }
            };
            log::warn!("Connection from: {:?}, {}", kind, client_addr);

            if self.config.server.client_count_max != -1
                && client_counter.load(std::sync::atomic::Ordering::SeqCst)
                    >= self.config.server.client_count_max
            {
                if let Err(e) = tokio::io::AsyncWriteExt::write_all(
                    &mut stream,
                    self.config
                        .server
                        .smtp
                        .codes
                        .get(&SMTPReplyCode::ConnectionMaxReached)
                        .unwrap()
                        .as_bytes(),
                )
                .await
                {
                    log::warn!("{}", e);
                }

                if let Err(e) = tokio::io::AsyncWriteExt::shutdown(&mut stream).await {
                    log::warn!("{}", e);
                }
                continue;
            }

            client_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            let session = Self::run_session(
                stream,
                client_addr,
                kind,
                self.config.clone(),
                self.tls_config.clone(),
                self.rule_engine.clone(),
                self.working_sender.clone(),
                self.delivery_sender.clone(),
            );
            let client_counter_copy = client_counter.clone();
            tokio::spawn(async move {
                if let Err(e) = session.await {
                    log::warn!("{}", e);
                }

                client_counter_copy.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn run_session(
        stream: tokio::net::TcpStream,
        client_addr: std::net::SocketAddr,
        kind: ConnectionKind,
        config: std::sync::Arc<Config>,
        tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
        rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
        working_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
        delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
    ) -> anyhow::Result<()> {
        let mut stream = stream.into_std()?;

        let begin = std::time::SystemTime::now();
        log::warn!("Handling client: {}", client_addr);

        let mut io_plain = IoService::new(&mut stream);

        let mut conn = Connection::<std::net::TcpStream>::from_plain(
            kind,
            client_addr,
            config.clone(),
            &mut io_plain,
        );
        match conn.kind {
            ConnectionKind::Opportunistic | ConnectionKind::Submission => {
                handle_connection(
                    &mut conn,
                    tls_config,
                    rule_engine,
                    working_sender,
                    delivery_sender,
                )
                .await
            }
            ConnectionKind::Tunneled => {
                handle_connection_secured(
                    &mut conn,
                    tls_config,
                    rule_engine,
                    working_sender,
                    delivery_sender,
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
    }
}
