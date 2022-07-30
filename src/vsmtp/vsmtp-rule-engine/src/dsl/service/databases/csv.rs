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

use super::{AccessMode, Refresh};
use crate::{api::EngineResult, dsl::service::Service};
use std::{io::Write, str::FromStr};
use vsmtp_common::re::anyhow::{self, Context};

/// query a record matching the first element.
pub fn query_key(
    path: &std::path::PathBuf,
    delimiter: u8,
    _: &Refresh,
    fd: &std::fs::File,
    key: &str,
) -> anyhow::Result<Option<csv::StringRecord>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .trim(csv::Trim::All)
        .delimiter(delimiter)
        .from_reader(fd);

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
}

/// add a record to the csv database.
pub fn add_record(
    path: &std::path::PathBuf,
    delimiter: u8,
    fd: &std::fs::File,
    record: &[String],
) -> anyhow::Result<()> {
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .delimiter(delimiter)
        .from_writer(fd);

    writer
        .write_record(record)
        .context(format!("failed to write to csv database at {path:?}"))?;

    writer
        .flush()
        .context(format!("failed to write to csv database at {path:?}"))?;

    Ok(())
}

/// remove a record from a csv database.
pub fn remove_record(path: &std::path::PathBuf, key: &str) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(path)
        .context(format!("failed to read a csv database at {path:?}"))?;

    let mut writer = std::io::BufWriter::new(
        std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(path)
            .context(format!("failed to open a csv database at {path:?}"))?,
    );

    for line in content.lines() {
        if !line.starts_with(key) {
            writer
                .write_vectored(&[
                    std::io::IoSlice::new(line.as_bytes()),
                    std::io::IoSlice::new(b"\n"),
                ])
                .context(format!("failed to update a csv database at {path:?}"))?;
        }
    }

    writer
        .flush()
        .context(format!("failed to update a csv database at {path:?}"))?;

    Ok(())
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

    let fd = std::fs::OpenOptions::new()
        .append(true)
        .read(match access {
            AccessMode::ReadWrite | AccessMode::Read => true,
            AccessMode::Write => false,
        })
        .write(match access {
            AccessMode::ReadWrite | AccessMode::Write => true,
            AccessMode::Read => false,
        })
        .open(&connector)
        .map_err::<Box<rhai::EvalAltResult>, _>(|err| {
            format!("could not load database at {connector:?}: {err}").into()
        })?;

    Ok(Service::CSVDatabase {
        path: connector,
        delimiter,
        access,
        refresh,
        fd,
    })
}
