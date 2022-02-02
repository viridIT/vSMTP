mod config_builder;
// pub mod custom_code;
// pub mod default;
pub mod serializer;
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
