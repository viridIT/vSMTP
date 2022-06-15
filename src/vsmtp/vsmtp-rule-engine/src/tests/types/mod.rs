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
use crate::{rule_engine::RuleEngine, rule_state::RuleState, tests::helpers::get_default_state};
use vsmtp_common::{
    addr, mail_context::MessageBody, state::StateSMTP, status::Status, CodeID, ReplyOrCodeID,
};
use vsmtp_config::{builder::VirtualEntry, field::FieldServerDNS, Config};

#[test]
fn test_status() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(rules_path!["status", "main.vsl"]),
    )
    .unwrap();
    let (mut state, _) = get_default_state("./tmp/app");

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::Connect),
        Status::Accept(ReplyOrCodeID::CodeID(CodeID::Ok)),
    );
}

#[test]
fn test_time_and_date() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(rules_path!["time", "main.vsl"]),
    )
    .unwrap();
    let (mut state, _) = get_default_state("./tmp/app");

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::Connect),
        Status::Accept(ReplyOrCodeID::CodeID(CodeID::Ok)),
    );
}

#[test]
fn test_ip() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(rules_path!["ip", "main.vsl"]),
    )
    .unwrap();
    let (mut state, _) = get_default_state("./tmp/app");

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::Connect),
        Status::Accept(ReplyOrCodeID::CodeID(CodeID::Ok)),
    );
}

#[test]
fn test_address() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(rules_path!["address", "main.vsl"]),
    )
    .unwrap();
    let (mut state, _) = get_default_state("./tmp/app");

    state.context().write().unwrap().envelop.mail_from = addr!("mail.from@test.net");

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::Connect),
        Status::Accept(ReplyOrCodeID::CodeID(CodeID::Ok)),
    );
}

#[test]
fn test_objects() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(rules_path!["objects", "main.vsl"]),
    )
    .unwrap();
    let (mut state, _) = get_default_state("./tmp/app");

    assert_eq!(re.run_when(&mut state, &StateSMTP::Connect), Status::Next);
}

#[test]
fn test_services() {
    let config = Config::builder()
        .with_version_str("<1.0.0")
        .unwrap()
        .with_server_name("testserver.com")
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
        .with_app_at_location("./tmp/app")
        .with_vsl("./tmp/nothing")
        .with_default_app_logs()
        .with_system_dns()
        .without_virtual_entries()
        .validate()
        .unwrap();

    let re = RuleEngine::new(&config, &Some(rules_path!["service", "main.vsl"])).unwrap();
    let resolvers = std::sync::Arc::new(std::collections::HashMap::new());
    let mut state = RuleState::new(&config, resolvers, &re);

    *state.message().write().unwrap() = Some(MessageBody::Raw {
        headers: vec![],
        body: "".to_string(),
    });

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::Connect),
        Status::Accept(ReplyOrCodeID::CodeID(CodeID::Ok)),
    );
}

#[test]
fn test_config_display() {
    let config = Config::builder()
        .with_version_str("<1.0.0")
        .unwrap()
        .with_server_name("testserver.com")
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
        .with_app_at_location("./tmp/app")
        .with_vsl("./tmp/nothing")
        .with_default_app_logs()
        .with_system_dns()
        .with_virtual_entries(&[VirtualEntry {
            domain: "domain@example.com".to_string(),
            tls: Some((
                root_example!["../config/tls/certificate.crt"]
                    .to_str()
                    .unwrap()
                    .to_string(),
                root_example!["../config/tls/private_key.key"]
                    .to_str()
                    .unwrap()
                    .to_string(),
            )),
            dns: Some(FieldServerDNS::System),
        }])
        .unwrap()
        .validate()
        .unwrap();

    let re = RuleEngine::new(&config, &Some(rules_path!["objects", "main.vsl"])).unwrap();
    let resolvers = std::sync::Arc::new(std::collections::HashMap::new());
    let mut state = RuleState::new(&config, resolvers, &re);

    *state.message().write().unwrap() = Some(MessageBody::Raw {
        headers: vec![],
        body: "".to_string(),
    });

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::Helo),
        Status::Accept(ReplyOrCodeID::CodeID(CodeID::Ok)),
    );
}
