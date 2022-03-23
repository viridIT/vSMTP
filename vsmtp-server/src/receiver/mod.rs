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
use self::transaction::{Transaction, TransactionResult};
use crate::{processes::ProcessMessage, queue::Queue, server::SaslBackend};
use vsmtp_common::{auth::Mechanism, code::SMTPReplyCode, mail_context::MailContext, re::rsasl};
use vsmtp_rule_engine::rule_engine::RuleEngine;

mod connection;
mod io_service;
pub(crate) mod transaction;

pub use connection::{Connection, ConnectionKind};
pub use io_service::IoService;

#[cfg(test)]
mod tests;

// NOTE: not marked as #[cfg(test)] because it is used by the bench/fuzz
/// boilerplate for the tests
pub mod test_helpers;

async fn on_mail<S: std::io::Read + std::io::Write + Send>(
    conn: &mut Connection<'_, S>,
    mail: Box<MailContext>,
    helo_domain: &mut Option<String>,
    working_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
    delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
) -> anyhow::Result<()> {
    *helo_domain = Some(mail.envelop.helo.clone());

    match &mail.metadata {
        // quietly skipping mime & delivery processes when there is no resolver.
        // (in case of a quarantine for example)
        Some(metadata) if metadata.resolver == "none" => {
            log::warn!("delivery skipped due to NO_DELIVERY action call.");
            conn.send_code(SMTPReplyCode::Code250)?;
            Ok(())
        }
        Some(metadata) if metadata.skipped.is_some() => {
            log::warn!("postq skipped due to {:?}.", metadata.skipped.unwrap());
            match Queue::Deliver.write_to_queue(&conn.config, &mail) {
                Ok(_) => {
                    delivery_sender
                        .send(ProcessMessage {
                            message_id: metadata.message_id.clone(),
                        })
                        .await?;

                    conn.send_code(SMTPReplyCode::Code250)?;
                }
                Err(error) => {
                    log::error!("couldn't write to delivery queue: {}", error);
                    conn.send_code(SMTPReplyCode::Code554)?;
                }
            };
            Ok(())
        }
        Some(metadata) => {
            match Queue::Working.write_to_queue(&conn.config, &mail) {
                Ok(_) => {
                    working_sender
                        .send(ProcessMessage {
                            message_id: metadata.message_id.clone(),
                        })
                        .await?;

                    conn.send_code(SMTPReplyCode::Code250)?;
                }
                Err(error) => {
                    log::error!("couldn't write to queue: {}", error);
                    conn.send_code(SMTPReplyCode::Code554)?;
                }
            };
            Ok(())
        }
        _ => unreachable!(),
    }
}

fn auth_step<S>(
    conn: &mut Connection<'_, S>,
    session: &mut rsasl::DiscardOnDrop<rsasl::Session<()>>,
    buffer: String,
) -> anyhow::Result<bool>
where
    S: std::io::Read + std::io::Write + Send,
{
    let bytes64decoded = base64::decode(buffer).unwrap(); // 501 5.5.2

    match session.step(&bytes64decoded) {
        Ok(rsasl::Step::Done(buffer)) => {
            println!(
                "Authentication successful, bytes to return to client: {:?}",
                std::str::from_utf8(&*buffer)
            );
            // TODO: send buffer ?
            conn.send_code(SMTPReplyCode::AuthenticationSucceeded)?;
            anyhow::Ok(true)
        }
        Ok(rsasl::Step::NeedsMore(buffer)) => {
            let reply = format!(
                "334 {}\r\n",
                base64::encode(std::str::from_utf8(&*buffer).unwrap())
            ); // 501 5.5.2
            conn.send(&reply)?;
            Ok(false)
        }
        Err(e) if e.matches(rsasl::ReturnCode::GSASL_AUTHENTICATION_ERROR) => {
            panic!("Authentication failed, bad username or password");
        }
        Err(e) => panic!("Authentication errored: {}", e),
    }
}

