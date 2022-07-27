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
    channel_message::ProcessMessage,
    delivery::{
        deferred::flush_deferred_queue,
        deliver::{flush_deliver_queue, handle_one_in_delivery_queue},
    },
};
use anyhow::Context;
use time::format_description::well_known::Rfc2822;
use trust_dns_resolver::TokioAsyncResolver;
use vsmtp_common::{
    mail_context::MailContext,
    rcpt::Rcpt,
    re::{anyhow, log},
    status::Status,
    transfer::{ForwardTarget, Transfer},
    MessageBody,
};
use vsmtp_common::{re::tokio, transfer::EmailTransferStatus};
use vsmtp_config::{Config, Resolvers};
use vsmtp_delivery::transport::{Deliver, Forward, MBox, Maildir, Transport};
use vsmtp_rule_engine::RuleEngine;

mod deferred;
mod deliver;

/// process used to deliver incoming emails force accepted by the smtp process
/// or parsed by the vMime process.
pub async fn start(
    config: std::sync::Arc<Config>,
    rule_engine: std::sync::Arc<RuleEngine>,
    resolvers: std::sync::Arc<Resolvers>,
    mut delivery_receiver: tokio::sync::mpsc::Receiver<ProcessMessage>,
) {
    if let Err(e) =
        flush_deliver_queue(config.clone(), resolvers.clone(), rule_engine.clone()).await
    {
        log::error!("flushing queue failed: `{e}`");
    }

    // NOTE: emails stored in the deferred queue are likely to slow down the process.
    //       the pickup process of this queue should be slower than pulling from the delivery queue.
    //       https://www.postfix.org/QSHAPE_README.html#queues
    let mut flush_deferred_interval =
        tokio::time::interval(config.server.queues.delivery.deferred_retry_period);

    loop {
        tokio::select! {
            Some(pm) = delivery_receiver.recv() => {
                tokio::spawn(
                    handle_one_in_delivery_queue(
                        config.clone(),
                        resolvers.clone(),
                        pm,
                        rule_engine.clone(),
                    )
                );
            }
            _ = flush_deferred_interval.tick() => {
                log::info!("cronjob delay elapsed `{}s`, flushing queue.",
                    config.server.queues.delivery.deferred_retry_period.as_secs());
                tokio::spawn(flush_deferred_queue(config.clone(), resolvers.clone()));
            }
        };
    }
}

#[must_use]
pub enum SenderOutcome {
    MoveToDead,
    MoveToDeferred,
    RemoveFromDisk,
}

#[allow(clippy::too_many_lines)]
pub async fn send_mail(
    config: &Config,
    message_ctx: &mut MailContext,
    message_body: &MessageBody,
    resolvers: &std::collections::HashMap<String, TokioAsyncResolver>,
) -> SenderOutcome {
    let mut acc: std::collections::HashMap<Transfer, Vec<Rcpt>> = std::collections::HashMap::new();
    for i in message_ctx
        .envelop
        .rcpt
        .iter()
        .filter(|r| r.email_status.is_sendable())
    {
        acc.entry(i.transfer_method.clone())
            .and_modify(|domain| domain.push(i.clone()))
            .or_insert_with(|| vec![i.clone()]);
    }

    if acc.is_empty() {
        return SenderOutcome::MoveToDead;
    }

    let message_content = message_body.inner().to_string();

    let root_server_resolver = resolvers
        .get(&config.server.domain)
        .expect("root server's resolver is missing");

    let metadata = &message_ctx.metadata.as_ref().unwrap();
    let from = &message_ctx.envelop.mail_from;

    let futures = acc
        .into_iter()
        .filter(|(key, _)| !matches!(key, Transfer::None))
        .map(|(key, to)| match key {
            Transfer::Forward(forward_target) => {
                let resolver = match &forward_target {
                    ForwardTarget::Domain(domain) => resolvers.get(domain),
                    ForwardTarget::Ip(_) | ForwardTarget::Socket(_) => None,
                }
                .unwrap_or(root_server_resolver);

                Forward::new(forward_target.clone(), resolver).deliver(
                    config,
                    metadata,
                    from,
                    to,
                    &message_content,
                )
            }
            Transfer::Deliver => Deliver::new({
                resolvers
                    .get(
                        to.get(0)
                            .expect("at least one element in the group")
                            .address
                            .domain(),
                    )
                    .unwrap_or(root_server_resolver)
            })
            .deliver(config, metadata, from, to, &message_content),
            Transfer::Mbox => MBox.deliver(config, metadata, from, to, &message_content),
            Transfer::Maildir => Maildir.deliver(config, metadata, from, to, &message_content),
            Transfer::None => unreachable!(),
        })
        .collect::<Vec<_>>();

    message_ctx.envelop.rcpt = futures::future::join_all(futures)
        .await
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    log::trace!("the recipients are: `{:#?}`", message_ctx.envelop.rcpt);

    // updating retry count, set status to Failed if threshold reached.
    for rcpt in &mut message_ctx.envelop.rcpt {
        if matches!(&rcpt.email_status, EmailTransferStatus::HeldBack{errors}
            if errors.len() >= config.server.queues.delivery.deferred_retry_max)
        {
            rcpt.email_status = EmailTransferStatus::Failed {
                timestamp: std::time::SystemTime::now(),
                reason: format!(
                    "maximum retry count of '{}' reached",
                    config.server.queues.delivery.deferred_retry_max
                ),
            };
        }
    }

    // If there is no recipient, or no transfer method for each of them
    if message_ctx.envelop.rcpt.is_empty()
        || message_ctx
            .envelop
            .rcpt
            .iter()
            .all(|rcpt| matches!(rcpt.transfer_method, Transfer::None))
    {
        return SenderOutcome::MoveToDead;
    }

    // If all has been successfully sent
    if message_ctx
        .envelop
        .rcpt
        .iter()
        .all(|rcpt| matches!(rcpt.email_status, EmailTransferStatus::Sent { .. }))
    {
        return SenderOutcome::RemoveFromDisk;
    }

    // If there is no more sendable recipient
    if message_ctx
        .envelop
        .rcpt
        .iter()
        .all(|rcpt| !rcpt.email_status.is_sendable())
    {
        return SenderOutcome::MoveToDead;
    }

    for i in &mut message_ctx.envelop.rcpt {
        if matches!(&i.email_status, EmailTransferStatus::Waiting { .. }) {
            i.email_status
                .held_back("ignored by delivery transport".to_string());
        }
    }

    SenderOutcome::MoveToDeferred
}

