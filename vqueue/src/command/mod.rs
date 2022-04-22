use crate::{Args, Commands, MessageCommand};
use vsmtp_common::{
    queue::Queue,
    re::{
        anyhow::{self, Context},
        strum,
    },
};
use vsmtp_config::Config;

mod queue_show;
mod msg_command {
    pub mod r#move;
    pub mod remove;
    pub mod show;
}

fn get_message_path(
    id: &str,
    queues_dirpath: &std::path::Path,
) -> anyhow::Result<std::path::PathBuf> {
    for queue in <Queue as strum::IntoEnumIterator>::iter() {
        let queue_path = vsmtp_common::queue_path!(queues_dirpath, queue);
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

    anyhow::bail!(
        "No such message '{id}' in queues at '{}'",
        queues_dirpath.display()
    )
}

/// Execute the vQueue command
///
/// # Errors
pub fn execute(args: Args, config: &Config) -> anyhow::Result<()> {
    match args.command {
        Commands::Show {
            queues,
            empty_token,
        } => queue_show::queue_show(
            if queues.is_empty() {
                <Queue as strum::IntoEnumIterator>::iter().collect::<Vec<_>>()
            } else {
                queues
            },
            &config.server.queues.dirpath,
            empty_token,
            &mut std::io::stdout(),
        ),
        Commands::Msg { msg, command } => match command {
            MessageCommand::Show { format } => msg_command::show::show(
                &msg,
                &format,
                &config.server.queues.dirpath,
                &mut std::io::stdout(),
            ),
            MessageCommand::Move { queue } => {
                msg_command::r#move::r#move(&msg, queue, &config.server.queues.dirpath)
            }
            MessageCommand::Remove { yes } => msg_command::remove::remove(
                &msg,
                yes,
                &config.server.queues.dirpath,
                &mut std::io::stdout(),
                std::io::stdin().lock(),
            ),
            MessageCommand::ReRun {} => unimplemented!(),
        },
    }
}

#[cfg(test)]
mod tests {
    use vsmtp_common::queue_path;

    use super::*;

    #[test]
    fn find_one() {
        let queues_dirpath = "./tmp";

        let filepath =
            queue_path!(create_if_missing => queues_dirpath, Queue::Working, "toto").unwrap();

        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&filepath)
            .unwrap();

        assert_eq!(
            get_message_path("toto", &std::path::PathBuf::from(queues_dirpath)).unwrap(),
            filepath
        );

        std::fs::remove_file(filepath).unwrap();
    }

    #[test]
    fn not_found() {
        assert!(get_message_path("foobar", &std::path::PathBuf::from("./tmp")).is_err(),);
    }
}
