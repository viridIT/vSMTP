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
use vsmtp::resolver::maildir_resolver::MailDirResolver;
use vsmtp::resolver::mbox_resolver::MBoxResolver;
use vsmtp::resolver::smtp_resolver::SMTPResolver;
use vsmtp::rules::rule_engine;
use vsmtp::server::ServerVSMTP;

async fn server_main(config: std::sync::Arc<ServerConfig>) -> anyhow::Result<()> {
    log4rs::init_config(get_logger_config(&config)?)?;

    rule_engine::init(Box::leak(config.rules.dir.clone().into_boxed_str())).map_err(|error| {
        log::error!("could not initialize the rule engine: {}", error);
        error
    })?;

    let mut server = ServerVSMTP::new(config)
        .await
        .expect("Failed to create the server");
    log::warn!("Listening on: {:?}", server.addr());

    server
        .with_resolver("maildir", MailDirResolver::default())
        .with_resolver("smtp", SMTPResolver::default())
        .with_resolver("mbox", MBoxResolver::default())
        .listen_and_serve()
        .await
}

#[derive(clap::Parser, Debug)]
#[clap(about, version, author)]
#[clap(global_setting(clap::AppSettings::UseLongFormatForHelpSubcommand))]
struct Args {
    #[clap(short, long)]
    config: String,

    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, clap::Subcommand)]
enum Commands {
    /// Show the loaded config (as json)
    ConfigShow,
    /// Show the difference between the loaded config and the default one
    ConfigDiff,
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
                        .with_rfc_port(&config.server.domain, None)
                        .without_log()
                        // TODO: default
                        .without_smtps()
                        .with_default_smtp()
                        // TODO: default
                        .with_delivery("/var/spool/vsmtp", vsmtp::collection! {})
                        // TODO: default
                        .with_rules("/etc/vsmtp/rules")
                        .with_default_reply_codes()
                        .build(),
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

    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.server.thread_count)
        .enable_all()
        .build()?
        .block_on(server_main(std::sync::Arc::new(config)))
}
