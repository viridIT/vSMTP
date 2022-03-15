/**
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
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
use super::ProcessMessage;
use crate::{queue::Queue, resolver::Resolver};
use anyhow::Context;
use vsmtp_common::{
    mail_context::{Body, MailContext},
    status::Status,
};
use vsmtp_config::{log_channel::DELIVER, ServerConfig};
use vsmtp_rule_engine::rule_engine::{RuleEngine, RuleState};

/// process used to deliver incoming emails force accepted by the smtp process
/// or parsed by the vMime process.
///
/// # Errors
///
/// *
///
/// # Panics
///
/// * tokio::select!
pub async fn start<S: std::hash::BuildHasher + Send>(
    config: std::sync::Arc<ServerConfig>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    mut resolvers: std::collections::HashMap<
        vsmtp_common::transfer::Transfer,
        Box<dyn Resolver + Send + Sync>,
        S,
    >,
    mut delivery_receiver: tokio::sync::mpsc::Receiver<ProcessMessage>,
) -> anyhow::Result<()> {
    log::info!(
        target: DELIVER,
        "vDeliver (delivery) booting, flushing queue.",
    );
    flush_deliver_queue(&config, &rule_engine, &mut resolvers).await?;

    let mut flush_deferred_interval = tokio::time::interval(
        config
            .delivery
            .queues
            .deferred
            .cron_period
            .unwrap_or_else(|| std::time::Duration::from_secs(10)),
    );

    loop {
        tokio::select! {
            Some(pm) = delivery_receiver.recv() => {
                // FIXME: resolvers a are mutable, so must be in a mutex
                // for a delivery in a separated thread...
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
                flush_deferred_queue(&mut resolvers, &config).await?;
            }
        };
    }
}

/// filter recipients by their transfer method and domain name.
fn filter_recipients<S: std::hash::BuildHasher + Send>(
    ctx: &mut MailContext,
    resolvers: &std::collections::HashMap<
        vsmtp_common::transfer::Transfer,
        Box<dyn Resolver + Send + Sync>,
        S,
    >,
) -> std::collections::HashMap<vsmtp_common::transfer::Transfer, Vec<vsmtp_common::rcpt::Rcpt>> {
    ctx.envelop
        .rcpt
        .iter_mut()
        .fold(std::collections::HashMap::new(), |mut acc, rcpt| {
            if !resolvers.contains_key(&rcpt.transfer_method) {
                rcpt.email_status = vsmtp_common::transfer::EmailTransferStatus::Failed(format!(
                    // FIXME: find a better to couple Transfer method with Resolvers.
                    "{} transfer method does not have a delivery system setup",
                    rcpt.transfer_method.as_str()
                ));
                return acc;
            }

            if let Some(group) = acc.get_mut(&rcpt.transfer_method) {
                group.push(rcpt.clone());
            } else {
                acc.insert(rcpt.transfer_method, vec![rcpt.clone()]);
            }

            acc
        })
}

/// handle one email pulled from the delivery queue.
///
/// # Panics
///
/// # Errors
pub async fn handle_one_in_delivery_queue<S: std::hash::BuildHasher + Send>(
    config: &ServerConfig,
    path: &std::path::Path,
    rule_engine: &std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    resolvers: &mut std::collections::HashMap<
        vsmtp_common::transfer::Transfer,
        Box<dyn Resolver + Send + Sync>,
        S,
    >,
) -> anyhow::Result<()> {
    let message_id = path.file_name().and_then(std::ffi::OsStr::to_str).unwrap();

    log::trace!(
        target: DELIVER,
        "vDeliver (delivery) RECEIVED '{}'",
        message_id
    );

    let mut file = std::fs::OpenOptions::new().read(true).open(&path)?;

    let mut raw =
        String::with_capacity(usize::try_from(file.metadata().unwrap().len()).unwrap_or(0));
    std::io::Read::read_to_string(&mut file, &mut raw)?;

    let ctx: MailContext = serde_json::from_str(&raw)?;
    let mut state = RuleState::with_context(config, ctx);

    let result = rule_engine
        .read()
        .map_err(|_| anyhow::anyhow!("rule engine mutex poisoned"))?
        .run_when(&mut state, "delivery");

    if result == Status::Deny {
        Queue::Dead.write_to_queue(config, &state.get_context().read().unwrap())?;
    } else {
        let (metadata, from, mut triage, content) = {
            // FIXME: handle poison & missing metadata errors.
            let ctx = state.get_context();
            let mut ctx = ctx.write().unwrap();

            // filtering recipients by domains and delivery method.
            let triage = filter_recipients(&mut *ctx, resolvers);

            let content = match &ctx.body {
                Body::Empty => todo!("empty body should not be possible in delivery"),
                Body::Raw(raw) => raw.clone(),
                Body::Parsed(parsed) => parsed.to_raw(),
            };

            let metadata = ctx.metadata.as_ref().unwrap().clone();

            (metadata, ctx.envelop.mail_from.clone(), triage, content)
        };

        for (method, rcpt) in &mut triage {
            println!("'{method}' for '{rcpt:?}'");
            resolvers
                .get_mut(method)
                .unwrap()
                .deliver(config, &metadata, &from, &mut rcpt[..], &content)
                .await
                .with_context(|| {
                    format!("failed to deliver email using '{method}' for '{rcpt:?}'")
                })?;
        }

        // for rcpt in &ctx.envelop.rcpt {
        //     if rcpt.transfer_method == vsmtp_common::rcpt::NO_DELIVERY {
        //         Queue::Dead.write_to_queue(config, &ctx)?;
        //         continue;
        //     }

        //     let resolver = if let Some(resolver) = resolvers.get_mut(&rcpt.transfer_method) {
        //         resolver
        //     } else {
        //         log::trace!(
        //             target: DELIVER,
        //             "vDeliver (deferred) delivery method '{}' for '{}' not found",
        //             rcpt.transfer_method,
        //             rcpt.address
        //         );

        //         // TODO: set in deferred, add metadata for a particular rcpt.

        //         continue;
        //     };

        //     match resolver.deliver(config, &ctx, rcpt).await {
        //         Ok(_) => {
        //             log::trace!(
        //                 target: DELIVER,
        //                 "vDeliver (delivery) '{}' SEND successfully.",
        //                 message_id
        //             );

        //             std::fs::remove_file(&path)?;

        //             log::info!(
        //                 target: DELIVER,
        //                 "vDeliver (delivery) '{}' REMOVED successfully.",
        //                 message_id
        //             );
        //         }
        //         Err(error) => {
        //             log::warn!(
        //                 target: DELIVER,
        //                 "vDeliver (delivery) '{}' SEND FAILED, reason: '{}'",
        //                 message_id,
        //                 error
        //             );

        //             std::fs::rename(
        //                 path,
        //                 std::path::PathBuf::from_iter([
        //                     Queue::Deferred.to_path(&config.delivery.spool_dir)?,
        //                     std::path::Path::new(&message_id).to_path_buf(),
        //                 ]),
        //             )?;

        //             log::info!(
        //                 target: DELIVER,
        //                 "vDeliver (delivery) '{}' MOVED delivery => deferred.",
        //                 message_id
        //             );
        //         }
        //     }
        // }
    };

    Ok(())
}

async fn flush_deliver_queue<S: std::hash::BuildHasher + Send>(
    config: &ServerConfig,
    rule_engine: &std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    resolvers: &mut std::collections::HashMap<
        vsmtp_common::transfer::Transfer,
        Box<dyn Resolver + Send + Sync>,
        S,
    >,
) -> anyhow::Result<()> {
    for path in std::fs::read_dir(Queue::Deliver.to_path(&config.delivery.spool_dir)?)? {
        handle_one_in_delivery_queue(config, &path?.path(), rule_engine, resolvers).await?;
    }

    Ok(())
}

async fn handle_one_in_deferred_queue<S: std::hash::BuildHasher + Send>(
    resolvers: &mut std::collections::HashMap<
        vsmtp_common::transfer::Transfer,
        Box<dyn Resolver + Send + Sync>,
        S,
    >,
    path: &std::path::Path,
    config: &ServerConfig,
) -> anyhow::Result<()> {
    let message_id = path.file_name().and_then(std::ffi::OsStr::to_str).unwrap();

    log::debug!(
        target: DELIVER,
        "vDeliver (deferred) received email '{}'",
        message_id
    );

    let mut file = std::fs::OpenOptions::new().read(true).open(&path)?;

    let mut raw =
        String::with_capacity(usize::try_from(file.metadata().unwrap().len()).unwrap_or(0));
    std::io::Read::read_to_string(&mut file, &mut raw)?;

    let mut ctx: MailContext = serde_json::from_str(&raw)?;

    let max_retry_deferred = config.delivery.queues.deferred.retry_max.unwrap_or(100);

    if ctx.metadata.is_none() {
        anyhow::bail!("email metadata is missing")
    }

    // for rcpt in &mut ctx.envelop.rcpt {
    //     if rcpt.retry >= max_retry_deferred {
    //         // TODO: move to dead queue.
    //         continue;
    //     }

    //     let resolver = if let Some(resolver) = resolvers.get_mut(&rcpt.transfer_method) {
    //         resolver
    //     } else {
    //         log::trace!(
    //             target: DELIVER,
    //             "vDeliver (deferred) delivery method '{}' for '{}' not found",
    //             rcpt.transfer_method,
    //             rcpt.address
    //         );

    //         // TODO: set in dead.

    //         continue;
    //     };

    //     match resolver.deliver(config, &ctx, rcpt).await {
    //         Ok(_) => {
    //             log::debug!(
    //                 target: DELIVER,
    //                 "vDeliver (deferred) '{}' email sent successfully.",
    //                 message_id
    //             );

    //             std::fs::remove_file(&path)?;
    //         }
    //         Err(error) => {
    //             log::warn!(
    //                 target: DELIVER,
    //                 "vDeliver (deferred) '{}' failed to send email, reason: '{}'",
    //                 message_id,
    //                 error
    //             );

    //             rcpt.retry += 1;

    //             let mut file = std::fs::OpenOptions::new()
    //                 .truncate(true)
    //                 .write(true)
    //                 .open(&path)
    //                 .unwrap();

    //             std::io::Write::write_all(&mut file, serde_json::to_string(&ctx)?.as_bytes())?;

    //             log::debug!(
    //                 target: DELIVER,
    //                 "vDeliver (deferred) '{}' increased retries to '{}'.",
    //                 message_id,
    //                 rcpt.retry
    //             );
    //         }
    //     }
    // }

    Ok(())
}

async fn flush_deferred_queue<S: std::hash::BuildHasher + Send>(
    resolvers: &mut std::collections::HashMap<
        vsmtp_common::transfer::Transfer,
        Box<dyn Resolver + Send + Sync>,
        S,
    >,
    config: &ServerConfig,
) -> anyhow::Result<()> {
    for path in std::fs::read_dir(Queue::Deferred.to_path(&config.delivery.spool_dir)?)? {
        handle_one_in_deferred_queue(resolvers, &path?.path(), config).await?;
    }

    Ok(())
}
