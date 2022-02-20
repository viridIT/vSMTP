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
// the "stage" variable to work properly.
// use it like this: run_rules!(rules)
fn run_rules(rules, stage) {
    let status = vsl::next();

    if rules.contains(stage) {
        for procedure in rules[stage] {
            try {
                let result = procedure.evaluate.call();

                switch procedure.type {
                    "rule" => {
                        switch result.type_of() {
                            "vsmtp::rules::rule_engine::Status" => status = result,
                            _ => {
                                throw #{
                                function: "run_rules",
                                error: "ErrorInFunctionCall",
                                message: "rules must return a status",
                                line: 0,
                                position: 0,
                                source: "",
                              };
                            }
                        };
                    },
                    _ => {}
                };
            } catch (err) {
                err.set("rule", procedure.name);
                throw err;
            }

            if status != vsl::next() {
                return status;
            }
        }
    }

    status
}