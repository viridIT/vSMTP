use crate::{Connection, ProcessMessage};
use vsmtp_common::{
    mail_context::MailContext,
    queue::Queue,
    re::{anyhow, log},
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
    ) -> anyhow::Result<CodeID>;
}

/// default mail handler for production.
pub struct MailHandler {
    /// message pipe to the working process.
    pub working_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
    /// message pipe to the delivery process.
    pub delivery_sender: tokio::sync::mpsc::Sender<ProcessMessage>,
}

#[async_trait::async_trait]
impl OnMail for MailHandler {
    async fn on_mail<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin>(
        &mut self,
        conn: &mut Connection<S>,
        mail: Box<MailContext>,
    ) -> anyhow::Result<CodeID> {
        let metadata = mail.metadata.as_ref().unwrap();

        let next_queue = match &metadata.skipped {
            Some(Status::Quarantine(path)) => {
                let mut path = create_app_folder(&conn.config, Some(path))?;
                path.push(format!("{}.json", metadata.message_id));

                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)?;

                std::io::Write::write_all(
                    &mut file,
                    vsmtp_common::re::serde_json::to_string_pretty(&*mail)?.as_bytes(),
                )?;

                log::warn!("postq & delivery skipped due to quarantine.");
                return Ok(CodeID::Ok);
            }
            Some(reason) => {
                log::warn!("postq skipped due to '{}'.", reason.as_ref());
                Queue::Deliver
            }
            None => Queue::Working,
        };

        if let Err(error) = next_queue.write_to_queue(&conn.config.server.queues.dirpath, &mail) {
            log::error!("couldn't write to '{}' queue: {}", next_queue, error);
            Ok(CodeID::Denied)
        } else {
            match next_queue {
                Queue::Working => &self.working_sender,
                Queue::Deliver => &self.delivery_sender,
                _ => unreachable!(),
            }
            .send(ProcessMessage {
                message_id: metadata.message_id.clone(),
            })
            .await?;

            Ok(CodeID::Ok)
        }
    }
}
