use crate::{Commands, MessageCommand};
use vsmtp_common::{
    queue::Queue,
    re::{anyhow, strum},
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
        let mut dir = match queue_path.read_dir() {
            Ok(dir) => dir,
            Err(_) => continue,
        };
        if let Some(found) = dir.find_map(|i| match i {
            Ok(i) if i.file_name() == id => Some(i.path()),
            // entry where process do not have permission, or other errors
            // in that case we ignore and continue searching the message
            _ => None,
        }) {
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
pub fn execute(command: Commands, config: &Config) -> anyhow::Result<()> {
    match command {
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
    use super::*;
    use crate::MessageShowFormat;
    use vsmtp_common::{
        addr,
        envelop::Envelop,
        mail_context::{ConnectionContext, MailContext, MessageMetadata},
        queue_path,
        rcpt::Rcpt,
        transfer::{EmailTransferStatus, Transfer},
    };

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
        assert!(get_message_path("foobar", &std::path::PathBuf::from("./tmp")).is_err());
    }

    fn get_mail(msg_id: &str) -> MailContext {
        MailContext {
            connection: ConnectionContext {
                timestamp: std::time::SystemTime::now(),
                credentials: None,
                is_authenticated: false,
                is_secured: false,
                server_name: "testserver.com".to_string(),
                server_address: "0.0.0.0:25".parse().unwrap(),
            },
            client_addr: "0.0.0.0:26".parse().unwrap(),
            envelop: Envelop {
                helo: "toto".to_string(),
                mail_from: addr!("foo@domain.com"),
                rcpt: vec![Rcpt {
                    address: addr!("foo+1@domain.com"),
                    transfer_method: Transfer::Mbox,
                    email_status: EmailTransferStatus::Waiting,
                }],
            },
            /*
            body: Some(MessageBody::Parsed(Box::new(Mail {
                headers: [
                    ("from", "foo2 foo <foo2@foo>"),
                    ("date", "tue, 30 nov 2021 20:54:27 +0100"),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Vec<_>>(),
                body: BodyType::Regular(vec!["Hello World!!".to_string()]),
            }))),
            */
            metadata: Some(MessageMetadata {
                timestamp: std::time::SystemTime::now(),
                message_id: msg_id.to_string(),
                skipped: None,
            }),
        }
    }

    #[test]

    fn execute_show_q_all() {
        let mut config = Config::default();
        config.server.queues.dirpath = "./tmp/execute_show_q_all".into();

        execute(
            Commands::Show {
                queues: vec![],
                empty_token: '.',
            },
            &config,
        )
        .unwrap();
    }

    #[test]

    fn execute_show_q_working() {
        let mut config = Config::default();
        config.server.queues.dirpath = "./tmp/execute_show_q_working".into();

        execute(
            Commands::Show {
                queues: vec![Queue::Working],
                empty_token: '.',
            },
            &config,
        )
        .unwrap();
    }

    #[test]

    fn execute_show() {
        let mut config = Config::default();
        config.server.queues.dirpath = "./tmp/execute_show".into();

        Queue::Dead
            .write_to_queue(
                &std::path::PathBuf::from("./tmp/execute_show"),
                &get_mail("foobar"),
            )
            .unwrap();

        execute(
            Commands::Msg {
                msg: "foobar".to_string(),
                command: MessageCommand::Show {
                    format: MessageShowFormat::Eml,
                },
            },
            &config,
        )
        .unwrap();
    }

    #[test]

    fn execute_move() {
        let mut config = Config::default();
        config.server.queues.dirpath = "./tmp/execute_move".into();

        queue_path!(create_if_missing => config.server.queues.dirpath.clone(), Queue::Dead)
            .unwrap();

        Queue::Working
            .write_to_queue(
                &std::path::PathBuf::from("./tmp/execute_move"),
                &get_mail("foobar"),
            )
            .unwrap();

        execute(
            Commands::Msg {
                msg: "foobar".to_string(),
                command: MessageCommand::Move { queue: Queue::Dead },
            },
            &config,
        )
        .unwrap();
    }

    #[test]

    fn execute_remove() {
        let mut config = Config::default();
        config.server.queues.dirpath = "./tmp/execute_remove".into();

        Queue::Working
            .write_to_queue(
                &std::path::PathBuf::from("./tmp/execute_remove"),
                &get_mail("foobar"),
            )
            .unwrap();

        execute(
            Commands::Msg {
                msg: "foobar".to_string(),
                command: MessageCommand::Remove { yes: true },
            },
            &config,
        )
        .unwrap();
    }
}
