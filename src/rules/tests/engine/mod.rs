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
use crate::rules::{
    rule_engine::{RuleEngine, Status},
    tests::helpers::get_default_state,
};

#[test]
fn test_engine_errors() {
    crate::receiver::test_helpers::logs::setup_logs();

    let re = RuleEngine::new("./src/rules/tests/engine/error_handling".into())
        .expect("couldn't build rule engine");
    let mut state = get_default_state();

    assert_eq!(re.run_when(&mut state, "connect"), Status::Next);
    assert_eq!(re.run_when(&mut state, "helo"), Status::Next);
    assert_eq!(re.run_when(&mut state, "mail"), Status::Deny);
}

#[test]
fn test_engine_rules_syntax() {
    crate::receiver::test_helpers::logs::setup_logs();

    let re = RuleEngine::new("./src/rules/tests/engine/syntax".into())
        .expect("couldn't build rule engine");
    let mut state = get_default_state();

    assert_eq!(re.run_when(&mut state, "connect"), Status::Accept);
    assert_eq!(re.run_when(&mut state, "helo"), Status::Next);
    assert_eq!(re.run_when(&mut state, "mail"), Status::Next);
    assert_eq!(re.run_when(&mut state, "rcpt"), Status::Next);
    assert_eq!(re.run_when(&mut state, "preq"), Status::Next);
    assert_eq!(re.run_when(&mut state, "postq"), Status::Next);
}
