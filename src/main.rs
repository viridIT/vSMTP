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
use vsmtp::config::get_logger_config;
use vsmtp::config::server_config::ServerConfig;
use vsmtp::rules::rule_engine;
use vsmtp::server::ServerVSMTP;

#[derive(clap::Parser, Debug)]
#[clap(about, version, author)]
struct Args {
    #[clap(short, long)]
    config: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = <Args as clap::StructOpt>::parse();
    println!("Loading with configuration: '{}'", args.config);

    let mut config: ServerConfig =
        toml::from_str(&std::fs::read_to_string(args.config).expect("cannot read file"))
            .expect("cannot parse config from toml");
    config.prepare();
    let config = std::sync::Arc::new(config);

    log4rs::init_config(get_logger_config(&config)?)?;

    rule_engine::init(Box::leak(config.rules.dir.clone().into_boxed_str())).map_err(|error| {
        log::error!("could not initialize the rule engine: {}", error);
        error
    })?;

    let server = ServerVSMTP::new(config.clone())
        .await
        .expect("Failed to create the server");
    log::warn!("Listening on: {:?}", server.addr());

    server.listen_and_serve().await
}
