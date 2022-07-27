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
    receiver::{Connection, MailHandler},
};
use vsmtp_common::{
    re::{
        anyhow::{self, Context},
        log, tokio, vsmtp_rsasl,
    },
    CodeID, ConnectionKind,
};
use vsmtp_config::{get_rustls_config, re::rustls, Config, Resolvers};
use vsmtp_rule_engine::RuleEngine;

/// TCP/IP server
pub struct Server {
    config: std::sync::Arc<Config>,
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    rsasl: Option<std::sync::Arc<tokio::sync::Mutex<auth::Backend>>>,
    rule_engine: std::sync::Arc<RuleEngine>,
    resolvers: std::sync::Arc<Resolvers>,
    working_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
    delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
}

/// Create a `TCPListener` ready to be listened to
///
/// # Errors
///
/// * failed to bind to the socket address
/// * failed to set the listener to non blocking
pub fn socket_bind_anyhow<A: std::net::ToSocketAddrs + std::fmt::Debug>(
    addr: A,
) -> anyhow::Result<std::net::TcpListener> {
    let socket = std::net::TcpListener::bind(&addr)
        .with_context(|| format!("Failed to bind socket on addr: '{:?}'", addr))?;

    socket
        .set_nonblocking(true)
        .with_context(|| format!("Failed to set non-blocking socket on addr: '{:?}'", addr))?;

    Ok(socket)
}

type ListenerStreamItem = std::io::Result<(tokio::net::TcpStream, std::net::SocketAddr)>;

fn listener_to_stream(
    listener: &tokio::net::TcpListener,
) -> impl tokio_stream::Stream<Item = ListenerStreamItem> + '_ {
    async_stream::try_stream! {
        loop {
            let client = listener.accept().await?;
            yield client;
        }
    }
}

impl Server {
    /// Create a server with the configuration provided, and the sockets already bound
    ///
    /// # Errors
    ///
    /// * `spool_dir` does not exist and failed to be created
    /// * cannot convert sockets to `[tokio::net::TcpListener]`
    /// * cannot initialize [rustls] config
    pub fn new(
        config: std::sync::Arc<Config>,
        rule_engine: std::sync::Arc<RuleEngine>,
        resolvers: std::sync::Arc<Resolvers>,
        working_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
        delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
    ) -> anyhow::Result<Self> {
        if !config.server.queues.dirpath.exists() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .create(&config.server.queues.dirpath)?;
        }

