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
mod connect;
mod helo;
mod mail;
mod rcpt;

#[cfg(test)]
pub mod helpers {
    use crate::config::server_config::ServerConfig;

    use crate::rules::rule_engine::RuleState;

    pub(super) fn get_default_state() -> RuleState<'static> {
        let config = ServerConfig::builder()
            .with_rfc_port("test.server.com", None)
            .without_log()
            .without_smtps()
            .with_default_smtp()
            .with_delivery("./tmp/delivery", crate::collection! {})
            .with_rules("./tmp/nothing", vec![])
            .with_default_reply_codes()
            .build()
            .expect("could not build the default rule state");

        RuleState::new(&config)
    }
}
