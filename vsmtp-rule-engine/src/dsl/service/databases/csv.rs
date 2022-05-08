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

use vsmtp_common::re::anyhow;

use crate::{dsl::service::Service, modules::EngineResult};

use super::{AccessMode, Refresh};

/// query a record matching the first element.
pub fn query_key(
    path: &std::path::PathBuf,
    access: &AccessMode,
    delimiter: u8,
    _: &Refresh,
    content: &str,
    key: &str,
) -> anyhow::Result<Option<csv::StringRecord>> {
    if let AccessMode::Read | AccessMode::ReadWrite = access {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .trim(csv::Trim::All)
            .delimiter(delimiter)
            .from_reader(content.as_bytes());

        for record in reader.records() {
            match record {
                Ok(record) => {
                    if record.get(0).filter(|fst| *fst == key).is_some() {
                        return Ok(Some(record));
                    }
                }
                Err(err) => anyhow::bail!(
                    "tried to read from csv database {path:?}, but an error occurred: {err}"
                ),
            };
        }

        Ok(None)
    } else {
        anyhow::bail!("tried to read from csv database {path:?}, but the access mode was {access}")
    }
}

pub fn parse_csv_database(db_name: &str, options: &rhai::Map) -> EngineResult<Service> {
    for key in ["connector", "access", "refresh", "delimiter"] {
        if !options.contains_key(key) {
            return Err(format!("database {db_name} is missing the '{key}' option.").into());
        }
    }

    let connector =
        std::path::PathBuf::from_str(&options.get("connector").unwrap().to_string()).unwrap();

    let refresh = options.get("refresh").unwrap().to_string();
    let refresh = Refresh::from_str(&refresh).map_err::<Box<rhai::EvalAltResult>, _>(|_| {
        format!("{} is not a correct database refresh rate", refresh).into()
    })?;

    let delimiter = options
        .get("delimiter")
        .unwrap()
        .as_char()
        .map_err::<Box<rhai::EvalAltResult>, _>(|_| {
            "the delimiter of a csv database must be a single char".into()
        })? as u8;

    let access = options.get("access").unwrap().to_string();
    let access = AccessMode::from_str(&access).map_err::<Box<rhai::EvalAltResult>, _>(|_| {
        format!("{} is not a correct database access mode", access).into()
    })?;

    let content =
        std::fs::read_to_string(&connector).map_err::<Box<rhai::EvalAltResult>, _>(|err| {
            format!("could not load database at {connector:?}: {err}").into()
        })?;

    Ok(Service::CSVDatabase {
        path: connector,
        delimiter,
        access,
        refresh,
        content,
    })
}