async fn on_authentication<S>(
    conn: &mut Connection<'_, S>,
    rsasl: std::sync::Arc<tokio::sync::Mutex<SaslBackend>>,
    mechanism: Mechanism,
    initial_response: Option<String>,
) -> anyhow::Result<()>
where
    S: std::io::Read + std::io::Write + Send,
{
    // TODO: if mechanism require initial data , but omitted => error
    // TODO: if initial data == "=" ; it mean empty ""

    let mut guard = rsasl.lock().await;
    let mut session = guard.server_start(&String::from(mechanism)).unwrap();

    let mut authenticated = auth_step(
        conn,
        &mut session,
        initial_response.unwrap_or_else(|| "".to_string()),
    )
    .unwrap();
    while !authenticated {
        authenticated = match conn.read(std::time::Duration::from_secs(1)).await {
            Ok(Ok(buffer)) => auth_step(conn, &mut session, buffer).unwrap(),
            Ok(Err(e)) => todo!("error {e:?}"),
            Err(e) => todo!("timeout {e}"),
        };
    }

    // TODO: get session property
    Ok(())
}

/// Receives the incomings mail of a connection
///
/// # Errors
///
/// * server failed to send a message
/// * a transaction failed
/// * the pre-queue processing of the mail failed
#[allow(clippy::missing_panics_doc)]
pub async fn handle_connection<S>(
    conn: &mut Connection<'_, S>,
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    rsasl: Option<std::sync::Arc<tokio::sync::Mutex<SaslBackend>>>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    working_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
    delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
) -> anyhow::Result<()>
where
    S: std::io::Read + std::io::Write + Send,
{
    let mut helo_domain = None;

    conn.send_code(SMTPReplyCode::Greetings)?;

    while conn.is_alive {
        println!("helo_domain: {:?}", helo_domain);
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
                    rsasl,
                    rule_engine,
                    working_sender,
                    delivery_sender,
                )
                .await;
            }
            TransactionResult::Authentication(_, _, _) if rsasl.as_ref().is_none() => {
                todo!();
            }
            TransactionResult::Authentication(helo_pre_auth, mechanism, initial_response) => {
                on_authentication(
                    conn,
                    rsasl.as_ref().unwrap().clone(),
                    mechanism,
                    initial_response,
                )
                .await?;
                conn.is_authenticated = true;
                // TODO:  When a security layer takes effect
                // helo_domain = None;

                helo_domain = Some(helo_pre_auth);
            }
        }
    }

    Ok(())
}

pub(crate) async fn handle_connection_secured<S>(
    conn: &mut Connection<'_, S>,
    // TODO: should not be an option at this point
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    rsasl: Option<std::sync::Arc<tokio::sync::Mutex<SaslBackend>>>,
    rule_engine: std::sync::Arc<std::sync::RwLock<RuleEngine>>,
    working_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
    delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
) -> anyhow::Result<()>
where
    S: std::io::Read + std::io::Write + Send,
{
    let smtps_config = conn.config.server.tls.as_ref().ok_or_else(|| {
        anyhow::anyhow!("server accepted tls encrypted transaction, but not tls config provided")
    })?;

    let mut tls_conn = rustls::ServerConnection::new(tls_config.unwrap()).unwrap();
    let mut tls_stream = rustls::Stream::new(&mut tls_conn, &mut conn.io_stream);
    let mut io_tls_stream = IoService::new(&mut tls_stream);

    Connection::<IoService<'_, S>>::complete_tls_handshake(
        &mut io_tls_stream,
        &smtps_config.handshake_timeout,
    )?;

    let mut secured_conn = Connection {
        kind: conn.kind,
        timestamp: conn.timestamp,
        config: conn.config.clone(),
        client_addr: conn.client_addr,
        error_count: conn.error_count,
        is_authenticated: conn.is_authenticated,
        is_alive: true,
        is_secured: true,
        io_stream: &mut io_tls_stream,
    };

    if let ConnectionKind::Tunneled = secured_conn.kind {
        secured_conn.send_code(SMTPReplyCode::Greetings)?;
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
            TransactionResult::Authentication(_, _, _) if rsasl.as_ref().is_none() => {
                todo!();
            }
            TransactionResult::Authentication(helo_pre_auth, mechanism, initial_response) => {
                on_authentication(
                    &mut secured_conn,
                    rsasl.as_ref().unwrap().clone(),
                    mechanism,
                    initial_response,
                )
                .await?;
                secured_conn.is_authenticated = true;
                // TODO:  When a security layer takes effect
                // helo_domain = None;

                helo_domain = Some(helo_pre_auth);
            }
            TransactionResult::TlsUpgrade => todo!(),
        }
    }
    Ok(())
}
