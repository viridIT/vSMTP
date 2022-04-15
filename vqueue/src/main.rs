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

#[derive(Debug)]
struct QueueEntry {
    path: std::path::PathBuf,
    modified: std::time::SystemTime,
    message: MailContext,
}

fn queue_entries(
    queue: &Queue,
    queues_dirpath: &std::path::Path,
) -> anyhow::Result<Vec<QueueEntry>> {
    let queue_path = queue.to_path(queues_dirpath)?;

    queue_path
        .read_dir()
        .context(format!("Error from read dir '{}'", queue_path.display()))?
        // TODO: raise error ?
        .filter_map(|i| i.ok())
        .map(|i| {
            let metadata = i.metadata().unwrap();
            let modified = metadata.modified().unwrap();
            let message = std::fs::read_to_string(&i.path())
                .context(format!("Failed to read file: '{}'", i.path().display()))?;

            let message: MailContext = serde_json::from_str(&message)?;

            anyhow::Ok(QueueEntry {
                path: i.path(),
                modified,
                message,
            })
        })
        // TODO: ignore error ?
        .collect::<anyhow::Result<Vec<_>>>()
}

fn get_message_path(
    id: &str,
    queues_dirpath: &std::path::Path,
) -> anyhow::Result<std::path::PathBuf> {
    for queue in <Queue as vsmtp_common::re::strum::IntoEnumIterator>::iter() {
        match queue.to_path(queues_dirpath) {
            // TODO: raise error ?
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
            fn lifetimes() -> Vec<u32> {
                (1..10)
                    .into_iter()
                    .scan(5, |state, _| {
                        *state *= 2;
                        Some(*state)
                    })
                    .collect()
            }

            let now = std::time::SystemTime::now();

            for q in queues {
                println!("{}", q);
                for i in lifetimes() {
                    println!("{i}");
                }
                let mut entries = queue_entries(&q, &config.server.queues.dirpath)?;
                entries.sort_by(|a, b| Ord::cmp(&a.message.envelop.helo, &b.message.envelop.helo));

                let groups = itertools::Itertools::group_by(entries.into_iter(), |i| {
                    i.message.envelop.helo.clone()
                });

                for (key, values) in &groups {
                    let mut values = values.into_iter().collect::<Vec<_>>();
                    values.sort_by(|a, b| Ord::cmp(&a.modified, &b.modified));

                    // let mut maps = std::collections::HashMap::<u32, Vec<QueueEntry>>::new();
                    //
                    // todo: sort by timestamp and print
                    println!("{:?}", key);
                    for i in values {
                        println!("{:?}", i);
                    }
                }
            }
        }
        Commands::Msg { msg, command } => match command {
            MessageCommand::Show { format } => {
                let message =
                    get_message_path(&msg, &config.server.queues.dirpath).and_then(|path| {
                        std::fs::read_to_string(&path)
                            .context(format!("Failed to read file: '{}'", path.display()))
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
