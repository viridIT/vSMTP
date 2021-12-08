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
use v_smtp::config::server_config::ServerConfig;
use v_smtp::resolver::ResolverWriteDisk;
use v_smtp::rules::rule_engine;
use v_smtp::server::ServerVSMTP;

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

    let config: ServerConfig = toml::from_str(&std::fs::read_to_string(
        args.value_of("config").expect("clap provide default value"),
    )?)?;

    println!("{:?}", &v_smtp::config::default::DEFAULT_CONFIG.log);

    ResolverWriteDisk::init_spool_folder(&config.smtp.spool_dir)?;
    let server = ServerVSMTP::<ResolverWriteDisk>::new(std::sync::Arc::new(config))?;

    rule_engine::init();

    log::warn!("Listening on: {:?}", server.addr());
    server.listen_and_serve().await
}
