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
use v_smtp::resolver::ResolverWriteDisk;
use v_smtp::rules::rule_engine;
use v_smtp::server::ServerVSMTP;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = clap::App::new("vSMTP")
        .version("1.0")
        .author("ViridIT https://www.viridit.com")
        .about("vSMTP : the next-gen MTA")
        .get_matches();

    let config: v_smtp::server_config::ServerConfig =
        toml::from_str(&std::fs::read_to_string("./config/vsmtp.toml").unwrap()).unwrap();

    ResolverWriteDisk::init_spool_folder(&config.smtp.spool_dir)?;

    let server = ServerVSMTP::<ResolverWriteDisk>::new(std::sync::Arc::new(config))?;

    rule_engine::init();

    log::warn!("Listening on: {:?}", server.addr());
    server.listen_and_serve().await
}
