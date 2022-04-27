use crate::{
    log_channels,
    processes::{delivery, postq},
    ProcessMessage, Server,
};
use vsmtp_common::{
    queue::Queue,
    re::{
        anyhow::{self, Context},
        log, strum,
    },
};
use vsmtp_config::Config;
use vsmtp_rule_engine::rule_engine::RuleEngine;

fn init_runtime<F: 'static>(
    sender: tokio::sync::mpsc::Sender<anyhow::Result<()>>,
    name: impl Into<String>,
    worker_thread_count: usize,
    future: F,
) -> anyhow::Result<std::thread::JoinHandle<anyhow::Result<()>>>
where
    F: std::future::Future<Output = anyhow::Result<()>> + Send,
{
    let name = name.into();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_thread_count)
        .enable_all()
        .thread_name(name.clone())
        .build()?;

    std::thread::Builder::new()
        .name(format!("{name}-main"))
        .spawn(move || {
            let output = if cfg!(test) {
                Ok(())
            } else {
                runtime
                    .block_on({
                        log::info!(
                            target: log_channels::RUNTIME,
                            "Runtime '{name}' started successfully"
                        );
                        future
                    })
                    .context(format!("An error terminated the '{name}' runtime"))
            };

            sender.blocking_send(output)?;
            Ok(())
        })
        .map_err(anyhow::Error::new)
}

/// Start the vSMTP server's runtime
///
/// # Errors
///
#[allow(clippy::module_name_repetitions)]
pub fn start_runtime(
    config: std::sync::Arc<Config>,
    sockets: (
        std::net::TcpListener,
        std::net::TcpListener,
        std::net::TcpListener,
    ),
) -> anyhow::Result<()> {
    <Queue as strum::IntoEnumIterator>::iter()
        .map(|q| vsmtp_common::queue_path!(create_if_missing => &config.server.queues.dirpath, q))
        .collect::<std::io::Result<Vec<_>>>()?;

    let (main_runtime_sender, mut main_runtime_receiver) =
        tokio::sync::mpsc::channel::<anyhow::Result<()>>(
            config.server.queues.delivery.channel_size,
        );

    let (delivery_sender, delivery_receiver) =
        tokio::sync::mpsc::channel::<ProcessMessage>(config.server.queues.delivery.channel_size);

    let (working_sender, working_receiver) =
        tokio::sync::mpsc::channel::<ProcessMessage>(config.server.queues.delivery.channel_size);

    let rule_engine = std::sync::Arc::new(std::sync::RwLock::new(RuleEngine::new(
        &config,
        &Some(config.app.vsl.filepath.clone()),
    )?));

    let _tasks_delivery = init_runtime(
        main_runtime_sender.clone(),
        "vsmtp-delivery",
        config.server.system.thread_pool.delivery,
        delivery::start(config.clone(), rule_engine.clone(), delivery_receiver),
    )?;

    let _tasks_processing = init_runtime(
        main_runtime_sender.clone(),
        "vsmtp-processing",
        config.server.system.thread_pool.processing,
        postq::start(
            config.clone(),
            rule_engine.clone(),
            working_receiver,
            delivery_sender.clone(),
        ),
    )?;

    let _tasks_receiver = init_runtime(
        main_runtime_sender,
        "vsmtp-receiver",
        config.server.system.thread_pool.receiver,
        async move {
            let mut server = Server::new(
                config,
                sockets,
                rule_engine,
                working_sender,
                delivery_sender,
            )?;
            log::info!(
                target: log_channels::RUNTIME,
                "Listening on: {:?}",
                server.addr()
            );
            server.listen_and_serve().await
        },
    )?;

    main_runtime_receiver
        .blocking_recv()
        .ok_or_else(|| anyhow::anyhow!("Channel closed, but should not"))?

    // if the runtime panicked (receiver/processing/delivery)
    // .join() would return an error,
    // but the join is CPU heavy and he blocking (so we can't join all of them)
    // for i in [tasks_receiver, tasks_delivery, tasks_processing] {
    //     i.join().map_err(|e| anyhow::anyhow!("{e:?}"))??;
    // }
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
