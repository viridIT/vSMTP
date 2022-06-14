use vsmtp_common::queue::Queue;

///
#[derive(clap::Parser)]
#[cfg_attr(test, derive(Debug, PartialEq))]
#[clap(about, version, author)]
pub struct Args {
    /// Path of the vSMTP configuration file (toml format)
    #[clap(short, long, action)]
    pub config: Option<String>,

    ///
    #[clap(subcommand)]
    pub command: Commands,
}

///
#[derive(clap::Subcommand)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub enum Commands {
    /// Show the content of the given queue(s)
    Show {
        /// List of queues to print
        #[clap(value_parser)]
        queues: Vec<Queue>,
        /// Character to print if the field is empty
        #[clap(short, long, action, default_value = "0")]
        empty_token: char,
    },
    /// Operate action to a given message
    Msg {
        /// ID of the concerned message
        #[clap(value_parser)]
        msg: String,
        ///
        #[clap(subcommand)]
        command: MessageCommand,
    },
}

///
#[derive(Clone, clap::Subcommand)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub enum MessageCommand {
    /// Print the content of the message
    Show {
        /// Format of the output
        #[clap(arg_enum, value_parser, default_value = "json")]
        format: MessageShowFormat,
    },
    /// Move the message to the given queue
    Move {
        ///
        #[clap(value_parser)]
        queue: Queue,
    },
    /// Remove the message from the filesystem
    Remove {
        /// If true, do not ask to confirm the deletion
        #[clap(short, long, value_parser)]
        yes: bool,
    },
    /// Re-introduce the message in the delivery system
    ReRun {},
}

///
#[derive(Clone, clap::ArgEnum)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub enum MessageShowFormat {
    /// Message's body as .eml (bytes between DATA and \r\n.\r\n)
    Eml,
    /// Complete mail context
    Json,
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn arg_show_queue() {
        assert_eq!(
            Args {
                config: None,
                command: Commands::Show {
                    queues: vec![],
                    empty_token: '0'
                }
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "show"]).unwrap()
        );

        assert_eq!(
            Args {
                config: None,
                command: Commands::Show {
                    queues: vec![Queue::Dead],
                    empty_token: '0'
                }
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "show", "dead"]).unwrap()
        );

        assert_eq!(
            Args {
                config: None,
                command: Commands::Show {
                    queues: vec![],
                    empty_token: '.'
                }
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "show", "-e", "."]).unwrap()
        );

        assert_eq!(
            Args {
                config: None,
                command: Commands::Show {
                    queues: vec![Queue::Dead, Queue::Deliver],
                    empty_token: '0'
                }
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "show", "dead", "deliver"]).unwrap()
        );
    }

    #[test]
    fn arg_show_message() {
        assert_eq!(
            Args {
                config: None,
                command: Commands::Msg {
                    msg: "foobar".to_string(),
                    command: MessageCommand::Show {
                        format: MessageShowFormat::Json
                    }
                }
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "msg", "foobar", "show"]).unwrap()
        );

        assert_eq!(
            Args {
                config: None,
                command: Commands::Msg {
                    msg: "foobar".to_string(),
                    command: MessageCommand::Show {
                        format: MessageShowFormat::Json
                    }
                }
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "msg", "foobar", "show", "json"])
                .unwrap()
        );

        assert_eq!(
            Args {
                config: None,
                command: Commands::Msg {
                    msg: "foobar".to_string(),
                    command: MessageCommand::Show {
                        format: MessageShowFormat::Eml
                    }
                }
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "msg", "foobar", "show", "eml"])
                .unwrap()
        );
    }

    #[test]
    fn arg_move_message() {
        assert_eq!(
            Args {
                config: None,
                command: Commands::Msg {
                    msg: "foobar".to_string(),
                    command: MessageCommand::Move { queue: Queue::Dead }
                }
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "msg", "foobar", "move", "dead"])
                .unwrap()
        );
    }

    #[test]
    fn arg_remove_message() {
        assert_eq!(
            Args {
                config: None,
                command: Commands::Msg {
                    msg: "foobar".to_string(),
                    command: MessageCommand::Remove { yes: false }
                }
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "msg", "foobar", "remove"]).unwrap()
        );

        assert_eq!(
            Args {
                config: None,
                command: Commands::Msg {
                    msg: "foobar".to_string(),
                    command: MessageCommand::Remove { yes: true }
                }
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "msg", "foobar", "remove", "--yes"])
                .unwrap()
        );
    }
}
