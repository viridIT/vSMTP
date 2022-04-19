use crate::{command::get_message_path, MessageShowFormat};
use vsmtp_common::{
    mail_context::MailContext,
    re::{
        anyhow::{self, Context},
        serde_json,
    },
};
use vsmtp_config::Config;

pub fn show(msg_id: &str, format: &MessageShowFormat, config: &Config) -> anyhow::Result<()> {
    let message = get_message_path(msg_id, &config.server.queues.dirpath).and_then(|path| {
        std::fs::read_to_string(&path).context(format!("Failed to read file: '{}'", path.display()))
    })?;

    let message: MailContext = serde_json::from_str(&message)?;

    match format {
        MessageShowFormat::Eml => println!("{}", message.body),
        MessageShowFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&message)?);
        }
    }

    Ok(())
}
