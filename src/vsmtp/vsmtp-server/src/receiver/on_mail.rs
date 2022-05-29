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

#[async_trait::async_trait]
impl OnMail for MailHandler {
    async fn on_mail<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
        &mut self,
        conn: &mut Connection<S>,
        mail: Box<MailContext>,
        message: MessageBody,
    ) -> CodeID {
        let metadata = mail.metadata.as_ref().unwrap();

        let next_queue = match &metadata.skipped {
            Some(Status::Quarantine(path)) => {
                let mut path = create_app_folder(&conn.config, Some(path)).unwrap();
                path.push(format!("{}.json", metadata.message_id));

                let mut file = tokio::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .await
                    .unwrap();

                tokio::io::AsyncWriteExt::write_all(
                    &mut file,
                    serde_json::to_string(mail.as_ref()).unwrap().as_bytes(),
                )
                .await
                .unwrap();

                Queue::write_to_mails(
                    &conn.config.server.queues.dirpath,
                    &metadata.message_id,
                    &message,
                )
                .unwrap();

                log::warn!("postq & delivery skipped due to quarantine.");
                return CodeID::Ok;
            }
            Some(reason) => {
                log::warn!("postq skipped due to '{}'.", reason.as_ref());
                Queue::Deliver
            }
            None => Queue::Working,
        };

        match next_queue.write_to_queue(&conn.config.server.queues.dirpath, &mail) {
            Ok(_) => {}
            Err(error) => {
                // TODO: handle io error properly

                log::error!("couldn't write to '{next_queue}' queue: {error}");
                return CodeID::Denied;
            }
        }

        match next_queue {
            Queue::Working => &self.working_sender,
            Queue::Deliver => &self.delivery_sender,
            _ => unreachable!(),
        }
        .send(ProcessMessage {
            message_id: metadata.message_id.clone(),
        })
        .await
        .unwrap();

        CodeID::Ok
    }
}
