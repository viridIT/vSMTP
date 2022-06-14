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

use vsmtp_common::re::anyhow;
use vsmtp_config::re::humantime;

///
#[derive(Debug, Clone, PartialEq)]
pub struct Timeout(pub std::time::Duration);

impl std::str::FromStr for Timeout {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(
            humantime::parse_duration(s).map_err(anyhow::Error::new)?,
        ))
    }
}

///
#[derive(Debug, clap::Parser, PartialEq)]
#[clap(about, version, author)]
pub struct Args {
    /// Path of the vSMTP configuration file (toml format)
    #[clap(short, long, action)]
    pub config: Option<String>,

    /// Commands
    #[clap(subcommand)]
    pub command: Option<Commands>,

    /// Do not run the program as a daemon
    #[clap(short, long, action)]
    pub no_daemon: bool,

    /// Make the server stop after a delay (human readable format)
    #[clap(short, long, action)]
    pub timeout: Option<Timeout>,
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
                no_daemon: false,
                timeout: None
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "-c", "path"]).unwrap()
        );

        assert_eq!(
            Args {
                command: Some(Commands::ConfigShow),
                config: Some("path".to_string()),
                no_daemon: false,
                timeout: None
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "-c", "path", "config-show"]).unwrap()
        );

        assert_eq!(
            Args {
                command: Some(Commands::ConfigDiff),
                config: Some("path".to_string()),
                no_daemon: false,
                timeout: None
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "-c", "path", "config-diff"]).unwrap()
        );

        assert_eq!(
            Args {
                command: None,
                config: Some("path".to_string()),
                no_daemon: true,
                timeout: None
            },
            <Args as clap::StructOpt>::try_parse_from(&["", "-c", "path", "--no-daemon"]).unwrap()
        );

        assert_eq!(
            Args {
                command: None,
                config: Some("path".to_string()),
                no_daemon: true,
                timeout: Some(Timeout(std::time::Duration::from_secs(1)))
            },
            <Args as clap::StructOpt>::try_parse_from(&[
                "",
                "-c",
                "path",
                "--no-daemon",
                "--timeout",
                "1s"
            ])
            .unwrap()
        );
    }
}
