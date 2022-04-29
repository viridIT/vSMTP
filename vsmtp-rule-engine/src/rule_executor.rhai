/*
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
*/

/// runs rules for a given stage and returns
/// a status.
/// if an error is catched by the function,
/// deny is returned.
fn run_rules(rules, stage) {
    if stage in rules {
        for procedure in rules[stage] {
            let result;

            try {
                result = procedure.evaluate.call();
            } catch (err) {
                throw switch err.type_of() {
                    "string" => {
                        #{
                            "error": "ErrorInFunctionCall",
                            "rule": procedure.name,
                            "message": err
                        }
                    },
                    _ => {
                        err.set("rule", procedure.name);
                        err
                    }
                };
            }

            let status = switch procedure.type {
                "rule" => switch result.type_of() {
                    "vsmtp_common::status::Status" => result,
                    _ => throw `the rule '${procedure.name}' must return a status, not a '${result.type_of()}'`,
                },
                "action" => vsl::next(),
                _ => throw `procedure "${procedure.type}" does not exists`,
            };

            if status != vsl::next() {
                return status;
            }
        }
    }

    // status returned by default.
    vsl::next()
}
