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
use vsmtp_common::status::Status;

#[test]
fn test_engine_errors() {
    let re = RuleEngine::new(&Some(rules_path!["error_handling", "main.vsl"])).unwrap();
    let mut state = get_default_state();

    assert_eq!(re.run_when(&mut state, "connect"), Status::Next);
    assert_eq!(re.run_when(&mut state, "helo"), Status::Next);
    assert_eq!(re.run_when(&mut state, "mail"), Status::Deny);
    assert_eq!(re.run_when(&mut state, "rcpt"), Status::Deny);
}

#[test]
fn test_engine_rules_syntax() {
    let re = RuleEngine::new(&Some(rules_path!["syntax", "main.vsl"])).unwrap();
    let mut state = get_default_state();

    assert_eq!(re.run_when(&mut state, "connect"), Status::Accept);
    assert_eq!(re.run_when(&mut state, "helo"), Status::Next);
    assert_eq!(re.run_when(&mut state, "mail"), Status::Next);
    assert_eq!(re.run_when(&mut state, "rcpt"), Status::Next);
    assert_eq!(re.run_when(&mut state, "preq"), Status::Next);
    assert_eq!(re.run_when(&mut state, "postq"), Status::Next);
}
