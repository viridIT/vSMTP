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

    use vsmtp_common::mail_context::MailContext;
    use vsmtp_config::ServerConfig;

    /// A trait allowing the [ServerVSMTP] to deliver a mail
    #[async_trait::async_trait]
    pub trait Resolver {
        /// the deliver method of the [Resolver] trait
        async fn deliver(
            &mut self,
            config: &ServerConfig,
            mail: &MailContext,
        ) -> anyhow::Result<()>;
    }

    pub(super) mod maildir_resolver;
    pub(super) mod mbox_resolver;
    pub(super) mod smtp_resolver;

    #[cfg(test)]
    #[must_use]
    pub fn get_default_context() -> MailContext {
        use vsmtp_common::{
            envelop::Envelop,
            mail_context::{Body, MessageMetadata},
        };

        MailContext {
            body: Body::Empty,
            connection_timestamp: std::time::SystemTime::now(),
            client_addr: std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
                0,
            ),
            envelop: Envelop::default(),
            metadata: Some(MessageMetadata {
                timestamp: std::time::SystemTime::now(),
                ..MessageMetadata::default()
            }),
        }
    }
}

use processes::ProcessMessage;
use vsmtp_config::ServerConfig;
use vsmtp_rule_engine::rule_engine::RuleEngine;

use crate::{
    resolver::{
        maildir_resolver::MailDirResolver, mbox_resolver::MBoxResolver, smtp_resolver::SMTPResolver,
    },
    server::ServerVSMTP,
};

#[doc(hidden)]
pub fn start_runtime(
    config: std::sync::Arc<ServerConfig>,
    sockets: (
        std::net::TcpListener,
        std::net::TcpListener,
        std::net::TcpListener,
    ),
) -> anyhow::Result<()> {
    let resolvers = {
        let mut resolvers =
            std::collections::HashMap::<String, Box<dyn resolver::Resolver + Send + Sync>>::new();
        resolvers.insert("maildir".to_string(), Box::new(MailDirResolver::default()));
        resolvers.insert("smtp".to_string(), Box::new(SMTPResolver::default()));
        resolvers.insert("mbox".to_string(), Box::new(MBoxResolver::default()));
        resolvers
    };

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
