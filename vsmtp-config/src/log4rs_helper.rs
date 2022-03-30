use crate::{log_channel, Config};
use vsmtp_common::re::{anyhow, log};

#[doc(hidden)]
pub fn get_log4rs_config(config: &Config, no_daemon: bool) -> anyhow::Result<log4rs::Config> {
    use anyhow::Context;
    use log4rs::{append, config, encode, Config};

    let server = append::file::FileAppender::builder()
        .encoder(Box::new(encode::pattern::PatternEncoder::new(
            &config.server.logs.format,
        )))
        .build(&config.server.logs.filepath)
        .with_context(|| format!("For filepath: '{}'", config.server.logs.filepath.display()))?;

    let app = append::file::FileAppender::builder()
        .encoder(Box::new(encode::pattern::PatternEncoder::new(
            &config.app.logs.format,
        )))
        .build(&config.app.logs.filepath)
        .with_context(|| format!("For filepath: '{}'", config.app.logs.filepath.display()))?;

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
