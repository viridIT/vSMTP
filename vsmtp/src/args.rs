/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/
///
#[derive(Debug, clap::Parser, PartialEq)]
#[clap(about, version, author)]
pub struct Args {
    /// Path of the vSMTP configuration file (toml format)
    #[clap(short, long)]
    pub config: Option<String>,

    /// Commands
    #[clap(subcommand)]
    pub command: Option<Commands>,

    /// Do not run the program as a daemon
    #[clap(short, long)]
    pub no_daemon: bool,
}

///
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
        assert!(<Args as clap::StructOpt>::try_parse_from(&[""]).is_ok());

        assert_eq!(
            Args {
                command: None,
                config: Some("path".to_string()),
                no_daemon: false
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "-c", "path"]).unwrap()
        );

        assert_eq!(
            Args {
                command: Some(Commands::ConfigShow),
                config: Some("path".to_string()),
                no_daemon: false
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "-c", "path", "config-show"]).unwrap()
        );

        assert_eq!(
            Args {
                command: Some(Commands::ConfigDiff),
                config: Some("path".to_string()),
                no_daemon: false
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "-c", "path", "config-diff"]).unwrap()
        );

        assert_eq!(
            Args {
                command: None,
                config: Some("path".to_string()),
                no_daemon: true
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "-c", "path", "--no-daemon"]).unwrap()
        );
    }
}
