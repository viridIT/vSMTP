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

#[cfg(test)]
mod tests;

mod channel_message;
mod processes;
mod queue;
mod receiver;
mod server;

/// SMTP auth extension implementation
pub mod auth;
pub use channel_message::ProcessMessage;
pub use receiver::{handle_connection, Connection, ConnectionKind, IoService, OnMail};
pub use server::Server;

/// re-exported module
pub mod re {
    pub use base64;
    pub use tokio;
}

use vsmtp_common::re::{anyhow, log};
use vsmtp_config::Config;
use vsmtp_rule_engine::rule_engine::RuleEngine;

#[doc(hidden)]
pub fn start_runtime(
    config: std::sync::Arc<Config>,
    sockets: (
        std::net::TcpListener,
        std::net::TcpListener,
        std::net::TcpListener,
    ),
) -> anyhow::Result<()> {
    let (delivery_sender, delivery_receiver) =
        tokio::sync::mpsc::channel::<ProcessMessage>(config.server.queues.delivery.channel_size);

    let (working_sender, working_receiver) =
        tokio::sync::mpsc::channel::<ProcessMessage>(config.server.queues.delivery.channel_size);

    let rule_engine = std::sync::Arc::new(std::sync::RwLock::new(RuleEngine::new(&Some(
        config.app.vsl.filepath.clone(),
    ))?));

    let config_copy = config.clone();
    let rule_engine_copy = rule_engine.clone();
    let tasks_delivery = std::thread::spawn(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(config_copy.server.system.thread_pool.delivery)
            .enable_all()
            .thread_name("vsmtp-delivery")
            .build()?
            .block_on(async move {
                let result = crate::processes::delivery::start(
                    config_copy,
                    rule_engine_copy,
                    delivery_receiver,
                )
                .await;
                log::error!("vsmtp-delivery thread ended unexpectedly '{:?}'", result);
            });
        std::io::Result::Ok(())
    });

    let config_copy = config.clone();
    let rule_engine_copy = rule_engine.clone();
    let mime_delivery_sender = delivery_sender.clone();
    let tasks_processing = std::thread::spawn(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(config_copy.server.system.thread_pool.processing)
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
                log::error!("vsmtp-processing thread ended unexpectedly '{:?}'", result);
            });
        std::io::Result::Ok(())
    });

    let tasks_receiver = std::thread::spawn(|| {
        let res = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(config.server.system.thread_pool.receiver)
            .enable_all()
            .thread_name("vsmtp-receiver")
            .build()?
            .block_on(async move {
                let mut server = Server::new(
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
