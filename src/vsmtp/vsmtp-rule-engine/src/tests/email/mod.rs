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
use crate::{
    rule_engine::RuleEngine,
    rule_state::RuleState,
    tests::helpers::{get_default_config, get_default_state},
};
use vsmtp_common::{
    addr,
    mail::{BodyType, Mail},
    mail_context::{Body, MessageMetadata},
    state::StateSMTP,
    status::Status,
    CodeID, ReplyOrCodeID,
};

#[test]
fn test_email_context() {
    let config = get_default_config("./tmp/app");
    let re = RuleEngine::new(&config, &Some(rules_path!["main.vsl"])).unwrap();
    let resolvers = std::sync::Arc::new(std::collections::HashMap::new());
    let mut state = RuleState::new(&config, resolvers, &re);

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::Connect),
        Status::Accept(ReplyOrCodeID::CodeID(CodeID::Ok)),
    );
    state.context().write().unwrap().body = Body::Raw(String::default());
    assert_eq!(
        re.run_when(&mut state, &StateSMTP::PreQ),
        Status::Accept(ReplyOrCodeID::CodeID(CodeID::Ok)),
    );
    state.context().write().unwrap().body = Body::Parsed(Box::new(Mail {
        headers: vec![(
            "to".to_string(),
            "other.rcpt@toremove.org, other.rcpt@torewrite.net".to_string(),
        )],
        body: BodyType::Regular(vec![]),
    }));
    state.context().write().unwrap().envelop.rcpt = vec![
        addr!("rcpt@toremove.org").into(),
        addr!("rcpt@torewrite.net").into(),
    ];
    state.context().write().unwrap().metadata = Some(MessageMetadata::default());
    assert_eq!(
        re.run_when(&mut state, &StateSMTP::PostQ),
        Status::Accept(ReplyOrCodeID::CodeID(CodeID::Ok)),
    );

    assert_eq!(
        state.context().read().unwrap().body.get_header("to"),
        Some("other.new@rcpt.net, other.added@rcpt.com")
    );
}

#[test]
fn test_email_bcc() {
    let config = get_default_config("./tmp/app");
    let re = RuleEngine::new(&config, &Some(rules_path!["bcc", "main.vsl"])).unwrap();
    let resolvers = std::sync::Arc::new(std::collections::HashMap::new());
    let mut state = RuleState::new(&config, resolvers, &re);

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::PostQ),
        Status::Accept(ReplyOrCodeID::CodeID(CodeID::Ok)),
    );
}

#[test]
fn test_email_add_get_set_header() {
    let config = get_default_config("./tmp/app");
    let re = RuleEngine::new(&config, &Some(rules_path!["mutate_header", "main.vsl"])).unwrap();
    let resolvers = std::sync::Arc::new(std::collections::HashMap::new());
    let mut state = RuleState::new(&config, resolvers, &re);

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::Connect),
        Status::Deny(ReplyOrCodeID::CodeID(CodeID::Denied))
    );
    let (mut state, _) = get_default_state("./tmp/app");
    state.context().write().unwrap().body = Body::Raw(String::default());
    assert_eq!(
        re.run_when(&mut state, &StateSMTP::PreQ),
        Status::Accept(ReplyOrCodeID::CodeID(CodeID::Ok)),
    );
    state.context().write().unwrap().body = Body::Parsed(Box::new(Mail {
        headers: vec![],
        body: BodyType::Regular(vec![]),
    }));
    state.context().write().unwrap().metadata = Some(MessageMetadata::default());
    assert_eq!(
        re.run_when(&mut state, &StateSMTP::PostQ),
        Status::Accept(ReplyOrCodeID::CodeID(CodeID::Ok)),
    );
}
