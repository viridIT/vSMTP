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

use processes::ProcessMessage;
use vsmtp_config::ServerConfig;
use vsmtp_rule_engine::rule_engine::RuleEngine;

use crate::server::ServerVSMTP;

///
pub mod processes;
///
pub mod queue;
///
pub mod receiver;
///
pub mod server;
mod tls_helpers;

#[doc(hidden)]
pub fn start_runtime(
    config: std::sync::Arc<ServerConfig>,
    sockets: (
        std::net::TcpListener,
        std::net::TcpListener,
        std::net::TcpListener,
    ),
) -> anyhow::Result<()> {
    let resolvers = vsmtp_delivery::transport::create_transports();

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
                log::error!("vsmtp-delivery thread ended unexpectedly '{:?}'", result);
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
                log::error!("vsmtp-processing thread ended unexpectedly '{:?}'", result);
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

#[cfg(test)]
mod test {

    #[test]
    fn test_build_lettre_envelop() {
        let mut ctx = vsmtp_delivery::test::get_default_context();

        // assert!(build_envelop(&ctx).is_err());

        ctx.envelop.rcpt.push(
            vsmtp_common::address::Address::try_from("john@doe.com")
                .unwrap()
                .into(),
        );

        // build_envelop(&ctx).expect("failed to build the envelop");
    }
}
