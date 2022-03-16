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
use crate::{queue::Queue, transport::Transport};
use anyhow::Context;
use vsmtp_common::{
    mail_context::{Body, MailContext},
    status::Status,
    transfer::EmailTransferStatus,
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
        Box<dyn Transport + Send + Sync>,
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
/// the context is mutable because resolvers could not be correctly setup.
/// FIXME: find a better to couple Transfer methods with Resolvers.
///        that way, the email status would never be failed at this stage.
fn filter_recipients<S: std::hash::BuildHasher + Send>(
    ctx: &mut MailContext,
    resolvers: &std::collections::HashMap<
        vsmtp_common::transfer::Transfer,
        Box<dyn Transport + Send + Sync>,
        S,
    >,
) -> std::collections::HashMap<vsmtp_common::transfer::Transfer, Vec<vsmtp_common::rcpt::Rcpt>> {
    ctx.envelop
        .rcpt
        .iter_mut()
        .fold(std::collections::HashMap::new(), |mut acc, rcpt| {
            if !resolvers.contains_key(&rcpt.transfer_method) {
                rcpt.email_status = vsmtp_common::transfer::EmailTransferStatus::Failed(format!(
                    "{} transfer method does not have a transfer system setup",
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
        Box<dyn Transport + Send + Sync>,
        S,
    >,
) -> anyhow::Result<()> {
    let message_id = path.file_name().and_then(std::ffi::OsStr::to_str).unwrap();

    log::trace!(
        target: DELIVER,
        "vDeliver (delivery) RECEIVED '{}'",
        message_id
    );

    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let ctx: MailContext = serde_json::from_reader(reader)?;
    let mut state = RuleState::with_context(config, ctx);

    let result = rule_engine
        .read()
        .map_err(|_| anyhow::anyhow!("rule engine mutex poisoned"))?
        .run_when(&mut state, "delivery");

    // NOTE: should the engine able to return a status for a particular recipient ?
    if result == Status::Deny {
        // we update rcpt email status and write to dead queue in case of a deny.
        let ctx = state.get_context();
        let mut ctx = ctx.write().unwrap();

        for rcpt in &mut ctx.envelop.rcpt {
            rcpt.email_status =
                EmailTransferStatus::Failed("rule engine denied the email.".to_string());
        }
        Queue::Dead.write_to_queue(config, &ctx)?;
    } else {
        // we pickup a copy of the metadata and envelop of the context, so we can dispatch emails
        // to send by groups of recipients (grouped by transfer + destination)
        let (metadata, from, mut triage, content) = {
            // FIXME: handle poison & missing metadata errors.
            let ctx = state.get_context();
            let mut ctx = ctx.write().unwrap();

            // filtering recipients by domains and delivery method.
            let triage = filter_recipients(&mut *ctx, resolvers);

            // getting a raw copy of the email.
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

        let ctx = state.get_context();
        let mut ctx = ctx.write().unwrap();

        // FIXME: handle poison & missing metadata errors.
        // recipient email transfer status could have been updated.
        ctx.envelop.rcpt = triage
            .into_iter()
            .flat_map(|(_, rcpt)| {
                // FIXME: disk i/o could be avoided here by filtering rcpt statuses.
                // TODO: email should be written with updated ctx.
                for rcpt in &rcpt {
                    match &rcpt.email_status {
                        vsmtp_common::transfer::EmailTransferStatus::HeldBack(_) => {
                            std::fs::rename(
                                path,
                                std::path::PathBuf::from_iter([
                                    Queue::Deferred.to_path(&config.delivery.spool_dir).unwrap(),
                                    std::path::Path::new(&message_id).to_path_buf(),
                                ]),
                            )
                            .unwrap();
                        }
                        vsmtp_common::transfer::EmailTransferStatus::Failed(_) => std::fs::rename(
                            path,
                            std::path::PathBuf::from_iter([
                                Queue::Dead.to_path(&config.delivery.spool_dir).unwrap(),
                                std::path::Path::new(&message_id).to_path_buf(),
                            ]),
                        )
                        .unwrap(),
                        // Sent or Waiting (waiting should never happen), we can remove the file later.
                        _ => {}
                    }
                }

                rcpt
            })
            .collect();

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

    // after processing the email is removed from the delivery queue.
    std::fs::remove_file(path)?;

    Ok(())
}

async fn flush_deliver_queue<S: std::hash::BuildHasher + Send>(
    config: &ServerConfig,
    rule_engine: &std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    resolvers: &mut std::collections::HashMap<
        vsmtp_common::transfer::Transfer,
        Box<dyn Transport + Send + Sync>,
        S,
    >,
) -> anyhow::Result<()> {
    for path in std::fs::read_dir(Queue::Deliver.to_path(&config.delivery.spool_dir)?)? {
        handle_one_in_delivery_queue(config, &path?.path(), rule_engine, resolvers).await?;
    }

    Ok(())
}

// NOTE: emails stored in the deferred queue are lickly to slow down the process.
//       the pickup process of this queue should be slower than pulling from the delivery queue.
//       https://www.postfix.org/QSHAPE_README.html#queues
async fn handle_one_in_deferred_queue<S: std::hash::BuildHasher + Send>(
    resolvers: &mut std::collections::HashMap<
        vsmtp_common::transfer::Transfer,
        Box<dyn Transport + Send + Sync>,
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
        Box<dyn Transport + Send + Sync>,
        S,
    >,
    config: &ServerConfig,
) -> anyhow::Result<()> {
    for path in std::fs::read_dir(Queue::Deferred.to_path(&config.delivery.spool_dir)?)? {
        handle_one_in_deferred_queue(resolvers, &path?.path(), config).await?;
    }

    Ok(())
}
