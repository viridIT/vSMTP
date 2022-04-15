use vqueue::{Args, Commands, MessageCommand};
use vsmtp_common::{
    queue::Queue,
    re::anyhow::{self, Context},
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
    println!("vqueue");

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
        Commands::Show { queues } => todo!(),
        Commands::Msg { msg, command } => match command {
            MessageCommand::Show { format } => {
                let message =
                    get_message_path(&msg, &config.server.queues.dirpath).and_then(|path| {
                        std::fs::read_to_string(&path)
                            .context(format!("Failed to read file: '{:?}'", path))
                    })?;
                println!("{:?}", message);
                Ok(())
            }
            MessageCommand::Move { .. } => todo!(),
            MessageCommand::Remove {} => todo!(),
            MessageCommand::ReRun {} => todo!(),
        },
    }
}
