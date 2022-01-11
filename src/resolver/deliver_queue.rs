use crate::{
    config::{log_channel::RECEIVER, server_config::ServerConfig},
    smtp::code::SMTPReplyCode,
};

use super::DataEndResolver;

/// identifiers for all mail queues.
#[allow(unused)]
enum Queue {
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
}

/// used to write mail to the delivery queue and send a notification
/// to the delivery process.
pub struct DeliverQueueResolver {
    deliver_proc: crossbeam_channel::Sender<String>,
}

impl DeliverQueueResolver {
    pub fn new(sender: crossbeam_channel::Sender<String>) -> Self {
        Self {
            deliver_proc: sender,
        }
    }
}

#[async_trait::async_trait]
impl DataEndResolver for DeliverQueueResolver {
    async fn on_data_end(
        &mut self,
        server_config: &ServerConfig,
        ctx: &crate::model::mail::MailContext,
    ) -> Result<SMTPReplyCode, std::io::Error> {
        write_to_queue(Queue::Deliver, server_config, ctx)?;

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
        self.deliver_proc.send(message_id).unwrap();

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
    let message_id = ctx.metadata.as_ref().unwrap().message_id.clone();

    let to_deliver = <std::path::PathBuf as std::str::FromStr>::from_str(&format!(
        "{}/{}/{}",
        server_config.smtp.spool_dir,
        queue.as_str(),
        &message_id
    ))
    // infallible.
    .unwrap();

    // TODO: should loop if a file name is conflicting.
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&to_deliver)?;

    std::io::Write::write_all(&mut file, serde_json::to_string(ctx)?.as_bytes())
}
