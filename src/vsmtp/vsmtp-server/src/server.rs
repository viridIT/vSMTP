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
use crate::{
    auth,
    channel_message::ProcessMessage,
    log_channels,
    receiver::{
        handle_connection, {Connection, ConnectionKind},
    },
};
use vsmtp_common::{
    re::{anyhow, log, vsmtp_rsasl},
    CodeID,
};
use vsmtp_config::{get_rustls_config, re::rustls, Config};
use vsmtp_rule_engine::rule_engine::RuleEngine;

/// TCP/IP server
pub struct Server {
    listener: tokio::net::TcpListener,
    listener_submission: tokio::net::TcpListener,
    listener_submissions: tokio::net::TcpListener,
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    rsasl: Option<std::sync::Arc<tokio::sync::Mutex<auth::Backend>>>,
    config: std::sync::Arc<Config>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    working_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
    delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
}

impl Server {
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
        if !config.server.queues.dirpath.exists() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .create(&config.server.queues.dirpath)?;
        }

        if config.server.tls.is_none() {
            log::warn!(
                "No TLS configuration provided, listening on submissions protocol (port 465) will cause issue"
            );
        }

        Ok(Self {
            listener: tokio::net::TcpListener::from_std(sockets.0)?,
            listener_submission: tokio::net::TcpListener::from_std(sockets.1)?,
            listener_submissions: tokio::net::TcpListener::from_std(sockets.2)?,
            tls_config: if let Some(smtps) = &config.server.tls {
                Some(std::sync::Arc::new(get_rustls_config(
                    smtps,
                    &config.server.r#virtual,
                )?))
            } else {
                None
            },
            rsasl: if config.server.smtp.auth.is_some() {
                Some(std::sync::Arc::new(tokio::sync::Mutex::new({
                    let mut rsasl =
                        vsmtp_rsasl::SASL::new().map_err(|e| anyhow::anyhow!("{}", e))?;
                    rsasl.install_callback::<auth::Callback>();
                    rsasl.store(Box::new(config.clone()));
                    rsasl
                })))
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
    pub async fn listen_and_serve(self) -> anyhow::Result<()> {
        log::info!(
            target: log_channels::SERVER,
            "Listening on: {:?}",
            self.addr()
        );
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
            stream.set_nodelay(true)?;

            log::warn!(
                target: log_channels::SERVER,
                "Connection from: {:?}, {}",
                kind,
                client_addr
            );

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
                        .get(&CodeID::ConnectionMaxReached)
                        .unwrap()
                        .fold()
                        .as_bytes(),
                )
                .await
                {
                    log::warn!(target: log_channels::SERVER, "{}", e);
                }

                if let Err(e) = tokio::io::AsyncWriteExt::shutdown(&mut stream).await {
                    log::warn!(target: log_channels::SERVER, "{}", e);
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
                self.rsasl.clone(),
                self.rule_engine.clone(),
                self.working_sender.clone(),
                self.delivery_sender.clone(),
            );
            let client_counter_copy = client_counter.clone();
            tokio::spawn(async move {
                if let Err(e) = session.await {
                    log::warn!(target: log_channels::SERVER, "{}", e);
                }

                client_counter_copy.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            });
        }
    }

    ///
    /// # Errors
    #[allow(clippy::too_many_arguments)]
    pub async fn run_session(
        stream: tokio::net::TcpStream,
        client_addr: std::net::SocketAddr,
        kind: ConnectionKind,
        config: std::sync::Arc<Config>,
        tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
        rsasl: Option<std::sync::Arc<tokio::sync::Mutex<auth::Backend>>>,
        rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
        working_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
        delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
    ) -> anyhow::Result<()> {
        let begin = std::time::SystemTime::now();
        log::warn!(
            target: log_channels::SERVER,
            "Handling client: {}",
            client_addr
        );

        match handle_connection(
            &mut Connection::new(kind, client_addr, config.clone(), stream),
            tls_config,
            rsasl,
            rule_engine,
            &mut crate::receiver::MailHandler {
                working_sender,
                delivery_sender,
            },
        )
        .await
        {
            Ok(_) => {
                log::warn!(
                    target: log_channels::SERVER,
                    "{{ elapsed: {:?} }} Connection {} closed cleanly",
                    begin.elapsed(),
                    client_addr,
                );
                Ok(())
            }
            Err(error) => {
                log::error!(
                    target: log_channels::SERVER,
                    "{{ elapsed: {:?} }} Connection {} closed with an error {}",
                    begin.elapsed(),
                    client_addr,
                    error,
                );
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use vsmtp_rule_engine::rule_engine::RuleEngine;
    use vsmtp_test::config;

    use crate::{ProcessMessage, Server};

    macro_rules! listen_with {
        ($addr:expr, $addr_submission:expr, $addr_submissions:expr, $timeout:expr) => {{
            let config = std::sync::Arc::new({
                let mut config = config::local_test();
                config.server.interfaces.addr = $addr;
                config.server.interfaces.addr_submission = $addr_submission;
                config.server.interfaces.addr_submissions = $addr_submissions;
                config
            });

            let delivery = tokio::sync::mpsc::channel::<ProcessMessage>(
                config.server.queues.delivery.channel_size,
            );

            let working = tokio::sync::mpsc::channel::<ProcessMessage>(
                config.server.queues.working.channel_size,
            );

            let s = Server::new(
                config.clone(),
                (
                    std::net::TcpListener::bind(&config.server.interfaces.addr[..]).unwrap(),
                    std::net::TcpListener::bind(&config.server.interfaces.addr_submission[..])
                        .unwrap(),
                    std::net::TcpListener::bind(&config.server.interfaces.addr_submissions[..])
                        .unwrap(),
                ),
                std::sync::Arc::new(std::sync::RwLock::new(
                    RuleEngine::new(&config, &None).unwrap(),
                )),
                working.0,
                delivery.0,
            )
            .unwrap();

            println!("{:?}", s.addr());

            assert_eq!(
                s.addr(),
                [
                    config.server.interfaces.addr.clone(),
                    config.server.interfaces.addr_submission.clone(),
                    config.server.interfaces.addr_submissions.clone()
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
            );

            tokio::time::timeout(
                std::time::Duration::from_millis($timeout),
                s.listen_and_serve(),
            )
            .await
            .unwrap_err();
        }};
    }

    #[tokio::test]
    async fn basic() {
        listen_with![
            vec!["0.0.0.0:10026".parse().unwrap()],
            vec!["0.0.0.0:10588".parse().unwrap()],
            vec!["0.0.0.0:10466".parse().unwrap()],
            10
        ];
    }
}
