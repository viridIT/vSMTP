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
    mime::mail::{BodyType, Mail},
    rules::{
        rule_engine::{RuleEngine, Status},
        tests::helpers::get_default_state,
    },
    smtp::mail::{Body, MessageMetadata},
};

#[test]
fn test_email_context() {
    crate::receiver::test_helpers::logs::setup();

    let re = RuleEngine::new(&Some("./src/rules/tests/email/main.vsl".into()))
        .expect("couldn't build rule engine");
    let mut state = get_default_state();

    assert_eq!(re.run_when(&mut state, "connect"), Status::Accept);
    state.get_context().write().unwrap().body = Body::Raw(String::default());
    assert_eq!(re.run_when(&mut state, "preq"), Status::Accept);
    state.get_context().write().unwrap().body = Body::Parsed(Box::new(Mail {
        headers: vec![],
        body: BodyType::Regular(vec![]),
    }));
    state.get_context().write().unwrap().metadata = Some(MessageMetadata::default());
    assert_eq!(re.run_when(&mut state, "postq"), Status::Accept);
}

#[test]
fn test_email_bcc() {
    crate::receiver::test_helpers::logs::setup();

    let re = RuleEngine::new(&Some("./src/rules/tests/email/bcc/main.vsl".into()))
        .expect("couldn't build rule engine");
    let mut state = get_default_state();

    assert_eq!(re.run_when(&mut state, "postq"), Status::Accept);
}

#[test]
fn test_email_add_header() {
    crate::receiver::test_helpers::logs::setup();

    let re = RuleEngine::new(&Some("./src/rules/tests/email/add_header/main.vsl".into()))
        .expect("couldn't build rule engine");
    let mut state = get_default_state();

    assert_eq!(re.run_when(&mut state, "mail"), Status::Accept);
    state.get_context().write().unwrap().body = Body::Raw(String::default());
    assert_eq!(re.run_when(&mut state, "preq"), Status::Accept);
    state.get_context().write().unwrap().body = Body::Parsed(Box::new(Mail {
        headers: vec![],
        body: BodyType::Regular(vec![]),
    }));
    state.get_context().write().unwrap().metadata = Some(MessageMetadata::default());
    assert_eq!(re.run_when(&mut state, "postq"), Status::Accept);
}

#[test]
fn test_context_write() {
    crate::receiver::test_helpers::logs::setup();
    std::fs::DirBuilder::new()
        .recursive(true)
        .create("./tests/generated")
        .unwrap();

    let re = RuleEngine::new(&Some("./src/rules/tests/email/write/main.vsl".into()))
        .expect("couldn't build rule engine");
    let mut state = get_default_state();

    state.get_context().write().unwrap().metadata = Some(MessageMetadata {
        message_id: "test_message_id".to_string(),
        timestamp: std::time::SystemTime::now(),
        retry: 0,
        resolver: "default".to_string(),
        skipped: None,
    });
    assert_eq!(re.run_when(&mut state, "mail"), Status::Accept);
    state.get_context().write().unwrap().body = Body::Raw(
        r#"From: john doe <john@doe.com>
To: green@foo.net
Subject: test email

This is a raw email.
"#
        .to_string(),
    );
    assert_eq!(re.run_when(&mut state, "preq"), Status::Accept);
    assert_eq!(re.run_when(&mut state, "postq"), Status::Accept);

    // raw mail should have been written on disk.
    assert_eq!(
        std::fs::read_to_string("./tests/generated/test_message_id.eml")
            .expect("could not read 'test_message_id'"),
        r#"From: john doe <john@doe.com>
To: green@foo.net
Subject: test email

This is a raw email.
"#
    );

    std::fs::remove_file("./tests/generated/test_message_id.eml")
        .expect("could not remove generated test file");
}

#[test]
fn test_context_dump() {
    crate::receiver::test_helpers::logs::setup();
    std::fs::DirBuilder::new()
        .recursive(true)
        .create("./tests/generated")
        .unwrap();

    let re = RuleEngine::new(&Some("./src/rules/tests/email/dump/main.vsl".into()))
        .expect("couldn't build rule engine");
    let mut state = get_default_state();

    state.get_context().write().unwrap().metadata = Some(MessageMetadata {
        message_id: "test_message_id".to_string(),
        timestamp: std::time::SystemTime::now(),
        retry: 0,
        resolver: "default".to_string(),
        skipped: None,
    });
    assert_eq!(re.run_when(&mut state, "mail"), Status::Accept);
    state.get_context().write().unwrap().body = Body::Raw(String::default());
    assert_eq!(re.run_when(&mut state, "preq"), Status::Accept);
    state.get_context().write().unwrap().body = Body::Parsed(Box::new(Mail {
        headers: vec![
            ("From".to_string(), "john@doe.com".to_string()),
            ("To".to_string(), "green@bar.net".to_string()),
            ("X-Custom-Header".to_string(), "my header".to_string()),
        ],
        body: BodyType::Regular(vec!["this is an empty body".to_string()]),
    }));
    assert_eq!(re.run_when(&mut state, "postq"), Status::Accept);

    assert_eq!(
        std::fs::read_to_string("./tests/generated/test_message_id.dump.json")
            .expect("could not read 'test_message_id'"),
        serde_json::to_string_pretty(&*state.get_context().read().unwrap())
            .expect("couldn't convert context into string")
    );

    std::fs::remove_file("./tests/generated/test_message_id.dump.json")
        .expect("could not remove generated test file");
}
