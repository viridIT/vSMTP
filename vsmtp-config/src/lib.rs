//! vSMTP configuration

#![doc(html_no_source)]
#![deny(missing_docs)]
//
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
//
#![allow(clippy::doc_markdown)]

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

///
mod config_builder;
/// The default values of the configuration
pub mod default;
mod parser;
/// The rust representation of the configuration
#[allow(clippy::module_name_repetitions)]
mod server_config;

pub use server_config::{InnerSmtpsConfig, ServerConfig, SniKey, TlsSecurityLevel};

/// The external services used in .vsl format
pub mod service;

#[cfg(test)]
mod tests;

/// targets for log! macro
pub mod log_channel {
    /// receiver system
    pub const RECEIVER: &str = "receiver";
    /// server's rule
    pub const SRULES: &str = "rules";
    /// application side's rule
    pub const URULES: &str = "user_rules";
    /// delivery system
    pub const DELIVER: &str = "deliver";
}

mod next;

#[doc(hidden)]
#[allow(clippy::module_name_repetitions)]
pub fn get_logger_config(
    config: &server_config::ServerConfig,
    no_daemon: bool,
) -> anyhow::Result<log4rs::Config> {
    use log4rs::{append, config, encode, Config};

    let app = append::file::FileAppender::builder()
        .encoder(Box::new(encode::pattern::PatternEncoder::new(
            "{d} - {m}{n}",
        )))
        .build(config.log.file.clone())?;

    let user = append::file::FileAppender::builder()
        .encoder(Box::new(encode::pattern::PatternEncoder::new(
            config
                .rules
                .logs
                .format
                .as_ref()
                .unwrap_or(&"{d} - {m}{n}".to_string()),
        )))
        .build(config.rules.logs.file.clone())?;

    let mut builder = Config::builder();
    let mut root = config::Root::builder();

    if no_daemon {
        builder = builder.appender(
            config::Appender::builder().build(
                "stdout",
                Box::new(
                    append::console::ConsoleAppender::builder()
                        .encoder(Box::new(encode::pattern::PatternEncoder::new(
                            "{d(%Y-%m-%d %H:%M:%S)} {h({l:<5} {I})} ((line:{L:<3})) $ {m}{n}",
                        )))
                        .build(),
                ),
            ),
        );
        root = root.appender("stdout");
    }

    builder
        .appender(config::Appender::builder().build("app", Box::new(app)))
        .appender(config::Appender::builder().build("user", Box::new(user)))
        .loggers(
            config
                .log
                .level
                .iter()
                .map(|(name, level)| config::Logger::builder().build(name, *level)),
        )
        .logger(
            config::Logger::builder()
                .appender("user")
                .additive(false)
                .build(log_channel::URULES, config.rules.logs.level),
        )
        .build(
            root.appender("app").build(
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
