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
mod actions;
mod email;
mod engine;
mod rules;
mod types;

pub mod helpers {
    use crate::config::server_config::ServerConfig;

    use crate::rules::rule_engine::RuleState;

    pub(super) fn get_default_state() -> RuleState<'static> {
        let config = ServerConfig::builder()
            .with_version_str("<1.0.0")
            .unwrap()
            .with_rfc_port("test.server.com", "root", "root", None)
            .without_log()
            .without_smtps()
            .with_default_smtp()
            .with_delivery("./tmp/delivery", crate::collection! {})
            .with_rules("./src/receiver/tests/main.vsl", vec![])
            .with_default_reply_codes()
            .build()
            .expect("could not build the default rule state");

        RuleState::new(&config)
    }
}
