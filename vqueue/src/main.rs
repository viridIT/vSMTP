use vqueue::{execute, Args};
use vsmtp_common::re::anyhow::{self, Context};
use vsmtp_config::Config;

fn main() -> anyhow::Result<()> {
    let args = <Args as clap::StructOpt>::parse();

    let config = args.config.as_ref().map_or_else(
        || Ok(Config::default()),
        |config| {
            std::fs::read_to_string(&config)
                .context(format!("Cannot read file '{}'", config))
                .and_then(|f| Config::from_toml(&f).context("File contains format error"))
                .context("Cannot parse the configuration")
        },
    )?;

    execute(args, config)
}
