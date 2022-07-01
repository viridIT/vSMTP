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
    log_channels,
};
use anyhow::Context;
use time::format_description::well_known::Rfc2822;
use trust_dns_resolver::TokioAsyncResolver;
use vsmtp_common::re::tokio;
use vsmtp_common::{
    mail_context::{MailContext, MessageBody},
    rcpt::Rcpt,
    re::{anyhow, log},
    status::Status,
    transfer::{ForwardTarget, Transfer},
};
use vsmtp_config::{Config, Resolvers};
use vsmtp_delivery::transport::{deliver as smtp_deliver, forward, maildir, mbox, Transport};
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
pub async fn start(
    config: std::sync::Arc<Config>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    resolvers: std::sync::Arc<Resolvers>,
    mut delivery_receiver: tokio::sync::mpsc::Receiver<ProcessMessage>,
) {
    if let Err(e) =
        flush_deliver_queue(config.clone(), resolvers.clone(), rule_engine.clone()).await
    {
        log::error!(
            target: log_channels::DELIVERY,
            "flushing queue failed: {}",
            e
        );
    }

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
                log::info!(
                    target: log_channels::DEFERRED,
                    "cronjob delay elapsed, flushing queue.",
                );
                tokio::spawn(
                    flush_deferred_queue(config.clone(), resolvers.clone())
                );
            }
        };
    }
}

pub async fn send_mail(
    config: &Config,
    message_ctx: &mut MailContext,
    message_body: &MessageBody,
    resolvers: &std::collections::HashMap<String, TokioAsyncResolver>,
) {
    let mut acc: std::collections::HashMap<Transfer, Vec<Rcpt>> = std::collections::HashMap::new();
    for i in message_ctx
        .envelop
        .rcpt
        .iter()
        .filter(|r| r.email_status.is_sendable())
        .cloned()
    {
        if let Some(group) = acc.get_mut(&i.transfer_method) {
            group.push(i);
        } else {
            acc.insert(i.transfer_method.clone(), vec![i]);
        }
    }

    if acc.is_empty() {
        // TODO!
        return;
    }

    let message_content = message_body.to_string();

    let root_server_resolver = resolvers
        .get(&config.server.domain)
        .expect("root server's resolver is missing");

    let metadata = &message_ctx.metadata.as_ref().unwrap();
    let from = &message_ctx.envelop.mail_from;

    let mut updated_group = vec![];
    for (key, group) in acc {
        let mut transport: Box<dyn Transport + Send> = match &key {
            Transfer::Forward(forward_target) => {
                let resolver = match &forward_target {
                    ForwardTarget::Domain(domain) => resolvers.get(domain),
                    ForwardTarget::Ip(_) | ForwardTarget::Socket(_) => None,
                }
                .unwrap_or(root_server_resolver);

                Box::new(forward::Forward::new(forward_target.clone(), resolver))
            }
            Transfer::Deliver => Box::new(smtp_deliver::Deliver::new({
                resolvers
                    .get(
                        group
                            .get(0)
                            .expect("at least one element in the group")
                            .address
                            .domain(),
                    )
                    .unwrap_or(root_server_resolver)
            })),
            Transfer::Mbox => Box::new(mbox::MBox),
            Transfer::Maildir => Box::new(maildir::Maildir),
            Transfer::None => continue,
        };

        // TODO: make the futures run concurrently
        updated_group.extend(
            transport
                .deliver(config, metadata, from, group, &message_content)
                .await,
        );
    }

    log::info!(target: log_channels::DEFERRED, "{updated_group:#?}");

    message_ctx.envelop.rcpt = updated_group;
}

/*
/// send the email following each recipient transport method.
/// return a list of recipients with updated `email_status` field.
/// recipients tagged with the Sent `email_status` are discarded.
async fn send_email(
    config: &Config,
    resolvers: &std::collections::HashMap<String, TokioAsyncResolver>,
    metadata: &MessageMetadata,
    from: &Address,
    to: &[Rcpt],
    body: &MessageBody,
) -> anyhow::Result<Vec<Rcpt>> {
    // filtering recipients by domains and delivery method.
    let mut triage = vsmtp_common::rcpt::filter_by_transfer_method(to);

    let content = body.to_string();

    for (method, rcpt) in &mut triage {
        let mut transport: Box<dyn Transport + Send> = match method {
            Transfer::Forward(to) => Box::new(forward::Forward::new(
                to,
                // if we are using an ip the default dns is used.
                match to {
                    ForwardTarget::Domain(domain) => resolvers
                        .get(domain)
                        .unwrap_or_else(|| resolvers.get(&config.server.domain).unwrap()),
                    ForwardTarget::Ip(_) | ForwardTarget::Socket(_) => {
                        resolvers.get(&config.server.domain).unwrap()
                    }
                },
            )),
            Transfer::Deliver => Box::new(smtp_deliver::Deliver::new({
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
*/

/// prepend trace informations to headers.
/// see <https://datatracker.ietf.org/doc/html/rfc5321#section-4.4>
fn add_trace_information(
    config: &Config,
    ctx: &mut MailContext,
    message: &mut MessageBody,
    rule_engine_result: &Status,
) -> anyhow::Result<()> {
    let metadata = ctx
        .metadata
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("missing email metadata"))?;

    message.append_header(
        "X-VSMTP",
        &create_vsmtp_status_stamp(
            &metadata.message_id,
            env!("CARGO_PKG_VERSION"),
            rule_engine_result,
        ),
    );
    message.append_header(
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
        "id='{message_id}' version='{version}' status='{}'",
        status.as_ref()
    )
}

#[cfg(test)]
mod test {
    use super::add_trace_information;
    use vsmtp_common::{
        mail_context::{ConnectionContext, MessageBody},
        status::Status,
    };

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

        let mut message = MessageBody::Raw {
            headers: vec![],
            body: Some("".to_string()),
        };
        ctx.metadata.as_mut().unwrap().message_id = "test_message_id".to_string();
        add_trace_information(&config, &mut ctx, &mut message, &Status::Next).unwrap();

        pretty_assertions::assert_eq!(
            message,
            MessageBody::Raw {
                headers: vec![
                    format!(
                        "X-VSMTP: id='{id}' version='{ver}' status='next'",
                        id = ctx.metadata.as_ref().unwrap().message_id,
                        ver = env!("CARGO_PKG_VERSION"),
                    ),
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
                ],
                body: Some("".to_string()),
            }
        );
    }
}
