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
use config::FileFormat;

pub const DEFAULT_MTA_SERVER_ADDR: &str = "0.0.0.0:25";
pub const DEFAULT_RULES_DIR: &str = "./config/rules";
pub const DEFAULT_SPOOL_PATH: &str = "/var/vsmtp/spool";
pub const DEFAULT_QUARANTINE_DIR: &str = "/var/vsmtp/quarantine";
pub const DEFAULT_CLAMAV: bool = true;
pub const DEFAULT_CLAMAV_PORT: i64 = 3310;
pub const DEFAULT_CLAMAV_ADDRESS: &str = "127.0.0.1";
pub const DEFAULT_CONFIG_PATH: &str = "./config/vsmtp.toml";

lazy_static::lazy_static! {
    static ref CONFIG: config::Config = {
        let mut conf = config::Config::default();

        conf.set_default("server.addr", DEFAULT_MTA_SERVER_ADDR)
            .unwrap()
            .set_default("paths.rules_dir", DEFAULT_RULES_DIR)
            .unwrap()
            .set_default("paths.spool_dir", DEFAULT_SPOOL_PATH)
            .unwrap()
            .set_default("paths.quarantine_dir", DEFAULT_QUARANTINE_DIR)
            .unwrap()
            .set_default("clamav", DEFAULT_CLAMAV)
            .unwrap()
            .set_default("clamav_port", DEFAULT_CLAMAV_PORT)
            .unwrap()
            .set_default("clamav_address", DEFAULT_CLAMAV_ADDRESS)
            .unwrap();

        conf.merge(
            config::File::from(std::path::Path::new(DEFAULT_CONFIG_PATH)).format(FileFormat::Toml),
        )
        .unwrap();

        log::trace!("configuration: {:#?}", conf);

        conf
    };
}

pub fn get<'a, T: serde::Deserialize<'a>>(name: &str) -> Result<T, config::ConfigError> {
    CONFIG.get::<T>(name)
}
