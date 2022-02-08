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

fn register_rules(rules, stage) {
    let status = CONTINUE();

    for r in rules {
        if r.when == stage {
            let status = switch r.condition() {
                true => r.on_success(),
                false => r.on_failure(),
            };

            if status != CONTINUE() {
                return status;
            }
        }
    }

    // in this case, status == CONTINUE, default is used.
    status
}