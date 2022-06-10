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

pub mod cmd;
pub mod databases;
pub mod parsing;

#[derive(Debug)]
pub enum Service {
    /// A service can be a program to run in a subprocess
    Cmd {
        /// a duration after which the subprocess will be forced-kill
        timeout: std::time::Duration,
        /// optional: a user to run the subprocess under
        user: Option<String>,
        /// optional: a group to run the subprocess under
        group: Option<String>,
        /// the command to execute in the subprocess
        command: String,
        /// optional: parameters directly given to the executed program (argc, argv)
        args: Option<Vec<String>>,
    },

    /// a database connector based on the csv file format.
    CSVDatabase {
        /// a path to the file to open.
        path: std::path::PathBuf,
        /// access mode to the database.
        access: databases::AccessMode,
        /// delimiter character to separate fields in records.
        delimiter: u8,
        /// database refresh mode.
        refresh: databases::Refresh,
        /// raw content of the database.
        fd: std::fs::File,
    },
}

impl std::fmt::Display for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Service::Cmd { .. } => "cmd",
                Service::CSVDatabase { .. } => "csv-database",
            }
        )
    }
}
