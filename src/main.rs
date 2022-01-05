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
use vsmtp::config::server_config::ServerConfig;
use vsmtp::resolver::MailDirResolver;
use vsmtp::rules::rule_engine;

/// Simple program to greet a person
#[derive(clap::Parser, Debug)]
#[clap(about, version, author)]
struct Args {
    #[clap(short, long)]
    config: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = <Args as clap::StructOpt>::parse();

    log::warn!("Loading with configuration: \"{:?}\"", args.config);

    let config: ServerConfig =
        toml::from_str(&std::fs::read_to_string(args.config).expect("cannot read file"))
            .expect("cannot parse config from toml");

    MailDirResolver::init_spool_folder(&config.smtp.spool_dir)
        .expect("Failed to initialize the spool directory");

    // the leak is needed to pass from &'a str to &'static str
    // and initialize the rule engine's rule directory.
    let rules_dir = config.rules.dir.clone();
    if let Err(error) = rule_engine::init(Box::leak(rules_dir.into_boxed_str())) {
        // we can't use logs here because it is initialized when building the server.
        // NOTE: should we remove the log initialization inside the server ?
        eprintln!("could not initalize the rule engine: {}", error);
        return Err(error);
    }

    let server = config.build::<MailDirResolver>().await;

    log::warn!("Listening on: {:?}", server.addr());
    server.listen_and_serve().await
}
