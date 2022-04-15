use vqueue::{Args, Commands, MessageCommand, MessageShowFormat};
use vsmtp_common::{
    mail_context::MailContext,
    queue::Queue,
    re::{
        anyhow::{self, Context},
        serde_json,
        strum::IntoEnumIterator,
    },
};
use vsmtp_config::Config;

fn get_message_path(
    id: &str,
    queues_dirpath: &std::path::Path,
) -> anyhow::Result<std::path::PathBuf> {
    for queue in <Queue as vsmtp_common::re::strum::IntoEnumIterator>::iter() {
        match queue.to_path(queues_dirpath) {
            Err(_) => continue,
            Ok(queue_path) => {
                if let Some(found) = queue_path
                    .read_dir()
                    .context(format!("Error from read dir '{}'", queue_path.display()))?
                    .find_map(|i| match i {
                        Ok(i) if i.file_name() == id => Some(i.path()),
                        // entry where process do not have permission, or other errors
                        // in that case we ignore and continue searching the message
                        _ => None,
                    })
                {
                    return Ok(found);
                }
            }
        }
    }
    anyhow::bail!(
        "No such message '{id}' in queues at '{}'",
        queues_dirpath.display()
    )
}

fn main() -> anyhow::Result<()> {
    let args = <Args as clap::StructOpt>::parse();

    let config = args.config.as_ref().map_or_else(
        || Ok(Config::default()),
        |config| {
            std::fs::read_to_string(&config)
                .context(format!("Cannot read file '{}'", config))
                .and_then(|f| Config::from_toml(&f).context("File contains format error"))
                .context("Cannot parse the configuration")
        },
    )?;

    match args.command {
        Commands::Show { mut queues } => {
            if queues.is_empty() {
                queues = Queue::iter().collect::<Vec<_>>();
            }
            for i in queues {
                println!("{}", i);
            }
        }
        Commands::Msg { msg, command } => match command {
            MessageCommand::Show { format } => {
                let message =
                    get_message_path(&msg, &config.server.queues.dirpath).and_then(|path| {
                        std::fs::read_to_string(&path)
                            .context(format!("Failed to read file: '{:?}'", path))
                    })?;

                let message: MailContext = serde_json::from_str(&message)?;

                match format {
                    MessageShowFormat::Eml => println!("{}", message.body),
                    MessageShowFormat::Json => {
                        println!("{}", serde_json::to_string_pretty(&message)?)
                    }
                }
            }
            MessageCommand::Move { queue } => {
                let message = get_message_path(&msg, &config.server.queues.dirpath)?;
                std::fs::rename(
                    &message,
                    queue
                        .to_path(config.server.queues.dirpath)?
                        .join(message.file_name().unwrap()),
                )?;
            }
            MessageCommand::Remove { yes } => {
                let message = get_message_path(&msg, &config.server.queues.dirpath)?;
                println!("Removing file at location: '{}'", message.display());

                if !yes {
                    print!("Confirm ? [y|yes] ");
                    std::io::Write::flush(&mut std::io::stdout())?;

                    let confirmation =
                        std::io::BufRead::lines(std::io::stdin().lock())
                            .next()
                            .ok_or_else(|| anyhow::anyhow!("Fail to read line from stdio"))??;
                    if !["y", "yes"].contains(&confirmation.to_lowercase().as_str()) {
                        println!("Canceled");
                        return Ok(());
                    }
                }

                std::fs::remove_file(&message)?;
                println!("File removed");
            }
            MessageCommand::ReRun {} => todo!(),
        },
    }
    Ok(())
}
