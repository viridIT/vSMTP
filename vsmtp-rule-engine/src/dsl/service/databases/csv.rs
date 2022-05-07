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

use std::str::FromStr;

use crate::{dsl::service::Service, modules::EngineResult};

use super::{AccessMode, Refresh};

pub fn parse_csv_database(db_name: &str, options: &rhai::Map) -> EngineResult<Service> {
    for key in ["connector", "access", "refresh", "pattern"] {
        if !options.contains_key(key) {
            return Err(format!("database {db_name} is missing the '{key}' option.").into());
        }
    }

    let connector = options.get("connector").unwrap().to_string();
    let access = options.get("access").unwrap().to_string();
    let refresh = options.get("refresh").unwrap().to_string();
    let pattern = options.get("pattern").unwrap().to_string();

    Ok(Service::CSVDatabase {
        path: std::path::PathBuf::from_str(&connector).unwrap(),
        access: AccessMode::from_str(&access).map_err::<Box<rhai::EvalAltResult>, _>(|_| {
            format!("{} is not a correct database access mode", access).into()
        })?,
        refresh: Refresh::from_str(&refresh).map_err::<Box<rhai::EvalAltResult>, _>(|_| {
            format!("{} is not a correct database refresh rate", refresh).into()
        })?,
        pattern,
    })
}
