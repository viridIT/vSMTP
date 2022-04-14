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
use vsmtp_common::{
    address::Address,
    mail::{BodyType, Mail},
    mail_context::{Body, MessageMetadata},
    state::StateSMTP,
    status::Status,
};

#[test]
fn test_email_context() {
    let re = RuleEngine::new(&Some(rules_path!["main.vsl"])).unwrap();
    let mut state = get_default_state("./tmp/app");

    assert_eq!(re.run_when(&mut state, &StateSMTP::Connect), Status::Accept);
    state.get_context().write().unwrap().body = Body::Raw(String::default());
    assert_eq!(re.run_when(&mut state, &StateSMTP::PreQ), Status::Accept);
    state.get_context().write().unwrap().body = Body::Parsed(Box::new(Mail {
        headers: vec![],
        body: BodyType::Regular(vec![]),
    }));
    state.get_context().write().unwrap().envelop.rcpt = vec![
        Address::try_from("rcpt@toremove.org".to_string())
            .unwrap()
            .into(),
        Address::try_from("rcpt@torewrite.net".to_string())
            .unwrap()
            .into(),
    ];
    state.get_context().write().unwrap().metadata = Some(MessageMetadata::default());
    assert_eq!(re.run_when(&mut state, &StateSMTP::PostQ), Status::Accept);
}

#[test]
fn test_email_bcc() {
    let re = RuleEngine::new(&Some(rules_path!["bcc", "main.vsl"])).unwrap();
    let mut state = get_default_state("./tmp/app");

    assert_eq!(re.run_when(&mut state, &StateSMTP::PostQ), Status::Accept);
}

#[test]
fn test_email_add_header() {
    let re = RuleEngine::new(&Some(rules_path!["add_header", "main.vsl"])).unwrap();
    let mut state = get_default_state("./tmp/app");

    state.get_context().write().unwrap().body = Body::Raw(String::default());
    assert_eq!(re.run_when(&mut state, &StateSMTP::PreQ), Status::Accept);
    state.get_context().write().unwrap().body = Body::Parsed(Box::new(Mail {
        headers: vec![],
        body: BodyType::Regular(vec![]),
    }));
    state.get_context().write().unwrap().metadata = Some(MessageMetadata::default());
    assert_eq!(re.run_when(&mut state, &StateSMTP::PostQ), Status::Accept);
}
