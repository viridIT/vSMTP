use crate::{Connection, ProcessMessage};
use vsmtp_common::{
    mail_context::{MailContext, MessageBody},
    queue::Queue,
    re::{log, serde_json},
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
}

// TODO: refactor using thiserror & handle io error properly
#[async_trait::async_trait]
impl OnMail for MailHandler {
    async fn on_mail<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
        &mut self,
        conn: &mut Connection<S>,
        mail: Box<MailContext>,
        message: MessageBody,
    ) -> CodeID {
        let metadata = mail.metadata.as_ref().unwrap();

        if let Err(error) = Queue::write_to_mails(
            &conn.config.server.queues.dirpath,
            &metadata.message_id,
            &message,
        ) {
            log::error!("couldn't write to 'mails' queue: {error}");
            return CodeID::Denied;
        }

        let next_queue = match &metadata.skipped {
            Some(Status::Quarantine(path)) => {
                let mut path = match create_app_folder(&conn.config, Some(path)) {
                    Ok(path) => path,
                    Err(error) => {
                        log::error!("couldn't create app folder: {error}");
                        return CodeID::Denied;
                    }
                };
                path.push(format!("{}.json", metadata.message_id));

                let mut file = match tokio::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .await
                {
                    Ok(file) => file,
                    Err(error) => {
                        log::error!("couldn't open quarantine file: {error}");
                        return CodeID::Denied;
                    }
                };

                let serialized = match serde_json::to_string(mail.as_ref()) {
                    Ok(serialized) => serialized,
                    Err(error) => {
                        log::error!("couldn't serialize mail: {error}");
                        return CodeID::Denied;
                    }
                };

                if let Err(error) =
                    tokio::io::AsyncWriteExt::write_all(&mut file, serialized.as_bytes()).await
                {
                    log::error!("couldn't write to quarantine file: {error}");
                    return CodeID::Denied;
                }

                log::warn!("postq & delivery skipped due to quarantine.");
                return CodeID::Ok;
            }
            Some(reason) => {
                log::warn!("postq skipped due to '{}'.", reason.as_ref());
                Queue::Deliver
            }
            None => Queue::Working,
        };

        if let Err(error) = next_queue.write_to_queue(&conn.config.server.queues.dirpath, &mail) {
            log::error!("couldn't write to '{next_queue}' queue: {error}");
            return CodeID::Denied;
        }

        let sender_result = match next_queue {
            Queue::Working => &self.working_sender,
            Queue::Deliver => &self.delivery_sender,
            _ => unreachable!(),
        }
        .send(ProcessMessage {
            message_id: metadata.message_id.clone(),
        })
        .await;

        if let Err(error) = sender_result {
            log::error!("couldn't send message to next process '{next_queue}': {error}");
            return CodeID::Denied;
        }

        CodeID::Ok
    }
}