        Ok(Self {
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
            rule_engine,
            resolvers,
            config,
            working_sender,
            delivery_sender,
        })
    }

    #[tracing::instrument(skip(self, stream))]
    async fn handle_client(
        &self,
        client_counter: std::sync::Arc<std::sync::atomic::AtomicI64>,
        kind: ConnectionKind,
        mut stream: tokio::net::TcpStream,
        client_addr: std::net::SocketAddr,
        server_addr: std::net::SocketAddr,
    ) {
        log::info!("Connection accepted");

        if self.config.server.client_count_max != -1
            && client_counter.load(std::sync::atomic::Ordering::SeqCst)
                >= self.config.server.client_count_max
        {
            log::info!(
                "Connection count max reached ({}), rejecting connection",
                self.config.server.client_count_max
            );

            if let Err(e) = tokio::io::AsyncWriteExt::write_all(
                &mut stream,
                self.config
                    .server
                    .smtp
                    .codes
                    .get(&CodeID::ConnectionMaxReached)
                    .expect("ill-formed configuration")
                    .fold()
                    .as_bytes(),
            )
            .await
            {
                log::error!("{e}");
            }

            if let Err(e) = tokio::io::AsyncWriteExt::shutdown(&mut stream).await {
                log::warn!("{e}");
            }
            return;
        }

        client_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let session = Self::run_session(
            Connection::new(
                kind,
                client_addr,
                stream.local_addr().expect("retrieve local address"),
                self.config.clone(),
                stream,
            ),
            self.tls_config.clone(),
            self.rsasl.clone(),
            self.rule_engine.clone(),
            self.resolvers.clone(),
            self.working_sender.clone(),
            self.delivery_sender.clone(),
        );
        let client_counter_copy = client_counter.clone();
        tokio::spawn(async move {
            if let Err(e) = session.await {
                log::warn!("{e}");
            }

            client_counter_copy.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        });
    }

    /// Main loop of `vSMTP`'s server
    ///
    /// # Errors
    ///
    /// * failed to convert sockets to `[tokio::net::TcpListener]`
    #[tracing::instrument(skip(self, sockets))]
    pub async fn listen_and_serve(
        self,
        sockets: (
            Vec<std::net::TcpListener>,
            Vec<std::net::TcpListener>,
            Vec<std::net::TcpListener>,
        ),
    ) -> anyhow::Result<()> {
        fn to_tokio(
            s: Vec<std::net::TcpListener>,
        ) -> std::io::Result<Vec<tokio::net::TcpListener>> {
            s.into_iter()
                .map(tokio::net::TcpListener::from_std)
                .collect::<std::io::Result<Vec<tokio::net::TcpListener>>>()
        }

        if self.config.server.tls.is_none() && !sockets.2.is_empty() {
            log::warn!(
                "No TLS configuration provided, listening on submissions protocol (port 465) will cause issue"
            );
        }

        let client_counter = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));

        let (listener, listener_submission, listener_tunneled) = (
            to_tokio(sockets.0)?,
            to_tokio(sockets.1)?,
            to_tokio(sockets.2)?,
        );

        let mut map = tokio_stream::StreamMap::new();
        for (kind, sockets) in [
            (ConnectionKind::Relay, &listener),
            (ConnectionKind::Submission, &listener_submission),
            (ConnectionKind::Tunneled, &listener_tunneled),
        ] {
            for listener in sockets {
                let accept = listener_to_stream(listener);
                let transform = tokio_stream::StreamExt::map(accept, move |client| (kind, client));

                map.insert(
                    listener.local_addr().expect("retrieve local address"),
                    Box::pin(transform),
                );
            }
        }

        log::info!(
            "Listening for clients on: {:?}",
            map.keys().collect::<Vec<_>>()
        );

        while let Some((server_addr, (kind, client))) =
            tokio_stream::StreamExt::next(&mut map).await
        {
            let (stream, client_addr) = client?;

            self.handle_client(
                client_counter.clone(),
                kind,
                stream,
                client_addr,
                server_addr,
            )
            .await;
        }
        Ok(())
    }

    ///
    /// # Errors
    #[tracing::instrument(skip(
        conn,
        tls_config,
        rsasl,
        rule_engine,
        resolvers,
        working_sender,
        delivery_sender
    ))]
    pub async fn run_session(
        mut conn: Connection<tokio::net::TcpStream>,
        tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
        rsasl: Option<std::sync::Arc<tokio::sync::Mutex<auth::Backend>>>,
        rule_engine: std::sync::Arc<RuleEngine>,
        resolvers: std::sync::Arc<Resolvers>,
        working_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
        delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
    ) -> anyhow::Result<()> {
        let connection_result = conn
            .receive(
                tls_config,
                rsasl,
                rule_engine,
                resolvers,
                &mut MailHandler {
                    working_sender,
                    delivery_sender,
                },
            )
            .await;

        match &connection_result {
            Ok(_) => {
                log::info!("Connection closed cleanly");
            }
            Err(error) => {
                log::warn!("Connection closed with an error: `{error}`");
            }
        }
        connection_result
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{socket_bind_anyhow, ProcessMessage, Server};
    use vsmtp_rule_engine::RuleEngine;
    use vsmtp_test::config;

    macro_rules! listen_with {
        ($addr:expr, $addr_submission:expr, $addr_submissions:expr, $timeout:expr, $client_count_max:expr) => {{
            let config = std::sync::Arc::new({
                let mut config = config::local_test();
                config.server.interfaces.addr = $addr;
                config.server.interfaces.addr_submission = $addr_submission;
                config.server.interfaces.addr_submissions = $addr_submissions;
                config.server.client_count_max = $client_count_max;
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
                std::sync::Arc::new(RuleEngine::new(&config, &None).unwrap()),
                std::sync::Arc::new(std::collections::HashMap::new()),
                working.0,
                delivery.0,
            )
            .unwrap();

            tokio::time::timeout(
                std::time::Duration::from_millis($timeout),
                s.listen_and_serve((
                    config
                        .server
                        .interfaces
                        .addr
                        .iter()
                        .cloned()
                        .map(socket_bind_anyhow)
                        .collect::<anyhow::Result<Vec<std::net::TcpListener>>>()
                        .unwrap(),
                    config
                        .server
                        .interfaces
                        .addr_submission
                        .iter()
                        .cloned()
                        .map(socket_bind_anyhow)
                        .collect::<anyhow::Result<Vec<std::net::TcpListener>>>()
                        .unwrap(),
                    config
                        .server
                        .interfaces
                        .addr_submissions
                        .iter()
                        .cloned()
                        .map(socket_bind_anyhow)
                        .collect::<anyhow::Result<Vec<std::net::TcpListener>>>()
                        .unwrap(),
                )),
            )
            .await
            .unwrap_err();
        }};
    }

    #[tokio::test]
    async fn basic() {
        listen_with![
            vec!["0.0.0.0:10021".parse().unwrap()],
            vec!["0.0.0.0:10588".parse().unwrap()],
            vec!["0.0.0.0:10466".parse().unwrap()],
            10,
            1
        ];
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn one_client_max_ok() {
        let server = tokio::spawn(async move {
            listen_with![
                vec!["127.0.0.1:10016".parse().unwrap()],
                vec!["127.0.0.1:10578".parse().unwrap()],
                vec!["127.0.0.1:10456".parse().unwrap()],
                500,
                1
            ];
        });

        let client = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let mail = lettre::Message::builder()
                .from("NoBody <nobody@domain.tld>".parse().unwrap())
                .reply_to("Yuin <yuin@domain.tld>".parse().unwrap())
                .to("Hei <hei@domain.tld>".parse().unwrap())
                .subject("Happy new year")
                .body(String::from("Be happy!"))
                .unwrap();

            let sender = lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::builder_dangerous(
                "127.0.0.1",
            )
            .port(10016)
            .build();

            lettre::AsyncTransport::send(&sender, mail).await
        });

        let (client, server) = tokio::join!(client, server);
        server.unwrap();
        // client transaction is ok, but has been denied (because there is no rules)
        assert_eq!(
            format!("{}", client.unwrap().unwrap_err()),
            "permanent error (554): permanent problems with the remote server"
        );
    }

    // FIXME: randomly fail the CI
    /*
    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn one_client_max_err() {
        let server = tokio::spawn(async move {
            listen_with![
                vec!["127.0.0.1:10006".parse().unwrap()],
                vec!["127.0.0.1:10568".parse().unwrap()],
                vec!["127.0.0.1:10446".parse().unwrap()],
                1000,
                1
            ];
        });

        let now = tokio::time::Instant::now();
        let until = now
            .checked_add(std::time::Duration::from_millis(100))
            .unwrap();

        let client = tokio::spawn(async move {
            tokio::time::sleep_until(until).await;
            let mail = lettre::Message::builder()
                .from("NoBody <nobody@domain.tld>".parse().unwrap())
                .reply_to("Yuin <yuin@domain.tld>".parse().unwrap())
                .to("Hei <hei@domain.tld>".parse().unwrap())
                .subject("Happy new year")
                .body(String::from("Be happy!"))
                .unwrap();

            let sender = lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::builder_dangerous(
                "127.0.0.1",
            )
            .port(10006)
            .build();

            lettre::AsyncTransport::send(&sender, mail).await
        });

        let client2 = tokio::spawn(async move {
            tokio::time::sleep_until(until).await;
            let mail = lettre::Message::builder()
                .from("NoBody <nobody2@domain.tld>".parse().unwrap())
                .reply_to("Yuin <yuin@domain.tld>".parse().unwrap())
                .to("Hei <hei@domain.tld>".parse().unwrap())
                .subject("Happy new year")
                .body(String::from("Be happy!"))
                .unwrap();

            let sender = lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::builder_dangerous(
                "127.0.0.1",
            )
            .port(10006)
            .build();

            lettre::AsyncTransport::send(&sender, mail).await
        });

        let (server, client, client2) = tokio::join!(server, client, client2);
        server.unwrap();

        let client1 = format!("{}", client.unwrap().unwrap_err());
        let client2 = format!("{}", client2.unwrap().unwrap_err());

        // one of the client has been denied on connection, but we cant know which one
        let ok1_failed2 = client1
            == "permanent error (554): permanent problems with the remote server"
            && client2 == "permanent error (554): Cannot process connection, closing";
        let ok2_failed1 = client2
            == "permanent error (554): permanent problems with the remote server"
            && client1 == "permanent error (554): Cannot process connection, closing";

        assert!(ok1_failed2 || ok2_failed1);
    }
    */
}
