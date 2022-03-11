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
use vsmtp_common::{address::Address, mail_context::Body, status::Status};
use vsmtp_config::{service::Service, ServerConfig};

#[test]
fn test_status() {
    let re = RuleEngine::new(&Some(rules_path!["status", "main.vsl"])).unwrap();
    let mut state = get_default_state();

    assert_eq!(re.run_when(&mut state, "connect"), Status::Accept);
}

#[test]
fn test_time() {
    let re = RuleEngine::new(&Some(rules_path!["time", "main.vsl"])).unwrap();
    let mut state = get_default_state();

    state.add_data("time", std::time::SystemTime::UNIX_EPOCH);

    assert_eq!(re.run_when(&mut state, "connect"), Status::Accept);
}

#[test]
fn test_socket() {
    let re = RuleEngine::new(&Some(rules_path!["socket", "main.vsl"])).unwrap();
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
    let re = RuleEngine::new(&Some(rules_path!["address", "main.vsl"])).unwrap();
    let mut state = get_default_state();

    state.get_context().write().unwrap().envelop.mail_from =
        Address::new("mail.from@test.net").expect("could not parse address");

    assert_eq!(re.run_when(&mut state, "connect"), Status::Accept);
}

#[test]
fn test_objects() {
    let re = RuleEngine::new(&Some(rules_path!["objects", "main.vsl"])).unwrap();
    let mut state = get_default_state();

    assert_eq!(re.run_when(&mut state, "connect"), Status::Next);
}

#[test]
fn test_services() {
    let re = RuleEngine::new(&Some(rules_path!["service", "main.vsl"])).unwrap();

    let config = ServerConfig::builder()
        .with_version_str("<1.0.0")
        .unwrap()
        .with_rfc_port("test.server.com", "root", "root", None)
        .without_log()
        .without_smtps()
        .with_default_smtp()
        .with_delivery("./tmp/delivery")
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
