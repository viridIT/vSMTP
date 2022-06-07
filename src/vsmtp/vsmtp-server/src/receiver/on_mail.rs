use crate::{Connection, ProcessMessage};
use vsmtp_common::{
    mail_context::{MailContext, MessageBody},
    queue::Queue,
    re::{anyhow, log, tokio},
    status::Status,
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
    #[error("couldn't write to `mails` folder: `{0}`")]
    WriteMessageBody(std::io::Error),
    #[error("couldn't create app folder: `{0}`")]
    CreateAppFolder(anyhow::Error),
    #[error("couldn't write to quarantine file: `{0}`")]
    WriteQuarantineFile(std::io::Error),
    #[error("couldn't write to queue `{0}` got: `{1}`")]
    WriteToQueue(Queue, std::io::Error),
    #[error("couldn't send message to next process `{0}` got: `{1}`")]
    SendToNextProcess(Queue, tokio::sync::mpsc::error::SendError<ProcessMessage>),
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

    async fn on_mail_priv<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
        &self,
        conn: &mut Connection<S>,
        mail: Box<MailContext>,
        message: MessageBody,
    ) -> Result<(), MailHandlerError> {
        let metadata = mail.metadata.as_ref().unwrap();

        Queue::write_to_mails(
            &conn.config.server.queues.dirpath,
            &metadata.message_id,
            &message,
        )
        .map_err(MailHandlerError::WriteMessageBody)?;

        let next_queue = match &metadata.skipped {
            Some(Status::Quarantine(path)) => {
                let mut path = create_app_folder(&conn.config, Some(path))
                    .map_err(MailHandlerError::CreateAppFolder)?;

                path.push(format!("{}.json", metadata.message_id));

                Queue::write_to_quarantine(&path, &mail)
                    .await
                    .map_err(MailHandlerError::WriteQuarantineFile)?;

                log::warn!("postq & delivery skipped due to quarantine.");
                return Ok(());
            }
            Some(reason) => {
                log::warn!("postq skipped due to '{}'.", reason.as_ref());
                Queue::Deliver
            }
            None => Queue::Working,
        };

        next_queue
            .write_to_queue(&conn.config.server.queues.dirpath, &mail)
            .map_err(|error| MailHandlerError::WriteToQueue(next_queue, error))?;

        match next_queue {
            Queue::Working => &self.working_sender,
            Queue::Deliver => &self.delivery_sender,
            _ => unreachable!(),
        }
        .send(ProcessMessage {
            message_id: metadata.message_id.clone(),
        })
        .await
        .map_err(|error| MailHandlerError::SendToNextProcess(next_queue, error))?;

        Ok(())
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
                log::warn!("failed to process mail: {error}");
                CodeID::Denied
            }
        }
    }
}
