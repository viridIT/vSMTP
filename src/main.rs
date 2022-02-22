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
use vsmtp::config::get_logger_config;
use vsmtp::config::server_config::ServerConfig;
use vsmtp::my_libc::{fork, setgid, setuid, Fork};
use vsmtp::resolver::maildir_resolver::MailDirResolver;
use vsmtp::resolver::mbox_resolver::MBoxResolver;
use vsmtp::resolver::smtp_resolver::SMTPResolver;
use vsmtp::server::ServerVSMTP;

#[derive(Debug, clap::Parser, PartialEq)]
#[clap(about, version, author)]
struct Args {
    #[clap(short, long)]
    config: String,

    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, clap::Subcommand, PartialEq)]
enum Commands {
    /// Show the loaded config (as json)
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
                command: Some(crate::Commands::ConfigShow),
                config: "path".to_string()
            },
            <crate::Args as clap::StructOpt>::try_parse_from(&["", "-c", "path", "config-show"])
                .unwrap()
        );

        assert_eq!(
            crate::Args {
                command: Some(crate::Commands::ConfigDiff),
                config: "path".to_string()
            },
            <crate::Args as clap::StructOpt>::try_parse_from(&["", "-c", "path", "config-diff"])
                .unwrap()
        );
    }
}

fn main() -> anyhow::Result<()> {
    let args = <Args as clap::StructOpt>::parse();
    println!("Loading configuration at path='{}'", args.config);

    let config = ServerConfig::from_toml(&std::fs::read_to_string(&args.config)?)?;

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
                        .with_rfc_port(&config.server.domain, "vsmtp", "vsmtp", None)
                        .without_log()
                        // TODO: default
                        .without_smtps()
                        .with_default_smtp()
                        // TODO: default
                        .with_delivery("/var/spool/vsmtp", vsmtp::collection! {})
                        // TODO: default
                        .with_rules("/etc/vsmtp/rules", vec![])
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

    log4rs::init_config(get_logger_config(&config)?)?;

    let vsmtp_user = users::get_user_by_name(&config.server.vsmtp_user)
        .ok_or_else(|| anyhow::anyhow!("user not found: '{}'", config.server.vsmtp_user))?;

    let vsmtp_group = users::get_group_by_name(&config.server.vsmtp_group)
        .ok_or_else(|| anyhow::anyhow!("group not found: '{}'", config.server.vsmtp_group))?;

    fn socket_bind_anyhow(addr: std::net::SocketAddr) -> anyhow::Result<std::net::TcpListener> {
        std::net::TcpListener::bind(addr).map_err(|e| anyhow::anyhow!("{e}: '{addr}'"))
    }

    let sockets = (
        socket_bind_anyhow(config.server.addr)?,
        socket_bind_anyhow(config.server.addr_submission)?,
        socket_bind_anyhow(config.server.addr_submissions)?,
    );

    match fork()? {
        Fork::Child => {
            setgid(vsmtp_group.gid())?;
            setuid(vsmtp_user.uid())?;

            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(config.server.thread_count)
                .enable_all()
                // .on_thread_start(|| { println!("thread started"); })
                // .on_thread_stop(|| { println!("thread stopping"); })
                .build()?
                .block_on(async move {
                    let mut server = ServerVSMTP::new(std::sync::Arc::new(config), sockets)
                        .expect("Failed to create the server");
                    log::warn!("Listening on: {:?}", server.addr());

                    server
                        .with_resolver("maildir", MailDirResolver::default())
                        .with_resolver("smtp", SMTPResolver::default())
                        .with_resolver("mbox", MBoxResolver::default())
                        .listen_and_serve()
                        .await
                })
        }
        Fork::Parent(child) => {
            println!("vsmtp is running on process: {}", child);
            Ok(())
        }
    }
}
