use vsmtp_common::re::anyhow;
use vsmtp_config::Config;

use crate::command::get_message_path;

pub fn remove(msg_id: &str, ask_confirm: bool, config: &Config) -> anyhow::Result<()> {
    let message = get_message_path(msg_id, &config.server.queues.dirpath)?;
    println!("Removing file at location: '{}'", message.display());

    if !ask_confirm {
        print!("Confirm ? [y|yes] ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let confirmation = std::io::BufRead::lines(std::io::stdin().lock())
            .next()
            .ok_or_else(|| anyhow::anyhow!("Fail to read line from stdio"))??;
        if !["y", "yes"].contains(&confirmation.to_lowercase().as_str()) {
            println!("Canceled");
            return Ok(());
        }
    }

    std::fs::remove_file(&message)?;
    println!("File removed");
    Ok(())
}
