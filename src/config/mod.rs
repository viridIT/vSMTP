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
mod config_builder;
pub mod default;
mod serializer;
pub mod server_config;

pub mod log_channel {
    pub const RECEIVER: &str = "receiver";
    pub const RESOLVER: &str = "resolver";
    pub const RULES: &str = "rules";
    pub const DELIVER: &str = "deliver";
}

pub fn get_logger_config(config: &server_config::ServerConfig) -> anyhow::Result<log4rs::Config> {
    use log4rs::*;

    let console = append::console::ConsoleAppender::builder()
        .encoder(Box::new(encode::pattern::PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)} {h({l:<5} {I})} ((line:{L:<3})) $ {m}{n}",
        )))
        .build();

    let file = append::file::FileAppender::builder()
        .encoder(Box::new(encode::pattern::PatternEncoder::new(
            "{d} - {m}{n}",
        )))
        .build(config.log.file.clone())?;

    Config::builder()
        .appender(config::Appender::builder().build("stdout", Box::new(console)))
        .appender(config::Appender::builder().build("file", Box::new(file)))
        .loggers(
            config
                .log
                .level
                .iter()
                .map(|(name, level)| config::Logger::builder().build(name, *level)),
        )
        .build(
            config::Root::builder()
                .appender("stdout")
                .appender("file")
                .build(
                    *config
                        .log
                        .level
                        .get("default")
                        .unwrap_or(&log::LevelFilter::Warn),
                ),
        )
        .map_err(|e| {
            e.errors().iter().for_each(|e| log::error!("{}", e));
            anyhow::anyhow!(e)
        })
}
