use crate::{log_channel, Config};
use vsmtp_common::re::{anyhow, log};

fn init_rolling_log(
    format: &str,
    path: &std::path::Path,
    size_limit: u64,
    archive_count: u32,
) -> anyhow::Result<log4rs::append::rolling_file::RollingFileAppender> {
    use anyhow::Context;
    use log4rs::{
        append::rolling_file::{
            policy::compound::{roll, trigger, CompoundPolicy},
            RollingFileAppender,
        },
        encode,
    };

    RollingFileAppender::builder()
        .append(true)
        .encoder(Box::new(encode::pattern::PatternEncoder::new(format)))
        .build(
            path,
            Box::new(CompoundPolicy::new(
                Box::new(trigger::size::SizeTrigger::new(size_limit)),
                Box::new(
                    roll::fixed_window::FixedWindowRoller::builder()
                        .base(0)
                        .build(
                            &format!("{}-ar/trace.{{}}.gz", path.display()),
                            archive_count,
                        )
                        .unwrap(),
                ),
            )),
        )
        .with_context(|| format!("For filepath: '{}'", path.display()))
}

#[doc(hidden)]
pub fn get_log4rs_config(config: &Config, no_daemon: bool) -> anyhow::Result<log4rs::Config> {
    use log4rs::{append, config, encode, Config};

    let server = init_rolling_log(
        &config.server.logs.format,
        &config.server.logs.filepath,
        config.app.logs.size_limit,
        config.app.logs.archive_count,
    )?;
    let app = init_rolling_log(
        &config.app.logs.format,
        &config.app.logs.filepath,
        config.app.logs.size_limit,
        config.app.logs.archive_count,
    )?;

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
        .appender(config::Appender::builder().build("server", Box::new(server)))
        .appender(config::Appender::builder().build("app", Box::new(app)))
        .loggers(
            config
                .server
                .logs
                .level
                .iter()
                .map(|(name, level)| config::Logger::builder().build(name, *level)),
        )
        .logger(
            config::Logger::builder()
                .appender("app")
                .additive(false)
                .build(log_channel::URULES, config.app.logs.level),
        )
        .build(
            root.appender("server").build(
                *config
                    .server
                    .logs
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

#[cfg(test)]
mod tests {
    use crate::Config;

    use super::get_log4rs_config;

    #[test]
    fn init() {
        let mut config = Config::default();
        config.app.logs.filepath = "./tmp/app.log".into();
        config.server.logs.filepath = "./tmp/vsmtp.log".into();

        let res = get_log4rs_config(&config, true);
        assert!(res.is_ok(), "{:?}", res);
        let res = get_log4rs_config(&config, false);
        assert!(res.is_ok(), "{:?}", res);
    }
}
