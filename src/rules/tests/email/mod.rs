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
    crate::receiver::test_helpers::logs::setup_logs();

    let re = RuleEngine::new("./src/rules/tests/email".into()).expect("couldn't build rule engine");
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
    crate::receiver::test_helpers::logs::setup_logs();

    let re = RuleEngine::new("./src/rules/tests/email/bcc").expect("couldn't build rule engine");
    let mut state = get_default_state();

    assert_eq!(re.run_when(&mut state, "postq"), Status::Accept);
}

#[test]
fn test_email_add_header() {
    crate::receiver::test_helpers::logs::setup_logs();

    let re =
        RuleEngine::new("./src/rules/tests/email/add_header").expect("couldn't build rule engine");
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
    crate::receiver::test_helpers::logs::setup_logs();

    let re = RuleEngine::new("./src/rules/tests/email/write".into())
        .expect("couldn't build rule engine");
    let mut state = get_default_state();

    assert_eq!(re.run_when(&mut state, "mail"), Status::Accept);
    state.get_context().write().unwrap().body = Body::Raw(String::default());
    assert_eq!(re.run_when(&mut state, "preq"), Status::Accept);
    assert_eq!(re.run_when(&mut state, "postq"), Status::Accept);
}

#[test]
fn test_context_dump() {
    crate::receiver::test_helpers::logs::setup_logs();

    let re = RuleEngine::new("./src/rules/tests/email/dump").expect("couldn't build rule engine");
    let mut state = get_default_state();

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
}
