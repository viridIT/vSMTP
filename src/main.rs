/**
 * vSMTP mail transfer agent
 * Copyright (C) 2021 viridIT SAS
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
use v_smtp::config;
use v_smtp::resolver::ResolverWriteDisk;
use v_smtp::rules::rule_engine;
use v_smtp::server::ServerVSMTP;

fn get_log_config() -> Result<log4rs::Config, log4rs::config::runtime::ConfigErrors> {
    use log4rs::*;

    let stdout = append::console::ConsoleAppender::builder()
        .encoder(Box::new(encode::pattern::PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)} {h({l:<5} {I})} ((line:{L:<3})) $ {m}{n}",
        )))
        .build();

    let requests = append::file::FileAppender::builder()
        .encoder(Box::new(encode::pattern::PatternEncoder::new(
            "{d} - {m}{n}",
        )))
        .build(crate::config::get::<String>("log.file").unwrap_or_else(|_| "vsmtp.log".to_string()))
        .unwrap();

    fn get_log_level(name: &str) -> Result<log::LevelFilter, ::config::ConfigError> {
        crate::config::get::<String>(name).map(|s| {
            <log::LevelFilter as std::str::FromStr>::from_str(&s).expect("not a valid log level")
        })
    }

    let default_level = get_log_level("log.level.default").unwrap_or(log::LevelFilter::Warn);

    Config::builder()
        .appender(config::Appender::builder().build("stdout", Box::new(stdout)))
        .appender(config::Appender::builder().build("requests", Box::new(requests)))
        .logger(config::Logger::builder().build(
            "rule_engine",
            get_log_level("log.level.rule_engine").unwrap_or(default_level),
        ))
        .logger(config::Logger::builder().build(
            "mail_receiver",
            get_log_level("log.level.mail_receiver").unwrap_or(default_level),
        ))
        .build(
            config::Root::builder()
                .appender("stdout")
                .appender("requests")
                .build(default_level),
        )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    log4rs::init_config(get_log_config()?)?;

    ResolverWriteDisk::init_spool_folder(&config::get::<String>("paths.spool_dir").unwrap())?;

    let server = ServerVSMTP::<ResolverWriteDisk>::new(
        config::get::<Vec<String>>("server.addr")
            .unwrap_or_else(|_| vec![config::DEFAULT_MTA_SERVER_ADDR.to_string()])
            .into_iter()
            .filter_map(|s| match s.parse::<std::net::SocketAddr>() {
                Ok(addr) => Some(addr),
                Err(e) => {
                    log::error!("Failed to parse address from config {}", e);
                    None
                }
            })
            .collect::<Vec<_>>(),
    )?;

    rule_engine::init();

    log::warn!("Listening on: {:?}", server.addr());
    server.listen_and_serve().await
}
