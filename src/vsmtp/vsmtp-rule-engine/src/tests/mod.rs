/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/

macro_rules! rules_path {
    ( $( $x:expr ),* ) => [
        std::path::PathBuf::from(file!())
            .parent()
            .unwrap()
            .join(std::path::PathBuf::from_iter([ $( $x, )* ]))
            .strip_prefix("src/vsmtp/vsmtp-rule-engine")
            .unwrap()
            .to_path_buf()
    ];
}

macro_rules! root_example {
    ( $( $x:expr ),* ) => {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples/vsl")
            .join(std::path::PathBuf::from_iter([ $( $x, )* ]))
            .to_path_buf()
    };
}

mod actions;
mod email;
mod engine;
mod integrations;
mod rules;
mod types;

pub mod helpers {
    use vsmtp_config::Config;

    use crate::{rule_engine::RuleEngine, rule_state::RuleState};

    pub(super) fn get_default_config(dirpath: impl Into<std::path::PathBuf>) -> Config {
        Config::builder()
            .with_version_str("<1.0.0")
            .unwrap()
            .with_server_name_and_client_count("testserver.com", 32)
            .with_user_group_and_default_system("root", "root")
            .unwrap()
            .with_ipv4_localhost()
            .with_default_logs_settings()
            .with_spool_dir_and_default_queues("./tmp/delivery")
            .without_tls_support()
            .with_default_smtp_options()
            .with_default_smtp_error_handler()
            .with_default_smtp_codes()
            .without_auth()
            .with_app_at_location(dirpath)
            .with_vsl("./src/tests/empty_main.vsl")
            .with_default_app_logs()
            .with_system_dns()
            .without_virtual_entries()
            .validate()
            .unwrap()
    }

    /// create a rule engine state with it's associated configuration.
    pub(super) fn get_default_state(dirpath: impl Into<std::path::PathBuf>) -> (RuleState, Config) {
        let config = Config::builder()
            .with_version_str("<1.0.0")
            .unwrap()
            .with_server_name_and_client_count("testserver.com", 32)
            .with_user_group_and_default_system("root", "root")
            .unwrap()
            .with_ipv4_localhost()
            .with_default_logs_settings()
            .with_spool_dir_and_default_queues("./tmp/delivery")
            .without_tls_support()
            .with_default_smtp_options()
            .with_default_smtp_error_handler()
            .with_default_smtp_codes()
            .without_auth()
            .with_app_at_location(dirpath)
            .with_vsl("./src/tests/empty_main.vsl")
            .with_default_app_logs()
            .with_system_dns()
            .without_virtual_entries()
            .validate()
            .unwrap();

        let re = RuleEngine::from_script(&config, "#{}").unwrap();
        let resolvers = std::sync::Arc::new(std::collections::HashMap::new());
        (RuleState::new(&config, resolvers, &re), config)
    }
}
