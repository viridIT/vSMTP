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

use crate::modules::EngineResult;

pub fn parse_csv_database(db_name: &str, mut options: rhai::Map) -> EngineResult<rhai::Map> {
    options.insert("type".into(), rhai::Dynamic::from("database".to_string()));
    options.insert("format".into(), rhai::Dynamic::from("csv".to_string()));

    for key in ["connector", "open", "refresh", "pattern"] {
        if !options.contains_key(key) {
            return Err(format!("database {db_name} is missing the '{key}' key.").into());
        }
    }

    Ok(options)
}
