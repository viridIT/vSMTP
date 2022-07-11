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

use crate::{delegate, log_channels, Connection, Process, ProcessMessage};
use vsmtp_common::{
    mail_context::{MailContext, MessageBody},
    queue::Queue,
    re::{anyhow, log, tokio},
    status::Status,
    transfer::EmailTransferStatus,
    CodeID,
};
use vsmtp_config::create_app_folder;

/// will be executed once the email is received.
#[async_trait::async_trait]
pub trait OnMail {
    /// the server executes this function once the email as been received.
    async fn on_mail<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
        &mut self,
        conn: &mut Connection<S>,
        mail: Box<MailContext>,
        message: MessageBody,
    ) -> CodeID;
}

/// Send the email to the queue.
pub struct MailHandler {
    pub(crate) working_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
    pub(crate) delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
}

#[derive(Debug, thiserror::Error)]
pub enum MailHandlerError {
    #[error("Could not delegate message: `{0}`")]
    DelegateMessage(anyhow::Error),
    #[error("couldn't write to `mails` folder: `{0}`")]
    WriteMessageBody(std::io::Error),
    #[error("couldn't create app folder: `{0}`")]
    CreateAppFolder(anyhow::Error),
    #[error("couldn't write to quarantine file: `{0}`")]
    WriteQuarantineFile(std::io::Error),
    #[error("couldn't write to queue `{0}` got: `{1}`")]
    WriteToQueue(Queue, std::io::Error),
    #[error("couldn't send message to next process `{0}` got: `{1}`")]
    SendToNextProcess(Process, tokio::sync::mpsc::error::SendError<ProcessMessage>),
}

impl MailHandler {
    /// create a new mail handler
    #[must_use]
    pub const fn new(
        working_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
        delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
    ) -> Self {
        Self {
            working_sender,
            delivery_sender,
        }
    }

    #[allow(clippy::too_many_lines)]
    async fn on_mail_priv<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
        &self,
        conn: &mut Connection<S>,
        mut mail_context: Box<MailContext>,
        mail_message: MessageBody,
    ) -> Result<(), MailHandlerError> {
        let (mut message_id, skipped) = {
            let metadata = mail_context.metadata.as_ref().unwrap();
            (metadata.message_id.clone(), metadata.skipped.clone())
        };

        let mut write_to_queue = Option::<Queue>::None;
        let mut send_to_next_process = Option::<Process>::None;
        let mut delegated = false;

        match &skipped {
            Some(Status::Quarantine(path)) => {
                let mut path = create_app_folder(&conn.config, Some(path))
                    .map_err(MailHandlerError::CreateAppFolder)?;

                path.push(format!("{}.json", message_id));

                Queue::write_to_quarantine(&path, &mail_context)
                    .await
                    .map_err(MailHandlerError::WriteQuarantineFile)?;

                log::warn!(target: log_channels::PREQ, "skipped due to quarantine.",);
            }
            Some(Status::Delegated(delegator)) => {
                mail_context.metadata.as_mut().unwrap().skipped = None;

                // FIXME: find a way to use `write_to_queue` instead to be consistant
                //        with the rest of the function.
                Queue::Delegated
                    .write_to_queue(&conn.config.server.queues.dirpath, &mail_context)
                    .map_err(|error| MailHandlerError::WriteToQueue(Queue::Working, error))?;

                // NOTE: needs to be executed after writing, because the other
                //       thread could pickup the email faster than this function.
                delegate(delegator, &mail_context, &mail_message)
                    .map_err(MailHandlerError::DelegateMessage)?;

                log::warn!(target: log_channels::PREQ, " skipped due to delegation.",);

                return Ok(());
            }
            Some(Status::DelegationResult) => {
                if let Some(old_message_id) = mail_message
                    .get_header("X-VSMTP-DELEGATION")
                    .and_then(|header| {
                        vsmtp_mail_parser::get_mime_header("X-VSMTP-DELEGATION", header)
                            .args
                            .get("id")
                            .cloned()
                    })
                {
                    message_id = old_message_id;
                }

                delegated = true;
                send_to_next_process = Some(Process::Processing);
            }
            Some(Status::Deny(code)) => {
                for rcpt in &mut mail_context.envelop.rcpt {
                    rcpt.email_status = EmailTransferStatus::Failed {
                        timestamp: std::time::SystemTime::now(),
                        reason: format!("rule engine denied the message in preq: {code:?}."),
                    };
                }

                write_to_queue = Some(Queue::Dead);
            }
            Some(reason) => {
                log::warn!(
                    target: log_channels::PREQ,
                    "skipped due to '{}'.",
                    reason.as_ref()
                );
                write_to_queue = Some(Queue::Deliver);
                send_to_next_process = Some(Process::Delivery);
            }
            None => {
                write_to_queue = Some(Queue::Working);
                send_to_next_process = Some(Process::Processing);
            }
        };

        Queue::write_to_mails(
            &conn.config.server.queues.dirpath,
            &message_id,
            &mail_message,
        )
        .map_err(MailHandlerError::WriteMessageBody)?;

        log::debug!(
            target: log_channels::PREQ,
            "(msg={}) email written in 'mails' queue.",
            message_id
        );

        if let Some(queue) = write_to_queue {
            queue
                .write_to_queue(&conn.config.server.queues.dirpath, &mail_context)
                .map_err(|error| MailHandlerError::WriteToQueue(queue, error))?;
        }

        // TODO: even if it's a rare case, a result of None should remove the
        //       email from the queue.
        match &send_to_next_process {
            Some(Process::Processing) => &self.working_sender,
            Some(Process::Delivery) => &self.delivery_sender,
            Some(Process::Receiver) | None => return Ok(()),
        }
        .send(ProcessMessage {
            message_id,
            delegated,
        })
        .await
        .map_err(|error| MailHandlerError::SendToNextProcess(send_to_next_process.unwrap(), error))
    }
}

#[async_trait::async_trait]
impl OnMail for MailHandler {
    async fn on_mail<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
        &mut self,
        conn: &mut Connection<S>,
        mail: Box<MailContext>,
        message: MessageBody,
    ) -> CodeID {
        match self.on_mail_priv(conn, mail, message).await {
            Ok(_) => CodeID::Ok,
            Err(error) => {
                log::warn!(
                    target: log_channels::PREQ,
                    "[{}] failed to process mail: {error}",
                    conn.server_addr
                );
                CodeID::Denied
            }
        }
    }
}
