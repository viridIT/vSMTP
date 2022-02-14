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
use std::sync::{Arc, RwLock};

use crate::{
    config::server_config::ServerConfig,
    processes::ProcessMessage,
    receiver::{
        connection::{Connection, ConnectionKind},
        handle_connection, handle_connection_secured,
        io_service::IoService,
    },
    resolver::{smtp_resolver::SMTPResolver, Resolver},
    rules::rule_engine::RuleEngine,
    tls_helpers::get_rustls_config,
};

pub struct ServerVSMTP {
    resolvers: std::collections::HashMap<String, Box<dyn Resolver + Send + Sync>>,
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

        let mut resolvers =
            std::collections::HashMap::<String, Box<dyn Resolver + Send + Sync>>::new();
        resolvers.insert("default".to_string(), Box::new(SMTPResolver));

        Ok(Self {
            resolvers,
            listener: tokio::net::TcpListener::bind(&config.server.addr).await?,
            listener_submission: tokio::net::TcpListener::bind(&config.server.addr_submission)
                .await?,
            listener_submissions: tokio::net::TcpListener::bind(&config.server.addr_submissions)
                .await?,
            tls_config: if let Some(smtps) = &config.smtps {
                Some(std::sync::Arc::new(get_rustls_config(smtps)?))
            } else {
                None
            },
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

        let rule_engine = Arc::new(RwLock::new(RuleEngine::new(
            self.config.rules.dir.as_str(),
        )?));

        let re_delivery = rule_engine.clone();
        let config_deliver = self.config.clone();
        let resolvers = std::mem::take(&mut self.resolvers);
        tokio::spawn(async move {
            let result = crate::processes::delivery::start(
                config_deliver,
                re_delivery,
                resolvers,
                delivery_receiver,
            )
            .await;
            log::error!("v_deliver ended unexpectedly '{:?}'", result);
        });

        let re_mime = rule_engine.clone();
        let config_mime = self.config.clone();
        let mime_delivery_sender = delivery_sender.clone();
        tokio::spawn(async move {
            let result = crate::processes::mime::start(
                &config_mime,
                re_mime,
                working_receiver,
                mime_delivery_sender,
            )
            .await;
            log::error!("v_mime ended unexpectedly '{:?}'", result);
        });

        let working_sender = std::sync::Arc::new(working_sender);
        let delivery_sender = std::sync::Arc::new(delivery_sender);

        loop {
            let (stream, client_addr, kind) = tokio::select! {
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

            let re_smtp = rule_engine.clone();
            tokio::spawn(Self::run_session(
                stream,
                client_addr,
                kind,
                self.config.clone(),
                self.tls_config.clone(),
                re_smtp,
                working_sender.clone(),
                delivery_sender.clone(),
            ));
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn run_session(
        stream: tokio::net::TcpStream,
        client_addr: std::net::SocketAddr,
        kind: ConnectionKind,
        config: std::sync::Arc<ServerConfig>,
        tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
        rule_engine: Arc<RwLock<RuleEngine>>,
        working_sender: std::sync::Arc<tokio::sync::mpsc::Sender<ProcessMessage>>,
        delivery_sender: std::sync::Arc<tokio::sync::mpsc::Sender<ProcessMessage>>,
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
        )?;
        match conn.kind {
            ConnectionKind::Opportunistic | ConnectionKind::Submission => {
                handle_connection::<std::net::TcpStream>(
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

#[cfg(test)]
mod tests {
    use crate::config::server_config::TlsSecurityLevel;

    use super::*;

    #[tokio::test]
    async fn init_server_valid() {
        // NOTE: using debug port + 1 in case of a debug server running elsewhere
        let (addr, addr_submission, addr_submissions) = (
            "0.0.0.0:10026".parse().expect("valid address"),
            "0.0.0.0:10588".parse().expect("valid address"),
            "0.0.0.0:10466".parse().expect("valid address"),
        );

        let config = ServerConfig::builder()
            .with_server(
                "test.server.com",
                addr,
                addr_submission,
                addr_submissions,
                num_cpus::get(),
            )
            .without_log()
            .without_smtps()
            .with_default_smtp()
            .with_delivery("./tmp/trash", crate::collection! {})
            .with_rules("./tmp/no_rules")
            .with_default_reply_codes()
            .build();

        let s = ServerVSMTP::new(std::sync::Arc::new(config)).await.unwrap();
        assert_eq!(s.addr(), vec![addr, addr_submission, addr_submissions]);
    }

    #[tokio::test]
    async fn init_server_secured_valid() {
        // NOTE: using debug port + 1 in case of a debug server running elsewhere
        let (addr, addr_submission, addr_submissions) = (
            "0.0.0.0:10026".parse().expect("valid address"),
            "0.0.0.0:10588".parse().expect("valid address"),
            "0.0.0.0:10466".parse().expect("valid address"),
        );

        let config = ServerConfig::builder()
            .with_server(
                "test.server.com",
                addr,
                addr_submission,
                addr_submissions,
                num_cpus::get(),
            )
            .without_log()
            .with_safe_default_smtps(
                TlsSecurityLevel::May,
                "./src/receiver/tests/certs/certificate.crt",
                "./src/receiver/tests/certs/privateKey.key",
                None,
            )
            .with_default_smtp()
            .with_delivery("./tmp/trash", crate::collection! {})
            .with_rules("./tmp/no_rules")
            .with_default_reply_codes()
            .build();

        let s = ServerVSMTP::new(std::sync::Arc::new(config)).await.unwrap();
        assert_eq!(s.addr(), vec![addr, addr_submission, addr_submissions]);
    }
}
