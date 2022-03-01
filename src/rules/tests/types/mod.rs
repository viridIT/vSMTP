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
    config::server_config::{ServerConfig, Service},
    rules::{
        address::Address,
        rule_engine::{RuleEngine, RuleState, Status},
        tests::helpers::get_default_state,
    },
    smtp::mail::Body,
};

#[test]
fn test_status() {
    crate::receiver::test_helpers::logs::setup_logs();

    let re = RuleEngine::new("./src/rules/tests/types/status".into())
        .expect("couldn't build rule engine");
    let mut state = get_default_state();

    assert_eq!(re.run_when(&mut state, "connect"), Status::Accept);
}

#[test]
fn test_time() {
    crate::receiver::test_helpers::logs::setup_logs();

    let re =
        RuleEngine::new("./src/rules/tests/types/time".into()).expect("couldn't build rule engine");
    let mut state = get_default_state();

    state.add_data("time", std::time::SystemTime::UNIX_EPOCH);

    assert_eq!(re.run_when(&mut state, "connect"), Status::Accept);
}

#[test]
fn test_socket() {
    crate::receiver::test_helpers::logs::setup_logs();

    let re = RuleEngine::new("./src/rules/tests/types/socket".into())
        .expect("couldn't build rule engine");
    let mut state = get_default_state();

    state.add_data(
        "custom_socket",
        <std::net::SocketAddr as std::str::FromStr>::from_str("127.0.0.1:25")
            .expect("could not build socket"),
    );

    assert_eq!(re.run_when(&mut state, "connect"), Status::Accept);
}

#[test]
fn test_address() {
    crate::receiver::test_helpers::logs::setup_logs();

    let re = RuleEngine::new("./src/rules/tests/types/address".into())
        .expect("couldn't build rule engine");
    let mut state = get_default_state();

    state.get_context().write().unwrap().envelop.mail_from =
        Address::new("mail.from@test.net").expect("could not parse address");

    assert_eq!(re.run_when(&mut state, "connect"), Status::Accept);
}

#[test]
fn test_objects() {
    crate::receiver::test_helpers::logs::setup_logs();

    let re = RuleEngine::new("./src/rules/tests/types/objects".into())
        .expect("couldn't build rule engine");
    let mut state = get_default_state();

    assert_eq!(re.run_when(&mut state, "connect"), Status::Next);
}

#[test]
fn test_services() {
    crate::receiver::test_helpers::logs::setup_logs();

    let re = RuleEngine::new("./src/rules/tests/types/service".into())
        .expect("couldn't build rule engine");

    let config = ServerConfig::builder()
        .with_version("")
        .with_rfc_port("test.server.com", "foo", "foo", None)
        .without_log()
        .without_smtps()
        .with_default_smtp()
        .with_delivery("./tmp/delivery", crate::collection! {})
        .with_rules(
            "./tmp/nothing",
            vec![Service::UnixShell {
                name: "shell".to_string(),
                timeout: std::time::Duration::from_secs(2),
                user: None,
                group: None,
                command: "echo".to_string(),
                args: Some("test".to_string()),
            }],
        )
        .with_default_reply_codes()
        .build()
        .expect("could not build the default rule state");

    let mut state = RuleState::new(&config);

    state.get_context().write().unwrap().body = Body::Raw(String::default());

    assert_eq!(re.run_when(&mut state, "connect"), Status::Accept);
}
