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
use vsmtp::resolver::ResolverWriteDisk;
use vsmtp::rules::rule_engine;
use vsmtp::server::ServerVSMTP;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = clap::App::new("vSMTP")
        .version("1.0")
        .author("ViridIT https://www.viridit.com")
        .about("vSMTP : the next-gen MTA")
        .arg(
            clap::Arg::with_name("config")
                .short("-c")
                .long("--config")
                .takes_value(true)
                .default_value("config/vsmtp.toml"),
        )
        .get_matches();

    let config: ServerConfig = toml::from_str(
        &std::fs::read_to_string(args.value_of("config").expect("clap provide default value"))
            .expect("Failed to get the default config"),
    )
    .expect("Failed to create toml config");

    ResolverWriteDisk::init_spool_folder(&config.smtp.spool_dir)
        .expect("Failed to initialize the spool directory");

    // the leak is needed to pass from &'a str to &'static str
    // and initialise the rule engine's rule directory.
    let rules_dir = config.rules.dir.clone();
    rule_engine::init(Box::leak(rules_dir.into_boxed_str()));

    let server = ServerVSMTP::<ResolverWriteDisk>::new(std::sync::Arc::new(config))
        .expect("Failed to create the server");

    log::warn!("Listening on: {:?}", server.addr());
    server.listen_and_serve().await
}
