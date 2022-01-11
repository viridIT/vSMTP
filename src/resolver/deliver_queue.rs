use crate::{
    config::{log_channel::RECEIVER, server_config::ServerConfig},
    smtp::code::SMTPReplyCode,
};

use super::DataEndResolver;

/// identifiers for all mail queues.
pub enum Queue {
    Deliver,
    Working,
    Deferred,
    Dead,
}

impl Queue {
    fn as_str(&self) -> &'static str {
        match self {
            Queue::Deliver => "deliver",
            Queue::Working => "working",
            Queue::Deferred => "deferred",
            Queue::Dead => "dead",
        }
    }

    pub fn to_path(
        &self,
        parent: impl Into<std::path::PathBuf>,
    ) -> std::io::Result<std::path::PathBuf> {
        let dir = parent.into().join(self.as_str().to_string() + "/");
        if !dir.exists() {
            std::fs::DirBuilder::new().recursive(true).create(&dir)?;
        }
        Ok(dir)
    }
}

/// used to write mail to the delivery queue and send a notification
/// to the delivery process.
pub struct DeliverQueueResolver {
    sender: crossbeam_channel::Sender<String>,
}

impl DeliverQueueResolver {
    pub fn new(sender: crossbeam_channel::Sender<String>) -> Self {
        Self { sender }
    }
}

#[async_trait::async_trait]
impl DataEndResolver for DeliverQueueResolver {
    async fn on_data_end(
        &mut self,
        server_config: &ServerConfig,
        ctx: &crate::model::mail::MailContext,
    ) -> Result<SMTPReplyCode, std::io::Error> {
        write_to_queue(Queue::Working, server_config, ctx)?;

        let message_id = ctx.metadata.as_ref().unwrap().message_id.clone();

        log::trace!(
            target: RECEIVER,
            "mail {} successfully written to deliver queue",
            message_id
        );

        // TODO: handle send errors.
        // sending the message id to the delivery process.
        // NOTE: we could send the context instead, so that the delivery system won't have
        //       to touch the file system.
        self.sender.send(message_id).unwrap();

        // TODO: use the right codes.
        Ok(SMTPReplyCode::Code250)
    }
}

/// write a mail as JSON to the given queue using it's message id.
fn write_to_queue(
    queue: Queue,
    server_config: &ServerConfig,
    ctx: &crate::model::mail::MailContext,
) -> std::io::Result<()> {
    let to_deliver = queue
        .to_path(&server_config.smtp.spool_dir)?
        .join(&ctx.metadata.as_ref().unwrap().message_id);

    // TODO: should loop if a file name is conflicting.
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&to_deliver)?;

    std::io::Write::write_all(&mut file, serde_json::to_string(ctx)?.as_bytes())
}
