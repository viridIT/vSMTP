use vsmtp_common::{queue::Queue, re::anyhow};
use vsmtp_config::Config;

use crate::command::get_message_path;

pub fn r#move(msg_id: &str, queue: Queue, config: &Config) -> anyhow::Result<()> {
    let message = get_message_path(msg_id, &config.server.queues.dirpath)?;

    std::fs::rename(
        &message,
        queue.to_path(config.server.queues.dirpath.clone())?.join(
            message
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Not a valid filename: '{}'", message.display()))?,
        ),
    )?;

    Ok(())
}
