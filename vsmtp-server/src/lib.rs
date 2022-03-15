//! vSMTP server

#![doc(html_no_source)]
#![deny(missing_docs)]
//
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
//
#![allow(clippy::doc_markdown)]

///
pub mod processes;
///
pub mod queue;
///
pub mod receiver;
///
pub mod server;
mod tls_helpers;

///
pub mod resolver {

    use vsmtp_common::{address::Address, mail_context::MessageMetadata, rcpt::Rcpt};
    use vsmtp_config::ServerConfig;

    /// A trait allowing the [ServerVSMTP] to deliver a mail
    #[async_trait::async_trait]
    pub trait Resolver {
        /// the deliver method of the [Resolver] trait
        async fn deliver(
            &mut self,
            config: &ServerConfig,
            metadata: &MessageMetadata,
            from: &Address,
            to: &mut [Rcpt],
            content: &str,
        ) -> anyhow::Result<()>;
    }

    pub(super) mod maildir;
    pub(super) mod mbox;
    pub(super) mod relay;

    /// no transfer will be made if this resolver is selected.
    pub(super) struct NoTransfer;

    #[async_trait::async_trait]
    impl Resolver for NoTransfer {
        async fn deliver(
            &mut self,
            _: &ServerConfig,
            _: &MessageMetadata,
            _: &Address,
            _: &mut [Rcpt],
            _: &str,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[cfg(test)]
    #[must_use]
    pub fn get_default_context() -> vsmtp_common::mail_context::MailContext {
        vsmtp_common::mail_context::MailContext {
            body: vsmtp_common::mail_context::Body::Empty,
            connection_timestamp: std::time::SystemTime::now(),
            client_addr: std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
                0,
            ),
            envelop: vsmtp_common::envelop::Envelop::default(),
            metadata: Some(vsmtp_common::mail_context::MessageMetadata {
                timestamp: std::time::SystemTime::now(),
                ..vsmtp_common::mail_context::MessageMetadata::default()
            }),
        }
    }
}

use processes::ProcessMessage;
use vsmtp_config::ServerConfig;
use vsmtp_rule_engine::rule_engine::RuleEngine;

use crate::server::ServerVSMTP;

/// create a list of resolvers identified by their Transfer key.
#[must_use]
pub fn create_resolvers() -> std::collections::HashMap<
    vsmtp_common::transfer::Transfer,
    Box<dyn resolver::Resolver + Send + Sync>,
> {
    let mut resolvers = std::collections::HashMap::<
        vsmtp_common::transfer::Transfer,
        Box<dyn resolver::Resolver + Send + Sync>,
    >::new();
    resolvers.insert(
        vsmtp_common::transfer::Transfer::Maildir,
        Box::new(resolver::maildir::MailDir::default()),
    );
    resolvers.insert(
        vsmtp_common::transfer::Transfer::Mbox,
        Box::new(resolver::mbox::MBox::default()),
    );
    resolvers.insert(
        vsmtp_common::transfer::Transfer::Relay,
        Box::new(resolver::relay::Relay::default()),
    );
    resolvers.insert(
        vsmtp_common::transfer::Transfer::None,
        Box::new(resolver::NoTransfer {}),
    );
    resolvers
}

#[doc(hidden)]
pub fn start_runtime(
    config: std::sync::Arc<ServerConfig>,
    sockets: (
        std::net::TcpListener,
        std::net::TcpListener,
        std::net::TcpListener,
    ),
) -> anyhow::Result<()> {
    let resolvers = create_resolvers();

    let (delivery_sender, delivery_receiver) =
        tokio::sync::mpsc::channel::<ProcessMessage>(config.delivery.queues.deliver.capacity);

    let (working_sender, working_receiver) =
        tokio::sync::mpsc::channel::<ProcessMessage>(config.delivery.queues.working.capacity);

    let rule_engine = std::sync::Arc::new(std::sync::RwLock::new(RuleEngine::new(
        &config.rules.main_filepath.clone(),
    )?));

    let config_copy = config.clone();
    let rule_engine_copy = rule_engine.clone();
    let tasks_delivery = std::thread::spawn(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(config_copy.server.thread_count)
            .enable_all()
            .thread_name("vsmtp-delivery")
            .build()?
            .block_on(async move {
                let result = crate::processes::delivery::start(
                    config_copy,
                    rule_engine_copy,
                    resolvers,
                    delivery_receiver,
                )
                .await;
                log::error!("v_deliver ended unexpectedly '{:?}'", result);
            });
        std::io::Result::Ok(())
    });

    let config_copy = config.clone();
    let rule_engine_copy = rule_engine.clone();
    let mime_delivery_sender = delivery_sender.clone();
    let tasks_processing = std::thread::spawn(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(config_copy.server.thread_count)
            .enable_all()
            .thread_name("vsmtp-processing")
            .build()?
            .block_on(async move {
                let result = crate::processes::mime::start(
                    config_copy,
                    rule_engine_copy,
                    working_receiver,
                    mime_delivery_sender,
                )
                .await;
                log::error!("v_mime ended unexpectedly '{:?}'", result);
            });
        std::io::Result::Ok(())
    });

    let tasks_receiver = std::thread::spawn(|| {
        let res = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(config.server.thread_count)
            .enable_all()
            .thread_name("vsmtp-receiver")
            .build()?
            .block_on(async move {
                let mut server = ServerVSMTP::new(
                    config,
                    sockets,
                    rule_engine,
                    working_sender,
                    delivery_sender,
                )?;
                log::info!("Listening on: {:?}", server.addr());

                server.listen_and_serve().await
            });
        if res.is_err() {}
        std::io::Result::Ok(())
    });

    [
        tasks_delivery
            .join()
            .map_err(|e| anyhow::anyhow!("{:?}", e))?,
        tasks_processing
            .join()
            .map_err(|e| anyhow::anyhow!("{:?}", e))?,
        tasks_receiver
            .join()
            .map_err(|e| anyhow::anyhow!("{:?}", e))?,
    ]
    .into_iter()
    .collect::<std::io::Result<Vec<()>>>()?;

    Ok(())
}
