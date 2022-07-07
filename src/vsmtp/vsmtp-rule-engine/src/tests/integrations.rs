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

// TODO: move this file to vsmtp-test.
//       it's here right now because of the convenient macros
//       to locate vsl's example scripts.

use crate::{rule_engine::RuleEngine, rule_state::RuleState, tests::helpers::get_default_config};
use vsmtp_common::ReplyCode::Enhanced;
use vsmtp_common::{
    addr, rcpt::Rcpt, state::StateSMTP, status::Status, CodeID, Reply, ReplyOrCodeID,
};

#[test]
fn test_greylist() {
    std::fs::File::create(root_example!["greylist/greylist.csv"]).unwrap();

    let config = get_default_config("./tmp/app");
    let re = RuleEngine::new(&config, &Some(root_example!["greylist/main.vsl"])).unwrap();
    let resolvers = std::sync::Arc::new(std::collections::HashMap::new());
    let mut state = RuleState::new(&config, resolvers.clone(), &re);

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::MailFrom),
        Status::Deny(ReplyOrCodeID::CodeID(CodeID::Denied))
    );

    let re = RuleEngine::new(&config, &Some(root_example!["greylist/main.vsl"])).unwrap();
    let mut state = RuleState::new(&config, resolvers, &re);

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::MailFrom),
        Status::Accept(ReplyOrCodeID::CodeID(CodeID::Ok)),
    );

    std::fs::remove_file(root_example!["greylist/greylist.csv"]).unwrap();
}

// TODO: add more test cases for this example.
#[test]
fn test_check_relay() {
    let config = get_default_config("./tmp/app");
    let re = RuleEngine::new(&config, &Some(root_example!["anti_relaying/main.vsl"])).unwrap();

    let resolvers = std::sync::Arc::new(std::collections::HashMap::new());
    let mut state = RuleState::new(&config, resolvers.clone(), &re);

    // using our domain but the sender isn't identified.
    state.context().write().unwrap().envelop.mail_from = addr!("satan@testserver.com");

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::MailFrom),
        Status::Deny(ReplyOrCodeID::Reply(Reply::new(
            Enhanced {
                code: 554,
                enhanced: "5.7.1".to_string()
            },
            "Relay access denied"
        )))
    );

    let mut state = RuleState::new(&config, resolvers.clone(), &re);

    state
        .context()
        .write()
        .unwrap()
        .envelop
        .rcpt
        .push(Rcpt::new(addr!("satan@example.com")));

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::RcptTo),
        Status::Info(ReplyOrCodeID::Reply(Reply::new(
            Enhanced {
                code: 554,
                enhanced: "5.7.1".to_string()
            },
            "Relay access denied"
        )))
    );

    let mut state = RuleState::new(&config, resolvers, &re);

    state
        .context()
        .write()
        .unwrap()
        .envelop
        .rcpt
        .push(Rcpt::new(addr!("john.doe@testserver.com")));

    assert_eq!(re.run_when(&mut state, &StateSMTP::RcptTo), Status::Next);
}
