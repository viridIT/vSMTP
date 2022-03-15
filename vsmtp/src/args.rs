/// Flags and command to change vSMTP execution
#[derive(Debug, clap::Parser, PartialEq)]
#[clap(about, version, author)]
pub struct Args {
    /// Path of the vSMTP configuration file (toml format)
    #[clap(short, long)]
    pub config: String,

    /// Commands
    #[clap(subcommand)]
    pub command: Option<Commands>,

    /// Do not run the program as a daemon
    #[clap(short, long)]
    pub no_daemon: bool,
}

/// Subcommand run instead of the vSMTP server
#[derive(Debug, clap::Subcommand, PartialEq)]
pub enum Commands {
    /// Show the loaded config (as serialized json format)
    ConfigShow,
    /// Show the difference between the loaded config and the default one
    ConfigDiff,
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn parse_arg() {
        assert!(<Args as clap::StructOpt>::try_parse_from(&[""]).is_err());

        assert_eq!(
            Args {
                command: None,
                config: "path".to_string(),
                no_daemon: false
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "-c", "path"]).unwrap()
        );

        assert_eq!(
            Args {
                command: Some(Commands::ConfigShow),
                config: "path".to_string(),
                no_daemon: false
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "-c", "path", "config-show"]).unwrap()
        );

        assert_eq!(
            Args {
                command: Some(Commands::ConfigDiff),
                config: "path".to_string(),
                no_daemon: false
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "-c", "path", "config-diff"]).unwrap()
        );

        assert_eq!(
            Args {
                command: None,
                config: "path".to_string(),
                no_daemon: true
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "-c", "path", "--no-daemon"]).unwrap()
        );
    }
}