/// prepend trace informations to headers.
/// see <https://datatracker.ietf.org/doc/html/rfc5321#section-4.4>
fn add_trace_information(
    config: &Config,
    ctx: &MailContext,
    message: &mut MessageBody,
    rule_engine_result: &Status,
) -> anyhow::Result<()> {
    let metadata = ctx
        .metadata
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("missing email metadata"))?;

    message.prepend_header(
        "X-VSMTP",
        &create_vsmtp_status_stamp(
            &metadata.message_id,
            env!("CARGO_PKG_VERSION"),
            rule_engine_result,
        ),
    );
    message.prepend_header(
        "Received",
        &create_received_stamp(
            &ctx.envelop.helo,
            &config.server.domain,
            &metadata.message_id,
            &metadata.timestamp,
        )
        .context("failed to create Receive header timestamp")?,
    );

    Ok(())
}

/// create the "Received" header stamp.
fn create_received_stamp(
    client_helo: &str,
    server_domain: &str,
    message_id: &str,
    received_timestamp: &std::time::SystemTime,
) -> anyhow::Result<String> {
    let odt: time::OffsetDateTime = (*received_timestamp).into();
    let date = odt.format(&Rfc2822)?;
    Ok(format!(
        "from {client_helo} by {server_domain} with SMTP id {message_id}; {date}"
    ))
}

/// create the "X-VSMTP" header stamp.
fn create_vsmtp_status_stamp(message_id: &str, version: &str, status: &Status) -> String {
    format!(
        "id=\"{message_id}\"; version=\"{version}\"; status=\"{}\"",
        status.as_ref()
    )
}

#[cfg(test)]
mod test {
    use super::add_trace_information;
    use vsmtp_common::{mail_context::ConnectionContext, status::Status, MessageBody, RawBody};

    /*
    /// This test produce side-effect and may make other test fails
    #[tokio::test(flavor = "multi_thread", worker_threads = 3)]
    async fn start() {
        let mut config = config::local_test();
        config.server.queues.dirpath = "./tmp".into();

        let rule_engine = std::sync::Arc::new(std::sync::RwLock::new(
            RuleEngine::from_script("#{}").unwrap(),
        ));

        let (delivery_sender, delivery_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(10);

        let task = tokio::spawn(super::start(
            std::sync::Arc::new(config),
            rule_engine,
            delivery_receiver,
        ));

        delivery_sender
            .send(ProcessMessage {
                message_id: "test".to_string(),
            })
            .await
            .unwrap();

        task.await.unwrap().unwrap();
    }
    */

    #[test]
    fn test_add_trace_information() {
        let mut ctx = vsmtp_common::mail_context::MailContext {
            connection: ConnectionContext {
                timestamp: std::time::SystemTime::UNIX_EPOCH,
                credentials: None,
                is_authenticated: false,
                is_secured: false,
                server_name: "testserver.com".to_string(),
                server_address: "127.0.0.1:25".parse().unwrap(),
            },
            client_addr: std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
                0,
            ),
            envelop: vsmtp_common::envelop::Envelop {
                helo: "localhost".to_string(),
                mail_from: vsmtp_common::addr!("a@a.a"),
                rcpt: vec![],
            },
            metadata: Some(vsmtp_common::mail_context::MessageMetadata {
                timestamp: std::time::SystemTime::UNIX_EPOCH,
                ..vsmtp_common::mail_context::MessageMetadata::default()
            }),
        };

        let config = vsmtp_config::Config::default();

        let mut message = MessageBody::default();
        ctx.metadata.as_mut().unwrap().message_id = "test_message_id".to_string();
        add_trace_information(&config, &ctx, &mut message, &Status::Next).unwrap();

        pretty_assertions::assert_eq!(
            *message.inner(),
            RawBody::new_empty(vec![
                [
                    "Received: from localhost".to_string(),
                    format!(" by {domain}", domain = config.server.domain),
                    " with SMTP".to_string(),
                    format!(" id {id}; ", id = ctx.metadata.as_ref().unwrap().message_id),
                    {
                        let odt: time::OffsetDateTime =
                            ctx.metadata.as_ref().unwrap().timestamp.into();
                        odt.format(&time::format_description::well_known::Rfc2822)
                            .unwrap()
                    }
                ]
                .concat(),
                format!(
                    "X-VSMTP: id=\"{id}\"; version=\"{ver}\"; status=\"next\"",
                    id = ctx.metadata.as_ref().unwrap().message_id,
                    ver = env!("CARGO_PKG_VERSION"),
                ),
            ])
        );
    }
}
