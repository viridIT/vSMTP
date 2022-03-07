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
    mime::parser::MailMimeParser,
    rules::{
        address::Address,
        rule_engine::{RuleEngine, Status},
        tests::helpers::get_default_state,
    },
    smtp::mail::Body,
};

#[test]
fn test_connect_rules() {
    let re = RuleEngine::new(&Some("./src/rules/tests/rules/connect/main.vsl".into()))
        .expect("couldn't build rule engine");
    let mut state = get_default_state();

    // ctx.client_addr is 0.0.0.0 by default.
    state.get_context().write().unwrap().client_addr = "127.0.0.1:0".parse().unwrap();
    assert_eq!(re.run_when(&mut state, "connect"), Status::Next);

    state.get_context().write().unwrap().client_addr = "0.0.0.0:0".parse().unwrap();
    assert_eq!(re.run_when(&mut state, "connect"), Status::Deny);
}

#[test]
fn test_helo_rules() {
    crate::receiver::test_helpers::logs::setup_logs();

    let re = RuleEngine::new(&Some("./src/rules/tests/rules/helo/main.vsl".into()))
        .expect("couldn't build rule engine");

    let mut state = get_default_state();
    state.get_context().write().unwrap().envelop.helo = "viridit.com".to_string();

    assert_eq!(re.run_when(&mut state, "connect"), Status::Next);
    assert_eq!(re.run_when(&mut state, "helo"), Status::Next);
}

#[test]
fn test_mail_from_rules() {
    crate::receiver::test_helpers::logs::setup_logs();

    let re = RuleEngine::new(&Some("./src/rules/tests/rules/mail/main.vsl".into()))
        .expect("couldn't build rule engine");

    let mut state = get_default_state();
    {
        let email = state.get_context();
        let mut email = email.write().unwrap();

        email.envelop.mail_from = Address::new("staff@viridit.com").unwrap();
        email.body = Body::Parsed(Box::new(
            MailMimeParser::default()
                .parse(
                    br#"From: staff <staff@viridit.com>
Date: Fri, 21 Nov 1997 10:01:10 -0600

This is a reply to your hello."#,
                )
                .unwrap(),
        ));
    }

    assert_eq!(re.run_when(&mut state, "mail"), Status::Accept);
    assert_eq!(re.run_when(&mut state, "postq"), Status::Accept);
    assert_eq!(
        state.get_context().read().unwrap().envelop.mail_from.full(),
        "no-reply@viridit.com"
    );
}

#[test]
fn test_rcpt_rules() {
    crate::receiver::test_helpers::logs::setup_logs();

    let re = RuleEngine::new(&Some("./src/rules/tests/rules/rcpt/main.vsl".into()))
        .expect("couldn't build rule engine");

    let mut state = get_default_state();
    {
        let email = state.get_context();
        let mut email = email.write().unwrap();

        email.envelop.rcpt = std::collections::HashSet::from_iter([
            Address::new("johndoe@compagny.com").unwrap(),
            Address::new("user@viridit.com").unwrap(),
            Address::new("customer@company.com").unwrap(),
        ]);

        email.body = Body::Parsed(Box::new(
            MailMimeParser::default()
                .parse(
                    br#"From: staff <staff@viridit.com>
Date: Fri, 21 Nov 1997 10:01:10 -0600

This is a reply to your hello."#,
                )
                .unwrap(),
        ));
    }

    assert_eq!(re.run_when(&mut state, "rcpt"), Status::Accept);
    assert_eq!(re.run_when(&mut state, "postq"), Status::Next);
    assert_eq!(
        state.get_context().read().unwrap().envelop.rcpt,
        std::collections::HashSet::from_iter([
            Address::new("johndoe@viridit.com").unwrap(),
            Address::new("user@viridit.com").unwrap(),
            Address::new("no-reply@viridit.com").unwrap(),
        ])
    );
}
