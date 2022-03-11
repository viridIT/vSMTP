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

// runs rules for a given stage and returns
// a status.
fn run_rules(rules, stage) {
    let status = vsl::next();

    if stage in rules {
        for procedure in rules[stage] {
            let result;

            try {
                result = procedure.evaluate.call();
            } catch (err) {
                err.set("rule", procedure.name);
                result = err;
            };

            switch result.type_of() {
                "vsmtp_common::status::Status" => {
                    switch procedure.type {
                        "rule" => status = result,
                        _ => {},
                    };
                },
                "()" => switch procedure.type {
                    "rule" => throw `the rule "${procedure.name}" must return a status`,
                    _ => {}
                },
                _ => throw result,
            }

            if status != vsl::next() {
                return status;
            }
        }
    }

    status
}
