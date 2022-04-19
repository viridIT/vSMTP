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
    address::Address, mail_context::Body, state::StateSMTP, status::Status, MailParser,
};
use vsmtp_mail_parser::MailMimeParser;

#[test]
fn test_connect_rules() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(root_example!["rules/connect.vsl"]),
    )
    .unwrap();
    let mut state = get_default_state("./tmp/app");

    // ctx.client_addr is 0.0.0.0 by default.
    state.get_context().write().unwrap().client_addr = "127.0.0.1:0".parse().unwrap();
    assert_eq!(re.run_when(&mut state, &StateSMTP::Connect), Status::Next);

    state.get_context().write().unwrap().client_addr = "0.0.0.0:0".parse().unwrap();
    assert_eq!(re.run_when(&mut state, &StateSMTP::Connect), Status::Deny);
}

#[test]
fn test_helo_rules() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(root_example!["rules/helo.vsl"]),
    )
    .unwrap();
    let mut state = get_default_state("./tmp/app");
    state.get_context().write().unwrap().envelop.helo = "example.com".to_string();

    assert_eq!(re.run_when(&mut state, &StateSMTP::Connect), Status::Next);
    assert_eq!(re.run_when(&mut state, &StateSMTP::Helo), Status::Next);
}

#[test]
fn test_mail_from_rules() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(root_example!["rules/mail.vsl"]),
    )
    .unwrap();

    let mut state = get_default_state("./tmp/app");
    {
        let email = state.get_context();
        let mut email = email.write().unwrap();

        email.envelop.mail_from = Address::try_from("staff@example.com".to_string()).unwrap();
        email.body = Body::Parsed(Box::new(
            MailMimeParser::default()
                .parse(
                    br#"From: staff <staff@example.com>
Date: Fri, 21 Nov 1997 10:01:10 -0600

This is a reply to your hello."#,
                )
                .unwrap(),
        ));
    }

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::MailFrom),
        Status::Accept
    );
    assert_eq!(re.run_when(&mut state, &StateSMTP::PostQ), Status::Accept);
    assert_eq!(
        state.get_context().read().unwrap().envelop.mail_from.full(),
        "no-reply@example.com"
    );
}

#[test]
fn test_rcpt_rules() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(root_example!["rules/rcpt.vsl"]),
    )
    .unwrap();

    let mut state = get_default_state("./tmp/app");
    {
        let email = state.get_context();
        let mut email = email.write().unwrap();

        email.envelop.rcpt = vec![
            vsmtp_common::rcpt::Rcpt::new(
                Address::try_from("johndoe@compagny.com".to_string()).unwrap(),
            ),
            vsmtp_common::rcpt::Rcpt::new(
                Address::try_from("user@example.com".to_string()).unwrap(),
            ),
            vsmtp_common::rcpt::Rcpt::new(
                Address::try_from("customer@company.com".to_string()).unwrap(),
            ),
        ];

        email.body = Body::Parsed(Box::new(
            MailMimeParser::default()
                .parse(
                    br#"From: staff <staff@example.com>
Date: Fri, 21 Nov 1997 10:01:10 -0600

This is a reply to your hello."#,
                )
                .unwrap(),
        ));
    }

    assert_eq!(re.run_when(&mut state, &StateSMTP::RcptTo), Status::Accept);
    assert_eq!(re.run_when(&mut state, &StateSMTP::PostQ), Status::Next);
    assert_eq!(
        state.get_context().read().unwrap().envelop.rcpt,
        vec![
            vsmtp_common::rcpt::Rcpt::new(
                Address::try_from("johndoe@example.com".to_string()).unwrap()
            ),
            vsmtp_common::rcpt::Rcpt::new(
                Address::try_from("user@example.com".to_string()).unwrap()
            ),
            vsmtp_common::rcpt::Rcpt::new(
                Address::try_from("no-reply@example.com".to_string()).unwrap()
            ),
        ]
    );
}
