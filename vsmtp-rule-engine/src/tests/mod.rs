/**
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
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

macro_rules! rules_path {
    ( $( $x:expr ),* ) => [
        std::path::PathBuf::from(file!())
            .parent()
            .unwrap()
            .join(std::path::PathBuf::from_iter([ $( $x, )* ]))
            .strip_prefix("vsmtp-rule-engine")
            .unwrap()
            .to_path_buf()
    ];
}

mod actions;
mod email;
mod engine;
mod rules;
mod types;

pub mod helpers {
    use vsmtp_config::server_config::ServerConfig;

    use crate::rule_engine::RuleState;

    pub(super) fn get_default_state() -> RuleState<'static> {
        let config = ServerConfig::builder()
            .with_version_str("<1.0.0")
            .unwrap()
            .with_rfc_port("test.server.com", "root", "root", None)
            .without_log()
            .without_smtps()
            .with_default_smtp()
            .with_delivery("./tmp/delivery")
            .with_rules("./src/receiver/tests/main.vsl", vec![])
            .with_default_reply_codes()
            .build()
            .expect("could not build the default rule state");

        RuleState::new(&config)
    }
}
