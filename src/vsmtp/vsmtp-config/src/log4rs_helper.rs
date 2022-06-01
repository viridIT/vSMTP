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
use crate::{log_channel, Config};
use log4rs::append::rolling_file::RollingFileAppender;
use vsmtp_common::re::{anyhow, log};

fn init_rolling_log(
    format: &str,
    filepath: &std::path::Path,
    size_limit: u64,
    archive_count: u32,
) -> anyhow::Result<log4rs::append::rolling_file::RollingFileAppender> {
    use anyhow::Context;
    use log4rs::{
        append::rolling_file::policy::compound::{roll, trigger, CompoundPolicy},
        encode,
    };

    RollingFileAppender::builder()
        .append(true)
        .encoder(Box::new(encode::pattern::PatternEncoder::new(format)))
        .build(
            filepath,
            Box::new(CompoundPolicy::new(
                Box::new(trigger::size::SizeTrigger::new(size_limit)),
                Box::new(
                    roll::fixed_window::FixedWindowRoller::builder()
                        .base(0)
                        .build(
                            &format!("{}-ar/trace.{{}}.gz", filepath.display()),
                            archive_count,
                        )
                        .expect("{} in pattern && log4rs built with gzip"),
                ),
            )),
        )
        .with_context(|| format!("For filepath: '{}'", filepath.display()))
}

#[doc(hidden)]
pub fn get_log4rs_config(config: &Config, no_daemon: bool) -> anyhow::Result<log4rs::Config> {
    use log4rs::{append, config, encode, Config};

    let server: RollingFileAppender = init_rolling_log(
        &config.server.logs.format,
        &config.server.logs.filepath,
        config.server.logs.size_limit,
        config.server.logs.archive_count,
    )?;
    let app: RollingFileAppender = init_rolling_log(
        &config.app.logs.format,
        &config.app.logs.filepath,
        config.app.logs.size_limit,
        config.app.logs.archive_count,
    )?;

    let mut log_builder = Config::builder();
    let mut log_root_builder = config::Root::builder();

    if no_daemon {
        log_builder = log_builder.appender(
            config::Appender::builder().build(
                "stdout",
                Box::new(
                    append::console::ConsoleAppender::builder()
                        .encoder(Box::new(encode::pattern::PatternEncoder::new(
                            &config.server.logs.format,
                        )))
                        .build(),
                ),
            ),
        );

        log_root_builder = log_root_builder.appender("stdout");
    }

    log_builder
        .appender(config::Appender::builder().build(log_channel::DEFAULT, Box::new(server)))
        .appender(config::Appender::builder().build(log_channel::APP, Box::new(app)))
        .loggers(config.server.logs.level.iter().filter_map(|(name, level)| {
            // adding all loggers under the "server" logger to simulate a root logger.
            match name.as_str() {
                log_channel::DEFAULT | "root" => None,
                _ => Some(
                    config::Logger::builder()
                        .build(format!("{}::{}", log_channel::DEFAULT, name), *level),
                ),
            }
        }))
        .logger(
            config::Logger::builder()
                .appender("app")
                .build(log_channel::APP, config.app.logs.level),
        )
        // vSMTP's "root" logger under the name "default", all sub loggers inherit from this one.
        .logger(
            config::Logger::builder().appender("server").build(
                log_channel::DEFAULT,
                *config
                    .server
                    .logs
                    .level
                    .get(log_channel::DEFAULT)
                    .unwrap_or(&log::LevelFilter::Warn),
            ),
        )
        .build(
            // true "root" logger, enabling it set logs for vSMTP's dependencies.
            // the user doesn't need to set this 99% of the time.
            log_root_builder.appender("server").build(
                *config
                    .server
                    .logs
                    .level
                    .get("root")
                    .unwrap_or(&log::LevelFilter::Error),
            ),
        )
        .map_err(anyhow::Error::new)
}

#[cfg(test)]
mod tests {
    use crate::Config;

    use super::get_log4rs_config;

    #[test]
    fn init() {
        let mut config = Config::default();
        config.app.logs.filepath = "./tmp/app.log".into();
        config.server.logs.filepath = "./tmp/vsmtp.log".into();

        let res = get_log4rs_config(&config, false);
        assert!(res.is_ok(), "{:?}", res);
    }

    #[test]
    fn init_with_console() {
        let mut config = Config::default();
        config.app.logs.filepath = "./tmp/app.log".into();
        config.server.logs.filepath = "./tmp/vsmtp.log".into();

        let res = get_log4rs_config(&config, true);
        assert!(res.is_ok(), "{:?}", res);
    }

    #[test]
    fn invalid_server_path() {
        let mut config = Config::default();
        config.app.logs.filepath = "./tmp/app.log".into();
        config.server.logs.filepath = "/root/var/vsmtp.log".into();

        let res = get_log4rs_config(&config, true);
        assert!(res.is_err(), "{:?}", res);
    }

    #[test]
    fn invalid_app_path() {
        let mut config = Config::default();
        config.app.logs.filepath = "/root/var/app.log".into();
        config.server.logs.filepath = "./tmp/vsmtp.log".into();

        let res = get_log4rs_config(&config, true);
        assert!(res.is_err(), "{:?}", res);
    }
}
