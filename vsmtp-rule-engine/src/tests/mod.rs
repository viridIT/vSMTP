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
    use vsmtp_config::Config;

    use crate::rule_engine::RuleState;

    pub(super) fn get_default_state() -> RuleState<'static> {
        let config = Config::builder()
            .with_version_str("<1.0.0")
            .unwrap()
            .with_server_name_and_client_count("testserver.com", 32)
            .with_user_group_and_default_system("root", "root")
            .with_ipv4_localhost()
            .with_default_logs_settings()
            .with_spool_dir_and_default_queues("./tmp/delivery")
            .without_tls_support()
            .with_default_smtp_options()
            .with_default_smtp_error_handler()
            .with_default_smtp_codes()
            .with_default_app()
            .with_vsl("./src/receiver/tests/main.vsl")
            .with_default_app_logs()
            .without_services()
            .validate()
            .unwrap();

        RuleState::new(&config)
    }

    static INIT_LOGS: std::sync::Once = std::sync::Once::new();

    pub fn setup_logs_for_tests() {
        INIT_LOGS.call_once(|| {
            let mut config = ServerConfig::builder()
                .with_version_str("<1.0.0")
                .unwrap()
                .with_rfc_port("test.server.com", "root", "root", None)
                .without_log()
                .without_smtps()
                .with_default_smtp()
                .with_delivery("./tmp/delivery", "none")
                .with_rules("./src/receiver/tests/main.vsl", vec![])
                .with_default_reply_codes()
                .build()
                .expect("failed to create config for logs");

            config
                .log
                .level
                .insert("default".into(), log::LevelFilter::Warn);
            config.log.file =
                std::path::PathBuf::from_iter([".", "tests", "generated", "app.test.log"]);
            config.rules.logs.file =
                std::path::PathBuf::from_iter([".", "tests", "generated", "rules.test.log"]);

            log4rs::init_config(
                vsmtp_config::get_logger_config(&config, true).expect("failed to init logs"),
            )
            .expect("failed to init logs");
        });
    }
}
