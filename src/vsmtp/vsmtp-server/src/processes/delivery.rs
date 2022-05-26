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
    log_channels,
    processes::delivery::{
        deferred::flush_deferred_queue,
        deliver::{flush_deliver_queue, handle_one_in_delivery_queue},
    },
};
use anyhow::Context;
use time::format_description::well_known::Rfc2822;
use trust_dns_resolver::TokioAsyncResolver;
use vsmtp_common::{
    mail_context::{MailContext, MessageBody},
    queue::Queue,
    queue_path,
    re::{anyhow, log},
    status::Status,
    transfer::{EmailTransferStatus, Transfer},
};
use vsmtp_config::{Config, Resolvers};
use vsmtp_delivery::transport::{deliver as deliver2, forward, maildir, mbox, Transport};
use vsmtp_rule_engine::rule_engine::RuleEngine;

mod deferred;
mod deliver;

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
pub async fn start(
    config: std::sync::Arc<Config>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    resolvers: std::sync::Arc<Resolvers>,
    mut delivery_receiver: tokio::sync::mpsc::Receiver<ProcessMessage>,
) -> anyhow::Result<()> {
    log::info!(target: log_channels::DELIVERY, "booting, flushing queue.",);

    flush_deliver_queue(&config, &resolvers, &rule_engine).await?;

    let mut flush_deferred_interval =
        tokio::time::interval(config.server.queues.delivery.deferred_retry_period);

    loop {
        tokio::select! {
            Some(pm) = delivery_receiver.recv() => {
                let copy_config = config.clone();
                let copy_rule_engine = rule_engine.clone();
                let copy_resolvers = resolvers.clone();
                tokio::spawn(async move {
                    let path = queue_path!(&copy_config.server.queues.dirpath, Queue::Deliver);

                    if let Err(error) = handle_one_in_delivery_queue(
                        &copy_config,
                        &copy_resolvers,
                        &std::path::PathBuf::from_iter([
                            path,
                            std::path::Path::new(&pm.message_id).to_path_buf(),
                        ]),
                        &copy_rule_engine,
                    )
                    .await {
                        log::error!(target: log_channels::DELIVERY,
                             "(msg={}) could not deliver email: {error:?}", pm.message_id);
                    }
                });

                if cfg!(test) {
                    return Ok(());
                }
            }
            _ = flush_deferred_interval.tick() => {
                log::info!(
                    target: log_channels::DEFERRED,
                    "cronjob delay elapsed, flushing queue.",
                );
                flush_deferred_queue(&config, &resolvers).await?;
            }
        };
    }
}

/// send the email following each recipient transport method.
/// return a list of recipients with updated email_status field.
/// recipients tagged with the Sent email_status are discarded.
async fn send_email(
    config: &Config,
    resolvers: &std::collections::HashMap<String, TokioAsyncResolver>,
    metadata: &vsmtp_common::mail_context::MessageMetadata,
    from: &vsmtp_common::Address,
    to: &[vsmtp_common::rcpt::Rcpt],
    body: &MessageBody,
) -> anyhow::Result<Vec<vsmtp_common::rcpt::Rcpt>> {
    // filtering recipients by domains and delivery method.
    let mut triage = vsmtp_common::rcpt::filter_by_transfer_method(to);

    let content = body.to_string();

    for (method, rcpt) in &mut triage {
        let mut transport: Box<dyn Transport + Send> = match method {
            Transfer::Forward(to) => Box::new(forward::Forward::new(
                to,
                // if we are using an ip the default dns is used.
                match to {
                    vsmtp_common::transfer::ForwardTarget::Domain(domain) => resolvers
                        .get(domain)
                        .unwrap_or_else(|| resolvers.get(&config.server.domain).unwrap()),
                    vsmtp_common::transfer::ForwardTarget::Ip(_) => {
                        resolvers.get(&config.server.domain).unwrap()
                    }
                },
            )),
            Transfer::Deliver => Box::new(deliver2::Deliver::new({
                let domain = rcpt[0].address.domain();
                resolvers
                    .get(domain)
                    .unwrap_or_else(|| resolvers.get(&config.server.domain).unwrap())
            })),
            Transfer::Mbox => Box::new(mbox::MBox),
            Transfer::Maildir => Box::new(maildir::Maildir),
            Transfer::None => continue,
        };

        transport
            .deliver(config, metadata, from, &mut rcpt[..], &content)
            .await
            .with_context(|| {
                format!("failed to deliver email using '{method}' for group '{rcpt:?}'")
            })?;
    }

    // recipient email transfer status could have been updated.
    // we also filter out recipients if they have been sent the message already.
    Ok(triage
        .into_iter()
        .flat_map(|(_, rcpt)| rcpt)
        .filter(|rcpt| !matches!(rcpt.email_status, EmailTransferStatus::Sent))
        .collect::<Vec<_>>())
}

