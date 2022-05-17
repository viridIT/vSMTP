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
    timeout: Option<std::time::Duration>,
) -> anyhow::Result<std::thread::JoinHandle<anyhow::Result<()>>>
where
    F: std::future::Future<Output = anyhow::Result<()>> + Send,
{
    let name = name.into();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_thread_count)
        .enable_all()
        .thread_name(&name)
        .build()?;

    std::thread::Builder::new()
        .name(format!("{name}-main"))
        .spawn(move || {
            let name_rt = name.clone();
            let output = runtime
                .block_on(async move {
                    log::info!(
                        target: log_channels::RUNTIME,
                        "Runtime '{name_rt}' started successfully"
                    );

                    match timeout {
                        Some(duration) => {
                            tokio::time::timeout(duration, future).await.unwrap_err();
                            anyhow::Ok(())
                        }
                        None => future.await,
                    }
                })
                .context(format!("An error terminated the '{name}' runtime"));

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
    config: Config,
    sockets: (
        std::net::TcpListener,
        std::net::TcpListener,
        std::net::TcpListener,
    ),
    timeout: Option<std::time::Duration>,
) -> anyhow::Result<()> {
    <Queue as strum::IntoEnumIterator>::iter()
        .map(|q| vsmtp_common::queue_path!(create_if_missing => &config.server.queues.dirpath, q))
        .collect::<std::io::Result<Vec<_>>>()?;

    let mut error_handler = tokio::sync::mpsc::channel::<anyhow::Result<()>>(3);

    let delivery_channel =
        tokio::sync::mpsc::channel::<ProcessMessage>(config.server.queues.delivery.channel_size);

    let working_channel =
        tokio::sync::mpsc::channel::<ProcessMessage>(config.server.queues.working.channel_size);

    let rule_engine = std::sync::Arc::new(std::sync::RwLock::new(RuleEngine::new(
        &config,
        &config.app.vsl.filepath.clone(),
    )?));

    let resolvers = std::sync::Arc::new(
        vsmtp_config::build_resolvers(&config).context("could not initialize dns")?,
    );

    let config_arc = std::sync::Arc::new(config);

    let _tasks_delivery = init_runtime(
        error_handler.0.clone(),
        "vsmtp-delivery",
        config_arc.server.system.thread_pool.delivery,
        delivery::start(
            config_arc.clone(),
            rule_engine.clone(),
            resolvers.clone(),
            delivery_channel.1,
        ),
        timeout,
    )?;

    let _tasks_processing = init_runtime(
        error_handler.0.clone(),
        "vsmtp-processing",
        config_arc.server.system.thread_pool.processing,
        postq::start(
            config_arc.clone(),
            rule_engine.clone(),
            resolvers.clone(),
            working_channel.1,
            delivery_channel.0.clone(),
        ),
        timeout,
    )?;

    let _tasks_receiver = init_runtime(
        error_handler.0,
        "vsmtp-receiver",
        config_arc.server.system.thread_pool.receiver,
        async move {
            Server::new(
                config_arc.clone(),
                sockets,
                rule_engine.clone(),
                resolvers.clone(),
                working_channel.0.clone(),
                delivery_channel.0.clone(),
            )?
            .listen_and_serve()
            .await
        },
        timeout,
    );

    error_handler
        .1
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
            config::local_test(),
            (
                std::net::TcpListener::bind("0.0.0.0:22001").unwrap(),
                std::net::TcpListener::bind("0.0.0.0:22002").unwrap(),
                std::net::TcpListener::bind("0.0.0.0:22003").unwrap(),
            ),
            Some(std::time::Duration::from_millis(100)),
        )
    }
}
