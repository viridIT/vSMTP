/**
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
 **/
use anyhow::Context;
use vsmtp::{MBoxResolver, MailDirResolver, SMTPResolver};
use vsmtp_common::libc_abstraction::{daemon, setgid, setuid, ForkResult};
use vsmtp_config::get_logger_config;
use vsmtp_config::ServerConfig;
use vsmtp_server::server::ServerVSMTP;

#[derive(Debug, clap::Parser, PartialEq)]
#[clap(about, version, author)]
struct Args {
    /// Path of the vSMTP configuration file (toml format)
    #[clap(short, long)]
    config: String,

    #[clap(subcommand)]
    command: Option<Commands>,

    /// Do not run the program as a daemon
    #[clap(short, long)]
    no_daemon: bool,
}

#[derive(Debug, clap::Subcommand, PartialEq)]
enum Commands {
    /// Show the loaded config (as serialized json format)
    ConfigShow,
    /// Show the difference between the loaded config and the default one
    ConfigDiff,
}

mod tests {

    #[test]
    fn parse_arg() {
        assert!(<crate::Args as clap::StructOpt>::try_parse_from(&[""]).is_err());

        assert_eq!(
            crate::Args {
                command: None,
                config: "path".to_string(),
                no_daemon: false
            },
            <crate::Args as clap::StructOpt>::try_parse_from(&["", "-c", "path"]).unwrap()
        );

        assert_eq!(
            crate::Args {
                command: Some(crate::Commands::ConfigShow),
                config: "path".to_string(),
                no_daemon: false
            },
            <crate::Args as clap::StructOpt>::try_parse_from(&["", "-c", "path", "config-show"])
                .unwrap()
        );

        assert_eq!(
            crate::Args {
                command: Some(crate::Commands::ConfigDiff),
                config: "path".to_string(),
                no_daemon: false
            },
            <crate::Args as clap::StructOpt>::try_parse_from(&["", "-c", "path", "config-diff"])
                .unwrap()
        );

        assert_eq!(
            crate::Args {
                command: None,
                config: "path".to_string(),
                no_daemon: true
            },
            <crate::Args as clap::StructOpt>::try_parse_from(&["", "-c", "path", "--no-daemon"])
                .unwrap()
        );
    }
}

fn socket_bind_anyhow<A: std::net::ToSocketAddrs + std::fmt::Debug>(
    addr: A,
) -> anyhow::Result<std::net::TcpListener> {
    anyhow::Context::with_context(std::net::TcpListener::bind(&addr), || {
        format!("Failed to bind socket on addr: '{:?}'", addr)
    })
}

fn main() -> anyhow::Result<()> {
    let args = <Args as clap::StructOpt>::parse();

    let config = std::fs::read_to_string(&args.config)
        .with_context(|| format!("Cannot read file '{}'", args.config))
        .and_then(|data| {
            ServerConfig::from_toml(&data).with_context(|| "File contains format error")
        })
        .with_context(|| "Cannot parse the configuration")?;

    if let Some(command) = args.command {
        match command {
            Commands::ConfigShow => {
                let stringified = serde_json::to_string_pretty(&config)?;
                println!("Loaded configuration: {}", stringified);
                return Ok(());
            }
            Commands::ConfigDiff => {
                let loaded_config = serde_json::to_string_pretty(&config)?;
                let default_config = serde_json::to_string_pretty(
                    &ServerConfig::builder()
                        .with_version_str("<1.0.0")
                        .unwrap()
                        .with_rfc_port(&config.server.domain, "root", "root", None)
                        .without_log()
                        .without_smtps()
                        .with_default_smtp()
                        .with_delivery("/var/spool/vsmtp", "smtp")
                        .with_empty_rules()
                        .with_default_reply_codes()
                        .build()
                        .unwrap(),
                )?;
                for diff in diff::lines(&default_config, &loaded_config) {
                    match diff {
                        diff::Result::Left(left) => println!("-\x1b[0;31m{left}\x1b[0m"),
                        diff::Result::Both(same, _) => println!(" {same}"),
                        diff::Result::Right(right) => println!("+\x1b[0;32m{right}\x1b[0m"),
                    }
                }
                return Ok(());
            }
        }
    }

    get_logger_config(&config, args.no_daemon)
        .context("Logs configuration contain error")
        .map(log4rs::init_config)
        .context("Cannot initialize logs")??;

    let sockets = (
        socket_bind_anyhow(&config.server.addr[..])?,
        socket_bind_anyhow(&config.server.addr_submission[..])?,
        socket_bind_anyhow(&config.server.addr_submissions[..])?,
    );

    if args.no_daemon {
        start_runtime(config, sockets)
    } else {
        match daemon()? {
            ForkResult::Child => {
                setgid(
                    users::get_group_by_name(&config.server.vsmtp_group)
                        .unwrap()
                        .gid(),
                )?;
                setuid(
                    users::get_user_by_name(&config.server.vsmtp_user)
                        .unwrap()
                        .uid(),
                )?;
                start_runtime(config, sockets)
            }
            ForkResult::Parent(pid) => {
                log::info!("vSMTP running in process id={pid}");
                Ok(())
            }
        }
    }
}

fn start_runtime(
    config: ServerConfig,
    sockets: (
        std::net::TcpListener,
        std::net::TcpListener,
        std::net::TcpListener,
    ),
) -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.server.thread_count)
        .enable_all()
        // .on_thread_start(|| { println!("thread started"); })
        // .on_thread_stop(|| { println!("thread stopping"); })
        .build()?
        .block_on(async move {
            let mut server = ServerVSMTP::new(std::sync::Arc::new(config), sockets)?;
            log::info!("Listening on: {:?}", server.addr());

            server
                .with_resolver("maildir", MailDirResolver::default())
                .with_resolver("smtp", SMTPResolver::default())
                .with_resolver("mbox", MBoxResolver::default())
                .listen_and_serve()
                .await
        })
}
