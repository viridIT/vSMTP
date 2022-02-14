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
#[cfg(test)]
pub mod test {
    use crate::rules::{
        rule_engine::{RuleEngine, Status},
        tests::get_default_state,
    };

    #[tokio::test]
    async fn test_valid_connect_rules() {
        let re =
            RuleEngine::new("./src/rules/tests/rules/connect").expect("couldn't build rule engine");
        let mut state = get_default_state();

        // ctx.client_addr is 0.0.0.0 by default.
        state.get_context().write().unwrap().client_addr = "127.0.0.1:0".parse().unwrap();
        assert_eq!(re.run_when(&mut state, "connect"), Status::Continue);

        state.get_context().write().unwrap().client_addr = "0.0.0.0:0".parse().unwrap();
        assert_eq!(re.run_when(&mut state, "connect"), Status::Deny);
    }
}
