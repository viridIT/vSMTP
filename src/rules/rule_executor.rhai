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

// functions are pure, this is why the executor is
// wrapped in one. this function needs to capture
// "stage" & "rules" variables to work properly.
fn run_rules() {
    let status = CONTINUE();

    if rules.contains(stage) {
        for r in rules[stage] {
            let result = r.evaluate.call();

            switch r.type {
                "rule" => {
                    if result.type_of() != "vsmtp::rules::rule_engine::Status" {
                        status = r.default_status;
                    } else {
                        status = result;
                    }
                }
                _ => {}
            }

            if status != CONTINUE() {
                return status;
            }
        }
    }

    status
}