// FIXME: could be optimized by checking both conditions with the same iterator.
/// copy the message into the deferred / dead queue if any recipient is held back or have failed delivery.
fn move_to_queue(config: &Config, ctx: &MailContext) -> anyhow::Result<()> {
    if ctx
        .envelop
        .rcpt
        .iter()
        .any(|rcpt| matches!(rcpt.email_status, EmailTransferStatus::HeldBack(..)))
    {
        Queue::Deferred
            .write_to_queue(&config.server.queues.dirpath, ctx)
            .context("failed to move message from delivery queue to deferred queue")?;
    }

    if ctx.envelop.rcpt.iter().any(|rcpt| {
        matches!(rcpt.email_status, EmailTransferStatus::Failed(..))
            || matches!(rcpt.transfer_method, Transfer::None,)
    }) {
        Queue::Dead
            .write_to_queue(&config.server.queues.dirpath, ctx)
            .context("failed to move message from delivery queue to dead queue")?;
    }

    Ok(())
}

/// prepend trace informations to headers.
/// see https://datatracker.ietf.org/doc/html/rfc5321#section-4.4
fn add_trace_information(
    config: &Config,
    ctx: &mut MailContext,
    rule_engine_result: &Status,
) -> anyhow::Result<()> {
    let metadata = ctx
        .metadata
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("missing email metadata"))?;

    let stamp = create_received_stamp(
        &ctx.envelop.helo,
        &config.server.domain,
        &metadata.message_id,
        &metadata.timestamp,
    )
    .context("failed to create Receive header timestamp")?;

    let vsmtp_status = create_vsmtp_status_stamp(
        &ctx.metadata.as_ref().unwrap().message_id,
        env!("CARGO_PKG_VERSION"),
        rule_engine_result,
    );

    if ctx.body == MessageBody::Empty {
        anyhow::bail!("could not add trace information to email header: body is empty");
    }

    ctx.body.add_header("X-VSMTP", &vsmtp_status);
    ctx.body.add_header("Received", &stamp);

    Ok(())
}

/// create the "Received" header stamp.
fn create_received_stamp(
    client_helo: &str,
    server_domain: &str,
    message_id: &str,
    received_timestamp: &std::time::SystemTime,
) -> anyhow::Result<String> {
    Ok(format!(
        "from {client_helo}\n\tby {server_domain}\n\twith SMTP\n\tid {message_id};\n\t{}",
        {
            let odt: time::OffsetDateTime = (*received_timestamp).into();

            odt.format(&Rfc2822)?
        }
    ))
}

/// create the "X-VSMTP" header stamp.
fn create_vsmtp_status_stamp(message_id: &str, version: &str, status: &Status) -> String {
    format!(
        "id='{}'\n\tversion='{}'\n\tstatus='{}'",
        message_id,
        version,
        status.as_ref()
    )
}

#[cfg(test)]
mod test {
    use super::add_trace_information;
    use vsmtp_common::mail_context::{ConnectionContext, MessageBody};

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
            body: vsmtp_common::mail_context::MessageBody::Empty,
            connection: ConnectionContext {
                timestamp: std::time::SystemTime::UNIX_EPOCH,
                credentials: None,
                is_authenticated: false,
                is_secured: false,
                server_name: "testserver.com".to_string(),
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

        assert_eq!(
            &add_trace_information(&config, &mut ctx, &vsmtp_common::status::Status::Next)
                .unwrap_err()
                .to_string(),
            "could not add trace information to email header: body is empty"
        );

        ctx.body = MessageBody::Raw(vec![]);
        ctx.metadata.as_mut().unwrap().message_id = "test_message_id".to_string();
        add_trace_information(&config, &mut ctx, &vsmtp_common::status::Status::Next).unwrap();

        assert_eq!(
            ctx.body,
            MessageBody::Raw(vec![
                [
                    "Received: from localhost\n".to_string(),
                    format!("\tby {domain}\n", domain = config.server.domain),
                    "\twith SMTP\n".to_string(),
                    format!(
                        "\tid {id};\n",
                        id = ctx.metadata.as_ref().unwrap().message_id
                    ),
                    format!(
                        "\t{odt}",
                        odt = {
                            let odt: time::OffsetDateTime =
                                ctx.metadata.as_ref().unwrap().timestamp.into();
                            odt.format(&time::format_description::well_known::Rfc2822)
                                .unwrap()
                        }
                    ),
                ]
                .concat(),
                [
                    format!(
                        "X-VSMTP: id='{id}'\n",
                        id = ctx.metadata.as_ref().unwrap().message_id
                    ),
                    format!("\tversion='{}'\n", env!("CARGO_PKG_VERSION")),
                    "\tstatus='next'".to_string()
                ]
                .concat()
            ])
        );
    }
}
