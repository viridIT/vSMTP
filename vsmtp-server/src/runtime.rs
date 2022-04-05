use crate::{
    processes::{delivery, mime},
    ProcessMessage, Server,
};
use vsmtp_common::re::{anyhow, log};
use vsmtp_config::Config;
use vsmtp_rule_engine::rule_engine::RuleEngine;

/// Start the vSMTP server's runtime
///
/// # Errors
///
/// # Panics
///
/// * todo!(): a sub routine ended unexpectedly
#[allow(clippy::module_name_repetitions)]
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
        let res = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(config_copy.server.system.thread_pool.delivery)
            .enable_all()
            .thread_name("vsmtp-delivery")
            .build()?
            .block_on(async move {
                if cfg!(test) {
                    return Ok(());
                }

                let res = delivery::start(config_copy, rule_engine_copy, delivery_receiver).await;
                log::error!("vsmtp-delivery thread ended unexpectedly '{:?}'", res);
                anyhow::Ok(())
            });
        if res.is_err() {
            todo!();
        }
        std::io::Result::Ok(())
    });

    let config_copy = config.clone();
    let rule_engine_copy = rule_engine.clone();
    let mime_delivery_sender = delivery_sender.clone();
    let tasks_processing = std::thread::spawn(|| {
        let res = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(config_copy.server.system.thread_pool.processing)
            .enable_all()
            .thread_name("vsmtp-processing")
            .build()?
            .block_on(async move {
                if cfg!(test) {
                    return Ok(());
                }

                let res = mime::start(
                    config_copy,
                    rule_engine_copy,
                    working_receiver,
                    mime_delivery_sender,
                )
                .await;
                log::error!("vsmtp-processing thread ended unexpectedly '{:?}'", res);
                anyhow::Ok(())
            });
        if res.is_err() {
            todo!();
        }
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
                if cfg!(test) {
                    return Ok(());
                }

                server.listen_and_serve().await
            });
        if res.is_err() {
            todo!();
        }
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
mod tests {
    use vsmtp_test::config;

    use super::*;

    #[test]
    fn basic() -> anyhow::Result<()> {
        start_runtime(
            std::sync::Arc::new(config::local_test()),
            (
                std::net::TcpListener::bind("0.0.0.0:22001").unwrap(),
                std::net::TcpListener::bind("0.0.0.0:22002").unwrap(),
                std::net::TcpListener::bind("0.0.0.0:22003").unwrap(),
            ),
        )
    }
}
