use super::ProcessMessage;
/**
 * vSMTP mail transfer agent
 * Copyright (C) 2021 viridIT SAS
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
use crate::{
    config::{log_channel::DELIVER, server_config::ServerConfig},
    queue::Queue,
    resolver::Resolver,
    rules::rule_engine::{RuleEngine, RuleState, Status},
};
use std::collections::HashMap;

/// process used to deliver incoming emails force accepted by the smtp process
/// or parsed by the vMime process.
pub async fn start(
    config: std::sync::Arc<ServerConfig>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    mut resolvers: HashMap<String, Box<dyn Resolver + Send + Sync>>,
    mut delivery_receiver: tokio::sync::mpsc::Receiver<ProcessMessage>,
) -> anyhow::Result<()> {
    log::info!(
        target: DELIVER,
        "vDeliver (deferred) booting, flushing queue.",
    );
    flush_deferred_queue(&mut resolvers, &config).await?;

    log::info!(
        target: DELIVER,
        "vDeliver (delivery) booting, flushing queue.",
    );
    flush_deliver_queue(&config, &rule_engine, &mut resolvers).await?;

    let mut flush_deferred_interval = tokio::time::interval(
        config
            .delivery
            .queues
            .get("deferred")
            .map(|q| q.cron_period)
            .flatten()
            .unwrap_or_else(|| std::time::Duration::from_secs(10)),
    );

    loop {
        tokio::select! {
            Some(pm) = delivery_receiver.recv() => {
                            if let Err(error) = handle_one_in_delivery_queue(
                                &config,
                                &std::path::PathBuf::from_iter([
                                    Queue::Deliver.to_path(&config.delivery.spool_dir)?,
                                    std::path::Path::new(&pm.message_id).to_path_buf(),
                                ]),
                                &rule_engine,
                                &mut resolvers,
                )
                .await {
                    log::error!(target: DELIVER, "{error}");
                }
            }
            _ = flush_deferred_interval.tick() => {
                log::info!(
                    target: DELIVER,
                    "vDeliver (deferred) cronjob delay elapsed, flushing queue.",
                );
                flush_deferred_queue(&mut resolvers, &config).await.unwrap();
            }
        };
    }
}

pub(crate) async fn handle_one_in_delivery_queue(
    config: &ServerConfig,
    path: &std::path::Path,
    rule_engine: &std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    resolvers: &mut HashMap<String, Box<dyn Resolver + Send + Sync>>,
) -> anyhow::Result<()> {
    let message_id = path.file_name().and_then(|i| i.to_str()).unwrap();

    log::trace!(
        target: DELIVER,
        "vDeliver (delivery) RECEIVED '{}'",
        message_id
    );

    let mut file = std::fs::OpenOptions::new().read(true).open(&path)?;

    let mut raw = String::with_capacity(file.metadata().unwrap().len() as usize);
    std::io::Read::read_to_string(&mut file, &mut raw)?;

    let ctx: crate::model::mail::MailContext = serde_json::from_str(&raw)?;

    let mut state = RuleState::new(config);
    state.add_data("ctx", ctx);
    let result = rule_engine.read().unwrap().run_when(&mut state, "delivery");

    match result {
        Status::Deny => Queue::Dead.write_to_queue(config, &state.get_context().read().unwrap())?,
        Status::Block => {
            Queue::Quarantine.write_to_queue(config, &state.get_context().read().unwrap())?
        }
        _ => {
            let ctx = state.get_context().read().unwrap().clone();
            match &ctx.metadata {
                // quietly skipping delivery processes when there is no resolver.
                // (in case of a quarantine for example)
                Some(metadata) if metadata.resolver == "none" => {
                    log::warn!(
                        target: DELIVER,
                        "delivery skipped due to NO_DELIVERY action call."
                    );
                    return Ok(());
                }
                _ => {}
            };

            let resolver_name = &ctx.metadata.as_ref().unwrap().resolver;
            let resolver = match resolvers.get_mut(resolver_name) {
                Some(resolver) => resolver,
                None => anyhow::bail!("resolver '{resolver_name}' not found"),
            };

            match resolver.deliver(config, &ctx).await {
                Ok(_) => {
                    log::trace!(
                        target: DELIVER,
                        "vDeliver (delivery) '{}' SEND successfully.",
                        message_id
                    );

                    std::fs::remove_file(&path)?;

                    log::info!(
                        target: DELIVER,
                        "vDeliver (delivery) '{}' REMOVED successfully.",
                        message_id
                    );
                }
                Err(error) => {
                    log::warn!(
                        target: DELIVER,
                        "vDeliver (delivery) '{}' SEND FAILED, reason: '{}'",
                        message_id,
                        error
                    );

                    std::fs::rename(
                        path,
                        std::path::PathBuf::from_iter([
                            Queue::Deferred.to_path(&config.delivery.spool_dir)?,
                            std::path::Path::new(&message_id).to_path_buf(),
                        ]),
                    )?;

                    log::info!(
                        target: DELIVER,
                        "vDeliver (delivery) '{}' MOVED delivery => deferred.",
                        message_id
                    );
                }
            }
        }
    };

    Ok(())
}

async fn flush_deliver_queue(
    config: &ServerConfig,
    rule_engine: &std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    resolvers: &mut HashMap<String, Box<dyn Resolver + Send + Sync>>,
) -> anyhow::Result<()> {
    for path in std::fs::read_dir(Queue::Deliver.to_path(&config.delivery.spool_dir)?)? {
        handle_one_in_delivery_queue(config, &path?.path(), rule_engine, resolvers).await?
    }

    Ok(())
}

async fn handle_one_in_deferred_queue(
    resolvers: &mut HashMap<String, Box<dyn Resolver + Send + Sync>>,
    path: &std::path::Path,
    config: &ServerConfig,
) -> anyhow::Result<()> {
    let message_id = path.file_name().and_then(|i| i.to_str()).unwrap();

    log::debug!(
        target: DELIVER,
        "vDeliver (deferred) RECEIVED '{}'",
        message_id
    );

    let mut file = std::fs::OpenOptions::new().read(true).open(&path)?;

    let mut raw = String::with_capacity(file.metadata().unwrap().len() as usize);
    std::io::Read::read_to_string(&mut file, &mut raw)?;

    let mut mail: crate::model::mail::MailContext = serde_json::from_str(&raw)?;

    let max_retry_deferred = config
        .delivery
        .queues
        .get("deferred")
        .map(|q| q.retry_max)
        .flatten()
        .unwrap_or(100);

    if mail.metadata.is_none() {
        anyhow::bail!("email metadata is missing")
    }

    if mail.metadata.as_ref().unwrap().retry >= max_retry_deferred {
        log::warn!(
            target: DELIVER,
            "vDeliver (deferred) '{}' MAX RETRY, retry: '{}'",
            message_id,
            mail.metadata.as_ref().unwrap().retry
        );

        std::fs::rename(
            path,
            std::path::PathBuf::from_iter([
                Queue::Dead.to_path(&config.delivery.spool_dir)?,
                std::path::Path::new(&message_id).to_path_buf(),
            ]),
        )?;

        log::info!(
            target: DELIVER,
            "vDeliver (deferred) '{}' MOVED deferred => dead.",
            message_id
        );
    } else {
        let resolver_name = &mail.metadata.as_ref().unwrap().resolver;
        let resolver = match resolvers.get_mut(resolver_name) {
            Some(resolver) => resolver,
            None => anyhow::bail!("resolver '{resolver_name}' not found"),
        };

        match resolver.deliver(config, &mail).await {
            Ok(_) => {
                log::trace!(
                    target: DELIVER,
                    "vDeliver (deferred) '{}' SEND successfully.",
                    message_id
                );

                std::fs::remove_file(&path)?;

                log::info!(
                    target: DELIVER,
                    "vDeliver (deferred) '{}' REMOVED successfully.",
                    message_id
                );
            }
            Err(error) => {
                log::warn!(
                    target: DELIVER,
                    "vDeliver (deferred) '{}' SEND FAILED, reason: '{}'",
                    message_id,
                    error
                );

                mail.metadata.as_mut().unwrap().retry += 1;

                let mut file = std::fs::OpenOptions::new()
                    .truncate(true)
                    .write(true)
                    .open(&path)
                    .unwrap();

                std::io::Write::write_all(&mut file, serde_json::to_string(&mail)?.as_bytes())?;

                log::info!(
                    target: DELIVER,
                    "vDeliver (deferred) '{}' INCREASE retries to '{}'.",
                    message_id,
                    mail.metadata.as_ref().unwrap().retry
                );
            }
        }
    }

    Ok(())
}

async fn flush_deferred_queue(
    resolvers: &mut HashMap<String, Box<dyn Resolver + Send + Sync>>,
    config: &ServerConfig,
) -> anyhow::Result<()> {
    for path in std::fs::read_dir(Queue::Deferred.to_path(&config.delivery.spool_dir)?)? {
        handle_one_in_deferred_queue(resolvers, &path?.path(), config)
            .await
            .unwrap();
    }

    Ok(())
}
