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
use crate::{
    config::server_config::InnerSmtpsConfig,
    processes::ProcessMessage,
    queue::Queue,
    rules::rule_engine::RuleEngine,
    smtp::{code::SMTPReplyCode, mail::MailContext},
};

use self::{
    connection::{Connection, ConnectionKind},
    io_service::IoService,
    transaction::{Transaction, TransactionResult},
};

use std::sync::{Arc, RwLock};

pub(crate) mod connection;
pub(crate) mod io_service;
pub(crate) mod transaction;

#[cfg(test)]
mod tests;

// NOTE: not marked as #[cfg(test)] because it is used by the bench/fuzz
/// boilerplate for the tests
pub mod test_helpers;

fn is_version_requirement_satisfied(
    conn: &rustls::ServerConnection,
    config: &InnerSmtpsConfig,
) -> bool {
    let protocol_version_requirement = config
        .sni_maps
        .as_ref()
        .and_then(|map| {
            if let Some(sni) = conn.sni_hostname() {
                for i in map {
                    if i.domain == sni {
                        return Some(i);
                    }
                }
            }
            None
        })
        .and_then(|i| i.protocol_version.as_ref())
        .unwrap_or(&config.protocol_version);

    conn.protocol_version().map_or(false, |protocol_version| {
        protocol_version_requirement
            .0
            .iter()
            .filter(|i| i.0 == protocol_version)
            .count()
            != 0
    })
}

async fn on_mail<S: std::io::Read + std::io::Write>(
    conn: &mut Connection<'_, S>,
    mail: Box<MailContext>,
    helo_domain: &mut Option<String>,
    working_sender: std::sync::Arc<tokio::sync::mpsc::Sender<ProcessMessage>>,
    delivery_sender: std::sync::Arc<tokio::sync::mpsc::Sender<ProcessMessage>>,
) -> anyhow::Result<()> {
    *helo_domain = Some(mail.envelop.helo.clone());

    match &mail.metadata {
        // quietly skipping mime & delivery processes when there is no resolver.
        // (in case of a quarantine for example)
        Some(metadata) if metadata.resolver == "none" => {
            log::warn!("delivery skipped due to NO_DELIVERY action call.");
            conn.send_code(SMTPReplyCode::Code250)?;
        }
        Some(metadata) if metadata.skipped.is_some() => {
            log::warn!("postq skipped due to {:?}.", metadata.skipped.unwrap());
            match Queue::Deliver.write_to_queue(&conn.config, &mail) {
                Ok(_) => {
                    delivery_sender
                        .send(ProcessMessage {
                            message_id: mail.metadata.as_ref().unwrap().message_id.clone(),
                        })
                        .await?;

                    conn.send_code(SMTPReplyCode::Code250)?;
                }
                Err(error) => {
                    log::error!("couldn't write to delivery queue: {}", error);
                    conn.send_code(SMTPReplyCode::Code554)?;
                }
            };
        }
        _ => {
            match Queue::Working.write_to_queue(&conn.config, &mail) {
                Ok(_) => {
                    working_sender
                        .send(ProcessMessage {
                            message_id: mail.metadata.as_ref().unwrap().message_id.clone(),
                        })
                        .await?;

                    conn.send_code(SMTPReplyCode::Code250)?;
                }
                Err(error) => {
                    log::error!("couldn't write to queue: {}", error);
                    conn.send_code(SMTPReplyCode::Code554)?;
                }
            };
        }
    };

    Ok(())
}

/// Receives the incomings mail of a connection
///
/// # Errors
///
/// * server failed to send a message
/// * a transaction failed
/// * the pre-queue processing of the mail failed
pub async fn handle_connection<S>(
    conn: &mut Connection<'_, S>,
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    rule_engine: Arc<RwLock<RuleEngine>>,
    working_sender: std::sync::Arc<tokio::sync::mpsc::Sender<ProcessMessage>>,
    delivery_sender: std::sync::Arc<tokio::sync::mpsc::Sender<ProcessMessage>>,
) -> anyhow::Result<()>
where
    S: std::io::Read + std::io::Write,
{
    let mut helo_domain = None;

    conn.send_code(SMTPReplyCode::Code220)?;

    while conn.is_alive {
        match Transaction::receive(conn, &helo_domain, rule_engine.clone()).await? {
            TransactionResult::Nothing => {}
            TransactionResult::Mail(mail) => {
                on_mail(
                    conn,
                    mail,
                    &mut helo_domain,
                    working_sender.clone(),
                    delivery_sender.clone(),
                )
                .await?;
            }
            TransactionResult::TlsUpgrade if tls_config.is_none() => {
                conn.send_code(SMTPReplyCode::Code454)?;
                conn.send_code(SMTPReplyCode::Code221)?;
                return Ok(());
            }
            TransactionResult::TlsUpgrade => {
                return handle_connection_secured(
                    conn,
                    tls_config.clone(),
                    rule_engine,
                    working_sender.clone(),
                    delivery_sender.clone(),
                )
                .await;
            }
        }
    }

    Ok(())
}

pub(crate) async fn handle_connection_secured<S>(
    conn: &mut Connection<'_, S>,
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    rule_engine: Arc<RwLock<RuleEngine>>,
    working_sender: std::sync::Arc<tokio::sync::mpsc::Sender<ProcessMessage>>,
    delivery_sender: std::sync::Arc<tokio::sync::mpsc::Sender<ProcessMessage>>,
) -> anyhow::Result<()>
where
    S: std::io::Read + std::io::Write,
{
    let smtps_config = conn.config.smtps.as_ref().unwrap();

    let mut tls_conn = rustls::ServerConnection::new(tls_config.unwrap()).unwrap();
    let mut tls_stream = rustls::Stream::new(&mut tls_conn, &mut conn.io_stream);
    let mut io_tls_stream = IoService::new(&mut tls_stream);

    Connection::<IoService<'_, S>>::complete_tls_handshake(
        &mut io_tls_stream,
        &smtps_config.handshake_timeout,
    )
    .unwrap();

    let mut secured_conn = Connection {
        kind: conn.kind,
        timestamp: conn.timestamp,
        is_alive: true,
        config: conn.config.clone(),
        client_addr: conn.client_addr,
        error_count: conn.error_count,
        is_secured: true,
        io_stream: &mut io_tls_stream,
    };

    // FIXME: the rejection of the client because of the SSL/TLS protocol
    // version is done after the handshake...

    if !is_version_requirement_satisfied(secured_conn.io_stream.inner.conn, smtps_config) {
        log::error!("requirement not satisfied");
        // TODO: send error 500 ?
        return Ok(());
    }

    if let ConnectionKind::Tunneled = secured_conn.kind {
        secured_conn.send_code(SMTPReplyCode::Code220)?;
    }

    let mut helo_domain = None;

    while secured_conn.is_alive {
        match Transaction::receive(&mut secured_conn, &helo_domain, rule_engine.clone()).await? {
            TransactionResult::Nothing => {}
            TransactionResult::Mail(mail) => {
                on_mail(
                    &mut secured_conn,
                    mail,
                    &mut helo_domain,
                    working_sender.clone(),
                    delivery_sender.clone(),
                )
                .await?;
            }
            TransactionResult::TlsUpgrade => todo!(),
        }
    }
    Ok(())
}
