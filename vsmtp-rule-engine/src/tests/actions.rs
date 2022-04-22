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
use crate::{rule_engine::RuleEngine, tests::helpers::get_default_state};
use vsmtp_common::re::serde_json;
use vsmtp_common::{
    mail::Mail,
    mail_context::{Body, MessageMetadata},
    state::StateSMTP,
    status::Status,
    transfer::Transfer,
};

#[test]
fn test_logs() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(root_example!["actions/logs.vsl"]),
    )
    .unwrap();
    let (mut state, _) = get_default_state("./tmp/app");
    assert_eq!(re.run_when(&mut state, &StateSMTP::Connect), Status::Deny);
}

#[test]
fn test_users() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(root_example!["actions/users.vsl"]),
    )
    .unwrap();
    let (mut state, _) = get_default_state("./tmp/app");

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::Delivery),
        Status::Accept
    );
}

#[test]
fn test_send_mail() {
    let (mut state, config) = get_default_state(format!("{}", root_example!["actions"].display()));
    let re = RuleEngine::new(&config, &Some(root_example!["actions/send_mail.vsl"])).unwrap();

    // TODO: add test to send a valid email.
    assert_eq!(re.run_when(&mut state, &StateSMTP::Connect), Status::Accept);
}

#[test]
fn test_context_write() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(root_example!["actions/write.vsl"]),
    )
    .unwrap();
    let (mut state, _) = get_default_state("./tmp/app");

    state.get_context().write().unwrap().metadata = Some(MessageMetadata {
        message_id: "test_message_id".to_string(),
        timestamp: std::time::SystemTime::now(),
        skipped: None,
    });
    assert_eq!(
        re.run_when(&mut state, &StateSMTP::MailFrom),
        Status::Accept
    );
    state.get_context().write().unwrap().body = Body::Raw(
        r#"From: john doe <john@doe.com>
To: green@foo.net
Subject: test email

This is a raw email.
"#
        .to_string(),
    );
    assert_eq!(re.run_when(&mut state, &StateSMTP::PreQ), Status::Accept);
    assert_eq!(re.run_when(&mut state, &StateSMTP::PostQ), Status::Accept);

    // raw mail should have been written on disk.
    assert_eq!(
        std::fs::read_to_string("./tmp/app/tests/generated/test_message_id.eml")
            .expect("could not read 'test_message_id'"),
        r#"From: john doe <john@doe.com>
To: green@foo.net
Subject: test email

This is a raw email.
"#
    );

    std::fs::remove_file("./tmp/app/tests/generated/test_message_id.eml")
        .expect("could not remove generated test file");
}

#[test]
fn test_context_dump() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(root_example!["actions/dump.vsl"]),
    )
    .unwrap();
    let (mut state, _) = get_default_state("./tmp/app");

    state.get_context().write().unwrap().metadata = Some(MessageMetadata {
        message_id: "test_message_id".to_string(),
        timestamp: std::time::SystemTime::now(),
        skipped: None,
    });
    state.get_context().write().unwrap().body = Body::Raw(String::default());
    assert_eq!(re.run_when(&mut state, &StateSMTP::PreQ), Status::Accept);
    state.get_context().write().unwrap().body = Body::Parsed(Box::new(Mail {
        headers: vec![
            ("From".to_string(), "john@doe.com".to_string()),
            ("To".to_string(), "green@bar.net".to_string()),
            ("X-Custom-Header".to_string(), "my header".to_string()),
        ],
        body: vsmtp_common::mail::BodyType::Regular(vec!["this is an empty body".to_string()]),
    }));
    assert_eq!(re.run_when(&mut state, &StateSMTP::PostQ), Status::Accept);

    assert_eq!(
        std::fs::read_to_string("./tmp/app/tests/generated/test_message_id.json")
            .expect("could not read 'test_message_id'"),
        serde_json::to_string_pretty(&*state.get_context().read().unwrap())
            .expect("couldn't convert context into string")
    );

    std::fs::remove_file("./tmp/app/tests/generated/test_message_id.json")
        .expect("could not remove generated test file");
}

#[test]
fn test_quarantine() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(root_example!["actions/quarantine.vsl"]),
    )
    .unwrap();
    let (mut state, _) = get_default_state("./tmp/app");

    state.get_context().write().unwrap().metadata = Some(MessageMetadata {
        message_id: "test_message_id".to_string(),
        timestamp: std::time::SystemTime::now(),
        skipped: None,
    });
    state.get_context().write().unwrap().body = Body::Raw(String::default());
    assert_eq!(re.run_when(&mut state, &StateSMTP::PreQ), Status::Accept);

    assert!(state
        .get_context()
        .read()
        .unwrap()
        .envelop
        .rcpt
        .iter()
        .all(|rcpt| rcpt.transfer_method == Transfer::None));

    state.get_context().write().unwrap().body = Body::Parsed(Box::new(Mail {
        headers: vec![
            ("From".to_string(), "john@doe.com".to_string()),
            ("To".to_string(), "green@bar.net".to_string()),
            ("X-Custom-Header".to_string(), "my header".to_string()),
        ],
        body: vsmtp_common::mail::BodyType::Regular(vec!["this is an empty body".to_string()]),
    }));
    assert_eq!(re.run_when(&mut state, &StateSMTP::PostQ), Status::Deny);

    assert_eq!(
        std::fs::read_to_string("./tmp/app/tests/generated/quarantine2/test_message_id.json")
            .expect("could not read 'test_message_id'"),
        serde_json::to_string_pretty(&*state.get_context().read().unwrap())
            .expect("couldn't convert context into string")
    );

    std::fs::remove_file("./tmp/app/tests/generated/quarantine1/test_message_id.json")
        .expect("could not remove generated test file");
    std::fs::remove_file("./tmp/app/tests/generated/quarantine2/test_message_id.json")
        .expect("could not remove generated test file");
}
