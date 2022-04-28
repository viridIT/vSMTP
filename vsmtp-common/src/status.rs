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

/// A packet send from the application (.vsl) to the server (vsmtp)
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum InfoPacket {
    /// a string
    Str(String),
    /// a custom code.
    Code {
        /// the base code (550, 250 ...)
        base: i32,
        /// the enhanced code {5.7.1 ...}
        enhanced: String,
        /// a message to send.
        text: String,
    },
}

impl ToString for InfoPacket {
    fn to_string(&self) -> String {
        match self {
            InfoPacket::Str(string) => string.clone(),
            InfoPacket::Code {
                base,
                enhanced,
                text,
            } => {
                format!("{base} {enhanced} {text}")
            }
        }
    }
}

/// Status of the mail context treated by the rule engine
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Status {
    /// informational data needs to be sent to the client.
    Info(InfoPacket),

    /// accepts the current stage value, skips all rules in the stage.
    Accept,

    /// continue to the next rule / stage.
    Next,

    /// immediately stops the transaction and send an error code.
    Deny,

    /// ignore all future rules for the current transaction.
    Faccept,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Status::Accept => "accept",
                Status::Next => "next",
                Status::Deny => "deny",
                Status::Faccept => "faccept",
                Status::Info(_) => "info",
            }
        )
    }
}
