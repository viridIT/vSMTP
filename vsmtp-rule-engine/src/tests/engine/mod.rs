use std::net::{IpAddr, Ipv4Addr};

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
use crate::{
    rule_engine::{RuleEngine, RuleState},
    tests::helpers::get_default_state,
};
use vsmtp_common::{state::StateSMTP, status::Status};

#[test]
fn test_engine_errors() {
    let re = RuleEngine::new(&Some(rules_path!["error_handling", "main.vsl"])).unwrap();
    let mut state = get_default_state();

    assert_eq!(re.run_when(&mut state, &StateSMTP::Connect), Status::Next);
    assert_eq!(re.run_when(&mut state, &StateSMTP::Helo), Status::Next);
    assert_eq!(re.run_when(&mut state, &StateSMTP::MailFrom), Status::Deny);
    assert_eq!(re.run_when(&mut state, &StateSMTP::RcptTo), Status::Deny);
}

#[test]
fn test_engine_rules_syntax() {
    let re = RuleEngine::new(&Some(rules_path!["syntax", "main.vsl"])).unwrap();
    let mut state = get_default_state();

    assert_eq!(re.run_when(&mut state, &StateSMTP::Connect), Status::Accept);
    assert_eq!(re.run_when(&mut state, &StateSMTP::Helo), Status::Next);
    assert_eq!(re.run_when(&mut state, &StateSMTP::MailFrom), Status::Next);
    assert_eq!(re.run_when(&mut state, &StateSMTP::RcptTo), Status::Next);
    assert_eq!(re.run_when(&mut state, &StateSMTP::PreQ), Status::Next);
    assert_eq!(re.run_when(&mut state, &StateSMTP::PostQ), Status::Next);
}

#[test]
fn test_rule_state() {
    let config = vsmtp_config::Config::builder()
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
        .with_system_dns()
        .validate()
        .unwrap();

    let state = RuleState::new(&config);
    let state_with_context = RuleState::with_context(
        &config,
        vsmtp_common::mail_context::MailContext {
            connection_timestamp: std::time::SystemTime::now(),
            client_addr: std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
                25,
            ),
            envelop: vsmtp_common::envelop::Envelop {
                helo: "test".to_string(),
                mail_from: vsmtp_common::address::Address::try_from("a@a.a".to_string()).unwrap(),
                rcpt: vec![],
            },
            body: vsmtp_common::mail_context::Body::Empty,
            metadata: None,
        },
    );

    assert_eq!(
        state.get_context().read().unwrap().client_addr.ip(),
        IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))
    );
    assert_eq!(
        state_with_context
            .get_context()
            .read()
            .unwrap()
            .client_addr
            .ip(),
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))
    );
}
