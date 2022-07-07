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
use crate::{delivery, log_channels, processing, ProcessMessage, Server};
use vsmtp_common::{
    queue::Queue,
    re::{
        anyhow::{self, Context},
        log, strum, tokio,
    },
};
use vsmtp_config::Config;
use vsmtp_rule_engine::rule_engine::RuleEngine;

fn init_runtime<F>(
    sender: tokio::sync::mpsc::Sender<()>,
    name: impl Into<String>,
    worker_thread_count: usize,
    future: F,
    timeout: Option<std::time::Duration>,
) -> anyhow::Result<std::thread::JoinHandle<anyhow::Result<()>>>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let name = name.into();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_thread_count)
        .enable_all()
        .thread_name(format!("{name}-child"))
        .build()?;

    std::thread::Builder::new()
        .name(format!("{name}-main"))
        .spawn(move || {
            let name_rt = name.clone();
            runtime.block_on(async move {
                log::info!(
                    target: log_channels::RUNTIME,
                    "Runtime '{name_rt}' started successfully"
                );

                match timeout {
                    Some(duration) => {
                        tokio::time::timeout(duration, future).await.unwrap_err();
                    }
                    None => future.await,
                }
            });

            sender.blocking_send(())?;
            Ok(())
        })
        .map_err(anyhow::Error::new)
}

/// Start the `vSMTP` server's runtime
///
/// # Errors
///
#[allow(clippy::module_name_repetitions)]
pub fn start_runtime(
    config: Config,
    sockets: (
        Vec<std::net::TcpListener>,
        Vec<std::net::TcpListener>,
        Vec<std::net::TcpListener>,
    ),
    timeout: Option<std::time::Duration>,
) -> anyhow::Result<()> {
    <Queue as strum::IntoEnumIterator>::iter()
        .map(|q| vsmtp_common::queue_path!(create_if_missing => &config.server.queues.dirpath, q))
        .collect::<std::io::Result<Vec<_>>>()?;

    let mut error_handler = tokio::sync::mpsc::channel::<()>(3);

    let (delivery_channel, working_channel) = (
        tokio::sync::mpsc::channel::<ProcessMessage>(config.server.queues.delivery.channel_size),
        tokio::sync::mpsc::channel::<ProcessMessage>(config.server.queues.working.channel_size),
    );

    let rule_engine = RuleEngine::new(&config, &config.app.vsl.filepath.clone())?;

    let resolvers = std::sync::Arc::new(
        vsmtp_config::build_resolvers(&config).context("could not initialize dns")?,
    );

    let config_arc = std::sync::Arc::new(config);
    let rule_engine_arc = std::sync::Arc::new(std::sync::RwLock::new(rule_engine));

    let _tasks_delivery = init_runtime(
        error_handler.0.clone(),
        "delivery",
        config_arc.server.system.thread_pool.delivery,
        delivery::start(
            config_arc.clone(),
            rule_engine_arc.clone(),
            resolvers.clone(),
            delivery_channel.1,
        ),
        timeout,
    )?;

    let _tasks_processing = init_runtime(
        error_handler.0.clone(),
        "processing",
        config_arc.server.system.thread_pool.processing,
        processing::start(
            config_arc.clone(),
            rule_engine_arc.clone(),
            resolvers.clone(),
            working_channel.1,
            delivery_channel.0.clone(),
        ),
        timeout,
    )?;

    let _tasks_receiver = init_runtime(
        error_handler.0.clone(),
        "receiver",
        config_arc.server.system.thread_pool.receiver,
        async move {
            let server = match Server::new(
                config_arc.clone(),
                rule_engine_arc.clone(),
                resolvers.clone(),
                working_channel.0.clone(),
                delivery_channel.0.clone(),
            ) {
                Ok(server) => server,
                Err(error) => {
                    log::error!(target: log_channels::SERVER, "{}", error);
                    return;
                }
            };
            if let Err(error) = server.listen_and_serve(sockets).await {
                log::error!(target: log_channels::SERVER, "{}", error);
            }
        },
        timeout,
    );

    let error_handler_sig = error_handler.0.clone();
    let mut signals = signal_hook::iterator::Signals::new(&[
        // Send by `systemctl stop` (and then sending `SIGKILL`)
        signal_hook::consts::SIGTERM,
        // Ctrl+C on a terminal
        signal_hook::consts::SIGINT,
    ])?;
    let _signal_handler = std::thread::spawn(move || {
        for sig in signals.forever() {
            log::info!(target: log_channels::RUNTIME, "Received signal '{}'", sig);
            log::warn!(target: log_channels::RUNTIME, "Stopping vSMTP server");
            error_handler_sig
                .blocking_send(())
                .expect("failed to send terminating instruction");
        }
    });

    error_handler.1.blocking_recv();

    Ok(())

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
                vec![std::net::TcpListener::bind("0.0.0.0:22001").unwrap()],
                vec![std::net::TcpListener::bind("0.0.0.0:22002").unwrap()],
                vec![std::net::TcpListener::bind("0.0.0.0:22003").unwrap()],
            ),
            Some(std::time::Duration::from_millis(100)),
        )
    }
}
